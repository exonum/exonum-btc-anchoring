use std::collections::hash_map::{Entry, HashMap};

use byteorder::{BigEndian, ByteOrder};
use serde_json::value::from_value;

use exonum::blockchain::{Schema, StoredConfiguration, gen_prefix};
use exonum::storage::{Error as StorageError, List, ListTable, Map, MapTable, MerkleTable, View};
use exonum::crypto::Hash;

use bitcoin::util::base58::ToBase58;
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::ANCHORING_SERVICE_ID;
use blockchain::dto::{LectContent, MsgAnchoringSignature};
use details::btc;
use details::btc::transactions::BitcoinTx;

/// An anchoring information schema.
pub struct AnchoringSchema<'a> {
    view: &'a View,
}

// Data tables section.
impl<'a> AnchoringSchema<'a> {
    /// Returns table that contains signatures for the anchoring transaction with
    /// the given `normalized` `txid`.
    pub fn signatures(&self,
                      txid: &btc::TxId)
                      -> ListTable<MapTable<View, [u8], Vec<u8>>, MsgAnchoringSignature> {
        let prefix = self.gen_table_prefix(2, Some(txid.as_ref()));
        ListTable::new(MapTable::new(prefix, self.view))
    }

    /// Returns table that saves a list of `lects` for the validator with the given `public_key`.
    pub fn lects(&self,
                 validator_key: &btc::PublicKey)
                 -> MerkleTable<MapTable<View, [u8], Vec<u8>>, LectContent> {
        let prefix = self.gen_table_prefix(3, Some(validator_key.to_bytes().as_ref()));
        MerkleTable::new(MapTable::new(prefix, self.view))
    }

    /// Returns table that keeps the `lect` index for every anchoring `txid` for the validator.
    /// with given `public_key`.
    pub fn lect_indexes(&self, validator_key: &btc::PublicKey) -> MapTable<View, btc::TxId, u64> {
        let prefix = self.gen_table_prefix(4, Some(validator_key.to_bytes().as_ref()));
        MapTable::new(prefix, self.view)
    }

    /// Returns table that caches known anchoring addresses.
    pub fn known_addresses(&self) -> MapTable<View, str, Vec<u8>> {
        let prefix = self.gen_table_prefix(5, None);
        MapTable::new(prefix, self.view)
    }

    /// Returns the table of known signatures, where key is the tuple (txid, validator_id, input),
    /// see [`known_signature_id`](fn) for details.
    pub fn known_signatures(&self) -> MapTable<View, [u8], MsgAnchoringSignature> {
        let prefix = self.gen_table_prefix(6, None);
        MapTable::new(prefix, self.view)
    }

    /// Returns the table that keeps the `anchoring transaction` for any known `txid`.
    pub fn known_txs(&self) -> MapTable<View, btc::TxId, BitcoinTx> {
        let prefix = self.gen_table_prefix(7, None);
        MapTable::new(prefix, self.view)
    }

    fn gen_table_prefix(&self, ord: u8, suf: Option<&[u8]>) -> Vec<u8> {
        gen_prefix(ANCHORING_SERVICE_ID, ord, suf)
    }

    fn known_signature_id(msg: &MsgAnchoringSignature) -> Vec<u8> {
        let txid = msg.tx().id();

        let mut id = vec![txid.as_ref(), [0; 8].as_ref()].concat();
        BigEndian::write_u32(&mut id[32..36], msg.validator());
        BigEndian::write_u32(&mut id[36..40], msg.input());
        id
    }
}

// Business-logic section.
impl<'a> AnchoringSchema<'a> {
    /// Creates information schema for the given `View`.
    pub fn new(view: &'a View) -> AnchoringSchema {
        AnchoringSchema { view: view }
    }

    /// Returns an actual anchoring configuration.
    pub fn actual_anchoring_config(&self) -> Result<AnchoringConfig, StorageError> {
        let actual = Schema::new(self.view).actual_configuration()?;
        Ok(self.parse_config(&actual))
    }

    /// Returns the nearest following configuration if it exists.
    pub fn following_anchoring_config(&self) -> Result<Option<AnchoringConfig>, StorageError> {
        let schema = Schema::new(self.view);
        if let Some(stored) = schema.following_configuration()? {
            Ok(Some(self.parse_config(&stored)))
        } else {
            Ok(None)
        }
    }

    /// Returns the previous anchoring configuration if it exists.
    pub fn previous_anchoring_config(&self) -> Result<Option<AnchoringConfig>, StorageError> {
        let schema = Schema::new(self.view);
        if let Some(stored) = schema.previous_configuration()? {
            Ok(Some(self.parse_config(&stored)))
        } else {
            Ok(None)
        }
    }

