use std::collections::hash_map::{Entry, HashMap};

use byteorder::{BigEndian, ByteOrder};
use serde_json::value::from_value;

use exonum::blockchain::{Schema, StoredConfiguration};
use exonum::storage::{Error as StorageError, List, ListTable, Map, MapTable, MerkleTable, View};
use exonum::crypto::Hash;

use bitcoin::util::base58::ToBase58;
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::ANCHORING_SERVICE_ID;
use blockchain::dto::{LectContent, MsgAnchoringSignature};
use details::btc;
use details::btc::transactions::BitcoinTx;

#[doc(hidden)]
pub struct AnchoringSchema<'a> {
    view: &'a View,
}

impl<'a> AnchoringSchema<'a> {
    pub fn new(view: &'a View) -> AnchoringSchema {
        AnchoringSchema { view: view }
    }

    pub fn signatures(&self,
                      txid: &btc::TxId)
                      -> ListTable<MapTable<View, [u8], Vec<u8>>, MsgAnchoringSignature> {
        let prefix = [&[ANCHORING_SERVICE_ID as u8, 2], txid.as_ref()].concat();
        ListTable::new(MapTable::new(prefix, self.view))
    }

    pub fn lects(&self, validator: u32) -> MerkleTable<MapTable<View, [u8], Vec<u8>>, LectContent> {
        let mut prefix = vec![ANCHORING_SERVICE_ID as u8, 3, 0, 0, 0, 0];
        BigEndian::write_u32(&mut prefix[2..6], validator);
        MerkleTable::new(MapTable::new(prefix, self.view))
    }

    pub fn lect_indexes(&self, validator: u32) -> MapTable<View, btc::TxId, u64> {
        let mut prefix = vec![ANCHORING_SERVICE_ID as u8, 4, 0, 0, 0, 0];
        BigEndian::write_u32(&mut prefix[2..6], validator);
        MapTable::new(prefix, self.view)
    }

    // List of known anchoring addresses
    pub fn known_addresses(&self) -> MapTable<View, str, Vec<u8>> {
        let prefix = vec![ANCHORING_SERVICE_ID as u8, 5];
        MapTable::new(prefix, self.view)
    }

    // Key is tuple (txid, validator_id, input), see `known_signature_id`.
    pub fn known_signatures(&self) -> MapTable<View, [u8], MsgAnchoringSignature> {
        let prefix = vec![ANCHORING_SERVICE_ID as u8, 6];
        MapTable::new(prefix, self.view)
    }

    pub fn current_anchoring_config(&self) -> Result<AnchoringConfig, StorageError> {
        let actual = Schema::new(self.view).actual_configuration()?;
        Ok(self.parse_config(&actual))
    }

    pub fn following_anchoring_config(&self) -> Result<Option<AnchoringConfig>, StorageError> {
        let schema = Schema::new(self.view);
        if let Some(stored) = schema.following_configuration()? {
            Ok(Some(self.parse_config(&stored)))
        } else {
            Ok(None)
        }
    }

    pub fn anchoring_config_by_height(&self, height: u64) -> Result<AnchoringConfig, StorageError> {
        let schema = Schema::new(self.view);
        let stored = schema.configuration_by_height(height)?;
        Ok(self.parse_config(&stored))
    }

    pub fn create_genesis_config(&self, cfg: &AnchoringConfig) -> Result<(), StorageError> {
        let (_, addr) = cfg.redeem_script();
        self.add_known_address(&addr)?;
        for idx in 0..cfg.validators.len() {
            self.add_lect(idx as u32, cfg.funding_tx().clone(), Hash::zero())?;
        }
        Ok(())
    }

    pub fn add_lect<Tx>(&self, validator: u32, tx: Tx, msg_hash: Hash) -> Result<(), StorageError>
        where Tx: Into<BitcoinTx>
    {
        let lects = self.lects(validator);

        let tx = tx.into();
        let idx = lects.len()?;
        let txid = tx.id();
        lects.append(LectContent::new(&msg_hash, tx))?;
        self.lect_indexes(validator).put(&txid, idx)
    }

    pub fn lect(&self, validator: u32) -> Result<BitcoinTx, StorageError> {
        self.lects(validator).last().map(|x| x.unwrap().tx())
    }

    pub fn prev_lect(&self, validator: u32) -> Result<Option<BitcoinTx>, StorageError> {
        let lects = self.lects(validator);

        let idx = lects.len()?;
        if idx > 1 {
            let lect = lects.get(idx - 2)?.map(|content| content.tx());
            Ok(lect)
        } else {
            Ok(None)
        }
    }

    pub fn collect_lects(&self) -> Result<Option<BitcoinTx>, StorageError> {
        let cfg = self.current_anchoring_config()?;
        let validators_count = cfg.validators.len() as u32;

        let mut lects = HashMap::new();
        for validator_id in 0..validators_count {
            let last_lect = self.lect(validator_id)?;
            match lects.entry(last_lect.0) {
                Entry::Occupied(mut v) => {
                    *v.get_mut() += 1;
                }
                Entry::Vacant(v) => {
                    v.insert(1);
                }
            }
        }

        let lect = if let Some((lect, count)) = lects.iter().max_by_key(|&(_, v)| v) {
            if *count >= ::majority_count(validators_count as u8) {
                Some(BitcoinTx::from(lect.clone()))
            } else {
                None
            }
        } else {
            None
        };
        Ok(lect)
    }

    pub fn find_lect_position(&self,
                              validator: u32,
                              txid: &btc::TxId)
                              -> Result<Option<u64>, StorageError> {
        self.lect_indexes(validator).get(txid)
    }

    pub fn add_known_address(&self, addr: &btc::Address) -> Result<(), StorageError> {
        self.known_addresses().put(&addr.to_base58check(), vec![])
    }

    pub fn is_address_known(&self, addr: &btc::Address) -> Result<bool, StorageError> {
        self.known_addresses()
            .get(&addr.to_base58check())
            .map(|x| x.is_some())
    }

    pub fn add_known_signature(&self, msg: MsgAnchoringSignature) -> Result<(), StorageError> {
        let ntxid = msg.tx().nid();
        let signature_id = Self::known_signature_id(&msg);
        if let Some(sign_msg) = self.known_signatures().get(&signature_id)? {
            warn!("Received another signature for given tx propose msg={:#?}",
                  sign_msg);
        } else {
            self.signatures(&ntxid).append(msg.clone())?;
            self.known_signatures().put(&signature_id, msg)?;
        }
        Ok(())
    }

    pub fn state_hash(&self) -> Result<Vec<Hash>, StorageError> {
        let cfg = self.current_anchoring_config()?;
        let mut lect_hashes = Vec::new();
        for id in 0..cfg.validators.len() as u32 {
            lect_hashes.push(self.lects(id).root_hash()?);
        }
        Ok(lect_hashes)
    }

    fn known_signature_id(msg: &MsgAnchoringSignature) -> Vec<u8> {
        let txid = msg.tx().id();

        let mut id = vec![txid.as_ref(), [0; 8].as_ref()].concat();
        BigEndian::write_u32(&mut id[32..36], msg.validator());
        BigEndian::write_u32(&mut id[36..40], msg.input());
        id
    }

    fn parse_config(&self, cfg: &StoredConfiguration) -> AnchoringConfig {
        let service_id = ANCHORING_SERVICE_ID.to_string();
        from_value(cfg.services[&service_id].clone()).expect("Anchoring config does not exist")
    }
}
