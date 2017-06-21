use std::collections::hash_map::{Entry, HashMap};

use byteorder::{BigEndian, ByteOrder};
use serde_json::value::from_value;

use exonum::blockchain::{Schema, StoredConfiguration, gen_prefix};
use exonum::storage::{Fork, ListIndex, MapIndex, ProofListIndex, Snapshot, StorageKey};
use exonum::crypto::Hash;

use blockchain::consensus_storage::AnchoringConfig;
use blockchain::dto::{LectContent, MsgAnchoringSignature};
use details::btc;
use details::btc::transactions::BitcoinTx;
use service::{ANCHORING_SERVICE_ID, ANCHORING_SERVICE_NAME};

pub struct KnownSignatureId {
    pub txid: btc::TxId,
    pub validator_id: u16,
    pub input: u32,
}

impl StorageKey for KnownSignatureId {
    fn size(&self) -> usize {
        self.txid.size() + 6
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer[0..32].copy_from_slice(self.txid.as_ref());
        BigEndian::write_u16(&mut buffer[32..34], self.validator_id);
        BigEndian::write_u32(&mut buffer[34..38], self.input);
    }

    fn read(buffer: &[u8]) -> Self {
        let txid = btc::TxId::read(&buffer[0..32]);
        let validator_id = u16::read(&buffer[32..34]);
        let input = u32::read(&buffer[34..38]);
        KnownSignatureId {
            txid,
            validator_id,
            input,
        }
    }
}

impl<'a> From<&'a MsgAnchoringSignature> for KnownSignatureId {
    fn from(msg: &'a MsgAnchoringSignature) -> KnownSignatureId {
        KnownSignatureId {
            txid: msg.tx().id(),
            validator_id: msg.validator(),
            input: msg.input(),
        }
    }
}

#[doc(hidden)]
pub struct AnchoringSchema<T> {
    view: T,
}