    /// Returns the anchoring configuration from the `genesis` block.
    pub fn genesis_anchoring_config(&self) -> Result<AnchoringConfig, StorageError> {
        self.anchoring_config_by_height(0)
    }

    /// Returns the configuration that is the actual for the given height.
    pub fn anchoring_config_by_height(&self, height: u64) -> Result<AnchoringConfig, StorageError> {
        let schema = Schema::new(self.view);
        let stored = schema.configuration_by_height(height)?;
        Ok(self.parse_config(&stored))
    }

    /// Creates and commits the genesis anchoring configuration from the proposed `cfg`.
    pub fn create_genesis_config(&self, cfg: &AnchoringConfig) -> Result<(), StorageError> {
        let (_, addr) = cfg.redeem_script();
        self.add_known_address(&addr)?;
        for validator_key in &cfg.validators {
            self.add_lect(validator_key, cfg.funding_tx().clone(), Hash::zero())?;
        }
        Ok(())
    }

    /// Adds `lect` from validator with the given `public key`.
    pub fn add_lect<Tx>(&self,
                        validator_key: &btc::PublicKey,
                        tx: Tx,
                        msg_hash: Hash)
                        -> Result<(), StorageError>
        where Tx: Into<BitcoinTx>
    {
        let lects = self.lects(validator_key);

        let tx = tx.into();
        let idx = lects.len()?;
        let txid = tx.id();
        lects.append(LectContent::new(&msg_hash, tx.clone()))?;
        self.known_txs().put(&txid, tx.clone())?;
        self.lect_indexes(validator_key).put(&txid, idx)
    }

    /// Returns `lect` for validator with the given `public_key`.
    pub fn lect(&self, validator_key: &btc::PublicKey) -> Result<Option<BitcoinTx>, StorageError> {
        self.lects(validator_key).last().map(|x| x.map(|x| x.tx()))
    }

    /// Returns previous `lect` for validator with the given `public_key`.
    pub fn prev_lect(&self,
                     validator_key: &btc::PublicKey)
                     -> Result<Option<BitcoinTx>, StorageError> {
        let lects = self.lects(validator_key);

        let idx = lects.len()?;
        if idx > 1 {
            let lect = lects.get(idx - 2)?.map(|content| content.tx());
            Ok(lect)
        } else {
            Ok(None)
        }
    }

    /// Returns the current `lect` if it matches `+2/3` of the current set of validators.
    pub fn collect_lects(&self, cfg: &AnchoringConfig) -> Result<Option<BitcoinTx>, StorageError> {
        let mut lects = HashMap::new();
        for validator_key in &cfg.validators {
            if let Some(last_lect) = self.lect(validator_key)? {
                match lects.entry(last_lect.0) {
                    Entry::Occupied(mut v) => {
                        *v.get_mut() += 1;
                    }
                    Entry::Vacant(v) => {
                        v.insert(1);
                    }
                }
            }
        }

        let lect = if let Some((lect, count)) = lects.iter().max_by_key(|&(_, v)| v) {
            if *count >= cfg.majority_count() {
                Some(BitcoinTx::from(lect.clone()))
            } else {
                None
            }
        } else {
            None
        };
        Ok(lect)
    }

    /// Returns position in `lects` table for transaction with the given `txid`.
    pub fn find_lect_position(&self,
                              validator_key: &btc::PublicKey,
                              txid: &btc::TxId)
                              -> Result<Option<u64>, StorageError> {
        self.lect_indexes(validator_key).get(txid)
    }

    /// Marks address as known.
    pub fn add_known_address(&self, addr: &btc::Address) -> Result<(), StorageError> {
        self.known_addresses().put(&addr.to_base58check(), vec![])
    }

    /// Checks that address is known.
    pub fn is_address_known(&self, addr: &btc::Address) -> Result<bool, StorageError> {
        self.known_addresses()
            .get(&addr.to_base58check())
            .map(|x| x.is_some())
    }

    /// Adds signature to known if it is correct.
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

    /// Returns the `state_hash` table for anchoring tables.
    ///
    /// It contains a list of `root_hash` of the actual `lects` tables.
    pub fn state_hash(&self) -> Result<Vec<Hash>, StorageError> {
        let cfg = self.actual_anchoring_config()?;
        let mut lect_hashes = Vec::new();
        for key in &cfg.validators {
            lect_hashes.push(self.lects(key).root_hash()?);
        }
        Ok(lect_hashes)
    }

    fn parse_config(&self, cfg: &StoredConfiguration) -> AnchoringConfig {
        let service_id = ANCHORING_SERVICE_ID.to_string();
        from_value(cfg.services[&service_id].clone()).expect("Anchoring config does not exist")
    }
}
