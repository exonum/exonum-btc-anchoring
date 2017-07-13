// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::hash_map::{Entry, HashMap};

use byteorder::{BigEndian, ByteOrder};
use serde_json::value::from_value;

use exonum::blockchain::{Schema, StoredConfiguration, gen_prefix};
use exonum::storage::{Fork, ListIndex, MapIndex, ProofListIndex, Snapshot, StorageKey};
use exonum::crypto::Hash;

use blockchain::consensus_storage::AnchoringConfig;
use blockchain::dto::{LectContent, MsgAnchoringSignature};
use details::btc;
use details::btc::transactions::{AnchoringTx, BitcoinTx};
use service::{ANCHORING_SERVICE_ID, ANCHORING_SERVICE_NAME};

/// Unique identifier of signature for the `AnchoringTx`.
#[derive(Debug)]
pub struct KnownSignatureId {
    /// Normalized txid of the `AnchoringTx`.
    pub txid: btc::TxId,
    /// Identifier of the anchoring node in the current configuration.
    pub validator_id: u16,
    /// Transaction input for the signature.
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

/// Anchoring information schema.
#[derive(Debug)]
pub struct AnchoringSchema<T> {
    view: T,
}

impl<T> AnchoringSchema<T>
where
    T: AsRef<Snapshot>,
{
    /// Creates anchoring schema for the given `snapshot`.
    pub fn new(snapshot: T) -> AnchoringSchema<T> {
        AnchoringSchema { view: snapshot }
    }

    /// Returns table that contains signatures for the anchoring transaction with
    /// the given normalized `txid`.
    pub fn signatures(&self, txid: &btc::TxId) -> ListIndex<&T, MsgAnchoringSignature> {
        let prefix = self.gen_table_prefix(2, txid);
        ListIndex::new(prefix, &self.view)
    }

    /// Returns table that saves a list of lects for the validator with the given `validator_key`.
    pub fn lects(&self, validator_key: &btc::PublicKey) -> ProofListIndex<&T, LectContent> {
        let prefix = self.gen_table_prefix(3, validator_key);
        ProofListIndex::new(prefix, &self.view)
    }

    /// Returns table that keeps the lect index for every anchoring txid for the validator
    /// with given `validator_key`.
    pub fn lect_indexes(&self, validator_key: &btc::PublicKey) -> MapIndex<&T, btc::TxId, u64> {
        let prefix = self.gen_table_prefix(4, validator_key);
        MapIndex::new(prefix, &self.view)
    }

    /// Returns the table of known signatures, where key is the tuple `(txid, validator_id, input)`.
    ///
    /// [Read more.](struct.KnownSignatureId.html)
    pub fn known_signatures(&self) -> MapIndex<&T, KnownSignatureId, MsgAnchoringSignature> {
        let prefix = self.gen_table_prefix(6, &());
        MapIndex::new(prefix, &self.view)
    }

    /// Returns the table that keeps the anchoring transaction for any known txid.
    pub fn known_txs(&self) -> MapIndex<&T, btc::TxId, BitcoinTx> {
        let prefix = self.gen_table_prefix(7, &());
        MapIndex::new(prefix, &self.view)
    }

    /// Returns table that maps anchoring transactions to their heights.
    pub fn anchoring_tx_chain(&self) -> MapIndex<&T, u64, AnchoringTx> {
        let prefix = self.gen_table_prefix(128, &());
        MapIndex::new(prefix, &self.view)
    }

    /// Returns the actual anchoring configuration.
    pub fn actual_anchoring_config(&self) -> AnchoringConfig {
        let schema = Schema::new(&self.view);
        let actual = schema.actual_configuration();
        self.parse_config(&actual)
    }

    /// Returns the nearest following configuration if it exists.
    pub fn following_anchoring_config(&self) -> Option<AnchoringConfig> {
        let schema = Schema::new(&self.view);
        if let Some(stored) = schema.following_configuration() {
            Some(self.parse_config(&stored))
        } else {
            None
        }
    }

    /// Returns the previous anchoring configuration if it exists.
    pub fn previous_anchoring_config(&self) -> Option<AnchoringConfig> {
        let schema = Schema::new(&self.view);
        if let Some(stored) = schema.previous_configuration() {
            Some(self.parse_config(&stored))
        } else {
            None
        }
    }

    /// Returns the anchoring configuration from the genesis block.
    pub fn genesis_anchoring_config(&self) -> AnchoringConfig {
        self.anchoring_config_by_height(0)
    }

    /// Returns the configuration that is the actual for the given `height`.
    /// For non-existent heights, it will return the configuration closest to them.
    pub fn anchoring_config_by_height(&self, height: u64) -> AnchoringConfig {
        let schema = Schema::new(&self.view);
        let stored = schema.configuration_by_height(height);
        self.parse_config(&stored)
    }

    /// Returns `lect` for validator with the given `public_key`.
    pub fn lect(&self, validator_key: &btc::PublicKey) -> Option<BitcoinTx> {
        self.lects(validator_key).last().map(|x| x.tx())
    }

    /// Returns previous `lect` for validator with the given `public_key`.
    pub fn prev_lect(&self, validator_key: &btc::PublicKey) -> Option<BitcoinTx> {
        let lects = self.lects(validator_key);

        let idx = lects.len();
        if idx > 1 {
            lects.get(idx - 2).map(|content| content.tx())
        } else {
            None
        }
    }

    /// Returns a lect that is currently supported by at least 2/3 of the current set of validators.
    pub fn collect_lects(&self, cfg: &AnchoringConfig) -> Option<BitcoinTx> {
        let mut lects = HashMap::new();
        for anchoring_key in &cfg.anchoring_keys {
            if let Some(last_lect) = self.lect(anchoring_key) {
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

    /// Returns position in `lects` table of validator with the given `anchoring_key`
    /// for transaction with the given `txid`.
    pub fn find_lect_position(
        &self,
        anchoring_key: &btc::PublicKey,
        txid: &btc::TxId,
    ) -> Option<u64> {
        self.lect_indexes(anchoring_key).get(txid)
    }

    /// Returns the `state_hash` for anchoring tables.
    ///
    /// It contains a list of `root_hash` of the actual `lects` tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        let cfg = self.actual_anchoring_config();
        let mut lect_hashes = Vec::new();
        for key in &cfg.anchoring_keys {
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

impl<'a> AnchoringSchema<&'a mut Fork> {
    /// Mutable variant of the [`signatures`][1] index.
    ///
    /// [1]: struct.AnchoringSchema.html#method.signatures
    pub fn signatures_mut(
        &mut self,
        txid: &btc::TxId,
    ) -> ListIndex<&mut Fork, MsgAnchoringSignature> {
        let prefix = self.gen_table_prefix(2, txid);
        ListIndex::new(prefix, &mut self.view)
    }

    /// Mutable variant of the [`lects`][1] index.
    ///
    /// [1]: struct.AnchoringSchema.html#method.lects
    pub fn lects_mut(
        &mut self,
        validator_key: &btc::PublicKey,
    ) -> ProofListIndex<&mut Fork, LectContent> {
        let prefix = self.gen_table_prefix(3, validator_key);
        ProofListIndex::new(prefix, &mut self.view)
    }

    /// Mutable variant of the [`lect_indexes`][1] index.
    ///
    /// [1]: struct.AnchoringSchema.html#method.lect_indexes
    pub fn lect_indexes_mut(
        &mut self,
        validator_key: &btc::PublicKey,
    ) -> MapIndex<&mut Fork, btc::TxId, u64> {
        let prefix = self.gen_table_prefix(4, validator_key);
        MapIndex::new(prefix, &mut self.view)
    }


    /// Mutable variant of the [`known_signatures`][1] index.
    ///
    /// [1]: struct.AnchoringSchema.html#method.known_signatures
    pub fn known_signatures_mut(
        &mut self,
    ) -> MapIndex<&mut Fork, KnownSignatureId, MsgAnchoringSignature> {
        let prefix = self.gen_table_prefix(6, &());
        MapIndex::new(prefix, &mut self.view)
    }

    /// Mutable variant of the [`known_txs`][1] index.
    ///
    /// [1]: struct.AnchoringSchema.html#method.known_txs
    pub fn known_txs_mut(&mut self) -> MapIndex<&mut Fork, btc::TxId, BitcoinTx> {
        let prefix = self.gen_table_prefix(7, &());
        MapIndex::new(prefix, &mut self.view)
    }

    /// Mutable variant of the [`signatures`][1] index.
    ///
    /// [1]: struct.AnchoringSchema.html#method.anchoring_tx_chain
    pub fn anchoring_tx_chain_mut(&mut self) -> MapIndex<&mut Fork, u64, AnchoringTx> {
        let prefix = self.gen_table_prefix(128, &());
        MapIndex::new(prefix, &mut self.view)
    }

    /// Creates and commits the genesis anchoring configuration from the proposed `cfg`.
    pub fn create_genesis_config(&mut self, cfg: &AnchoringConfig) {
        for validator_key in &cfg.anchoring_keys {
            self.add_lect(validator_key, cfg.funding_tx().clone(), Hash::zero());
        }
    }

    /// Adds `lect` from validator with the given `public key`.
    pub fn add_lect<Tx>(&mut self, validator_key: &btc::PublicKey, tx: Tx, msg_hash: Hash)
    where
        Tx: Into<BitcoinTx>,
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

    /// Adds signature to known if it is correct.
    pub fn add_known_signature(&mut self, msg: MsgAnchoringSignature) {
        let ntxid = msg.tx().nid();
        let signature_id = KnownSignatureId::from(&msg);
        if let Some(sign_msg) = self.known_signatures().get(&signature_id) {
            warn!(
                "Received another signature for given tx propose msg={:#?}",
                sign_msg
            );
        } else {
            self.signatures_mut(&ntxid).push(msg.clone());
            self.known_signatures_mut().put(&signature_id, msg);
        }
    }
}

impl<T> AnchoringSchema<T> {
    /// Converts schema back into snapshot.
    pub fn into_snapshot(self) -> T {
        self.view
    }
}