// Define readonly indicies
impl<T> AnchoringSchema<T>
    where T: AsRef<Snapshot>
{
    pub fn new(snapshot: T) -> AnchoringSchema<T> {
        AnchoringSchema { view: snapshot }
    }

    pub fn signatures(&self, txid: &btc::TxId) -> ListIndex<&T, MsgAnchoringSignature> {
        let prefix = self.gen_table_prefix(2, txid);
        ListIndex::new(prefix, &self.view)
    }

    pub fn lects(&self, validator_key: &btc::PublicKey) -> ProofListIndex<&T, LectContent> {
        let prefix = self.gen_table_prefix(3, validator_key);
        ProofListIndex::new(prefix, &self.view)
    }

    pub fn lect_indexes(&self, validator_key: &btc::PublicKey) -> MapIndex<&T, btc::TxId, u64> {
        let prefix = self.gen_table_prefix(4, validator_key);
        MapIndex::new(prefix, &self.view)
    }

    // Key is tuple (txid, validator_id, input), see `known_signature_id`.
    pub fn known_signatures(&self) -> MapIndex<&T, KnownSignatureId, MsgAnchoringSignature> {
        let prefix = self.gen_table_prefix(6, &());
        MapIndex::new(prefix, &self.view)
    }

    pub fn known_txs(&self) -> MapIndex<&T, btc::TxId, BitcoinTx> {
        let prefix = self.gen_table_prefix(7, &());
        MapIndex::new(prefix, &self.view)
    }

    pub fn actual_anchoring_config(&self) -> AnchoringConfig {
        let schema = Schema::new(&self.view);
        let actual = schema.actual_configuration();
        self.parse_config(&actual)
    }

    pub fn following_anchoring_config(&self) -> Option<AnchoringConfig> {
        let schema = Schema::new(&self.view);
        if let Some(stored) = schema.following_configuration() {
            Some(self.parse_config(&stored))
        } else {
            None
        }
    }

    pub fn previous_anchoring_config(&self) -> Option<AnchoringConfig> {
        let schema = Schema::new(&self.view);
        if let Some(stored) = schema.previous_configuration() {
            Some(self.parse_config(&stored))
        } else {
            None
        }
    }

    pub fn genesis_anchoring_config(&self) -> AnchoringConfig {
        self.anchoring_config_by_height(0)
    }

    pub fn anchoring_config_by_height(&self, height: u64) -> AnchoringConfig {
        let schema = Schema::new(&self.view);
        let stored = schema.configuration_by_height(height);
        self.parse_config(&stored)
    }

    pub fn lect(&self, validator_key: &btc::PublicKey) -> Option<BitcoinTx> {
        self.lects(validator_key).last().map(|x| x.tx())
    }

    pub fn prev_lect(&self, validator_key: &btc::PublicKey) -> Option<BitcoinTx> {
        let lects = self.lects(validator_key);

        let idx = lects.len();
        if idx > 1 {
            lects.get(idx - 2).map(|content| content.tx())
        } else {
            None
        }
    }

    pub fn collect_lects(&self, cfg: &AnchoringConfig) -> Option<BitcoinTx> {
        let mut lects = HashMap::new();
        for validator_key in &cfg.validators {
            if let Some(last_lect) = self.lect(validator_key) {
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

        if let Some((lect, count)) = lects.iter().max_by_key(|&(_, v)| v) {
            if *count >= cfg.majority_count() {
                Some(BitcoinTx::from(lect.clone()))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn find_lect_position(&self,
                              validator_key: &btc::PublicKey,
                              txid: &btc::TxId)
                              -> Option<u64> {
        self.lect_indexes(validator_key).get(txid)
    }

    pub fn state_hash(&self) -> Vec<Hash> {
        let cfg = self.actual_anchoring_config();
        let mut lect_hashes = Vec::new();
        for key in &cfg.validators {
            lect_hashes.push(self.lects(key).root_hash());
        }
        lect_hashes
    }

    fn parse_config(&self, cfg: &StoredConfiguration) -> AnchoringConfig {
        from_value(cfg.services[ANCHORING_SERVICE_NAME].clone())
            .expect("Anchoring config does not exist")
    }

    fn gen_table_prefix<K: StorageKey>(&self, ord: u8, suf: &K) -> Vec<u8> {
        gen_prefix(ANCHORING_SERVICE_ID, ord, suf)
    }
}

// Define mutable operations and indicies
impl<'a> AnchoringSchema<&'a mut Fork> {
    pub fn signatures_mut(&mut self,
                          txid: &btc::TxId)
                          -> ListIndex<&mut Fork, MsgAnchoringSignature> {
        let prefix = self.gen_table_prefix(2, txid);
        ListIndex::new(prefix, &mut self.view)
    }

    pub fn lects_mut(&mut self,
                     validator_key: &btc::PublicKey)
                     -> ProofListIndex<&mut Fork, LectContent> {
        let prefix = self.gen_table_prefix(3, validator_key);
        ProofListIndex::new(prefix, &mut self.view)
    }

    pub fn lect_indexes_mut(&mut self,
                            validator_key: &btc::PublicKey)
                            -> MapIndex<&mut Fork, btc::TxId, u64> {
        let prefix = self.gen_table_prefix(4, validator_key);
        MapIndex::new(prefix, &mut self.view)
    }

    pub fn known_signatures_mut(&mut self)
                                -> MapIndex<&mut Fork, KnownSignatureId, MsgAnchoringSignature> {
        let prefix = self.gen_table_prefix(6, &());
        MapIndex::new(prefix, &mut self.view)
    }

    pub fn known_txs_mut(&mut self) -> MapIndex<&mut Fork, btc::TxId, BitcoinTx> {
        let prefix = self.gen_table_prefix(7, &());
        MapIndex::new(prefix, &mut self.view)
    }


    pub fn add_lect<Tx>(&mut self, validator_key: &btc::PublicKey, tx: Tx, msg_hash: Hash)
        where Tx: Into<BitcoinTx>
    {
        let (tx, txid, idx) = {
            let mut lects = self.lects_mut(validator_key);
            let tx = tx.into();
            let idx = lects.len();
            let txid = tx.id();
            lects.push(LectContent::new(&msg_hash, tx.clone()));
            (tx, txid, idx)
        };

        self.known_txs_mut().put(&txid, tx.clone());
        self.lect_indexes_mut(validator_key).put(&txid, idx)
    }

    pub fn create_genesis_config(&mut self, cfg: &AnchoringConfig) {
        for validator_key in &cfg.validators {
            self.add_lect(validator_key, cfg.funding_tx().clone(), Hash::zero());
        }
    }

    pub fn add_known_signature(&mut self, msg: MsgAnchoringSignature) {
        let ntxid = msg.tx().nid();
        let signature_id = KnownSignatureId::from(&msg);
        if let Some(sign_msg) = self.known_signatures().get(&signature_id) {
            warn!("Received another signature for given tx propose msg={:#?}",
                  sign_msg);
        } else {
            self.signatures_mut(&ntxid).push(msg.clone());
            self.known_signatures_mut().put(&signature_id, msg);
        }
    }
}

impl<T> AnchoringSchema<T> {
    pub fn into_snapshot(self) -> T {
        self.view
    }
}
