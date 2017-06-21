use bitcoin::blockdata::transaction::SigHashType;
use serde_json::{Value, to_value};

use exonum::blockchain::{Schema, Transaction};
use exonum::messages::Message;
use exonum::crypto::HexValue;
use exonum::storage::{Fork, Snapshot};

use blockchain::dto::{AnchoringMessage, MsgAnchoringSignature, MsgAnchoringUpdateLatest};
use blockchain::schema::AnchoringSchema;
use blockchain::consensus_storage::AnchoringConfig;
use details::btc;
use details::btc::transactions::{AnchoringTx, BitcoinTx, FundingTx, TxKind};

impl MsgAnchoringSignature {
    pub fn verify_content(&self) -> bool {
        // Do not verify signatures other than SigHashType::All
        let sighash_type_all = SigHashType::All.as_u32() as u8;
        if self.signature().last() != Some(&sighash_type_all) {
            warn!("Received msg with incorrect signature type, content={:#?}",
                  self);
            return false;
        }
        let tx = self.tx();
        // Check that the signature is provided for an existing anchoring tx input
        if tx.input.len() as u32 <= self.input() {
            warn!("Received msg for non-existing input index, content={:#?}",
                  self);
            return false;
        }
        // Check that input scriptSigs are empty
        for input in &tx.input {
            if !input.script_sig.is_empty() {
                warn!("Received msg with non empty input scriptSigs, content={:#?}",
                      self);
                return false;
            }
        }
        true
    }

    pub fn validate(&self, view: &Fork) -> bool {
        let core_schema = Schema::new(&view);
        let anchoring_schema = AnchoringSchema::new(&view);

        let tx = self.tx();
        let id = self.validator();
        let actual_cfg = core_schema.actual_configuration();
        // Verify from field
        if actual_cfg.validators.get(id as usize) != Some(self.from()) {
            warn!("Received msg from non-validator, content={:#?}", self);
            return false;
        }

        // Verify signature
        let anchoring_cfg = anchoring_schema.actual_anchoring_config();
        if let Some(pub_key) = anchoring_cfg.validators.get(id as usize) {
            let (redeem_script, addr) = anchoring_cfg.redeem_script();
            let tx_addr = tx.output_address(anchoring_cfg.network);
            // Use following address if it exists
            let addr = if let Some(following) = anchoring_schema.following_anchoring_config() {
                following.redeem_script().1
            } else {
                addr
            };
            if tx_addr != addr {
                warn!("Received msg with incorrect output address, content={:#?}",
                      self);
                return false;
            }
            if !verify_anchoring_tx_payload(&tx, &core_schema) {
                warn!("Received msg with incorrect payload, content={:#?}", self);
                return false;
            }
            if !tx.verify_input(&redeem_script, self.input(), pub_key, self.signature()) {
                warn!("Received msg with incorrect signature, content={:#?}", self);
                return false;
            }
            return true;
        } else {
            return false;
        }
    }

    pub fn execute(&self, fork: &mut Fork) {
        if !self.validate(fork) {
            return;
        }

        let mut anchoring_schema = AnchoringSchema::new(fork);
        anchoring_schema.add_known_signature(self.clone())
    }
}

impl MsgAnchoringUpdateLatest {
    pub fn verify_content(&self) -> bool {
        true
    }

    pub fn validate(&self, view: &Fork) -> Option<(btc::PublicKey, BitcoinTx)> {
        let anchoring_schema = AnchoringSchema::new(view);
        let core_schema = Schema::new(view);

        let tx = self.tx();
        let id = self.validator();
        // Verify lect with actual cfg
        let actual_cfg = core_schema.actual_configuration();

        if actual_cfg.validators.get(self.validator() as usize) != Some(self.from()) {
            warn!("Received lect from non validator, content={:#?}", self);
            return None;
        }

        let anchoring_cfg = anchoring_schema.actual_anchoring_config();
        let key = &anchoring_cfg.validators[id as usize];
        match TxKind::from(tx.clone()) {
            TxKind::Anchoring(tx) => {
                if !verify_anchoring_tx_payload(&tx, &core_schema) {
                    warn!("Received lect with incorrect payload, content={:#?}", self);
                    return None;
                }
                if !verify_anchoring_tx_prev_hash(&tx, &anchoring_schema) {
                    warn!("Received lect with prev_lect without 2/3+ confirmations, \
                            content={:#?}",
                          self);
                    return None;
                }
            }
            TxKind::FundingTx(tx) => {
                let anchoring_cfg = anchoring_schema.genesis_anchoring_config();
                if !verify_funding_tx(&tx, &anchoring_cfg) {
                    warn!("Received lect with incorrect funding_tx, content={:#?}",
                          self);
                    return None;
                }
            }
            TxKind::Other(_) => panic!("Incorrect fields deserialization."),
        }

        if anchoring_schema.lects(key).len() != self.lect_count() {
            warn!("Received lect with wrong count, content={:#?}", self);
            return None;
        }

        Some((*key, tx))
    }

    pub fn execute(&self, view: &mut Fork) {
        if let Some((key, tx)) = self.validate(view) {
            let mut anchoring_schema = AnchoringSchema::new(view);
            anchoring_schema.add_lect(&key, tx, self.hash())
        }
    }
}

impl AnchoringMessage {
    pub fn verify_content(&self) -> bool {
        match *self {
            AnchoringMessage::Signature(ref msg) => msg.verify_content(),
            AnchoringMessage::UpdateLatest(ref msg) => msg.verify_content(),
        }
    }
}


impl Transaction for AnchoringMessage {
    fn verify(&self) -> bool {
        self.verify_signature(self.from()) && self.verify_content()
    }

    fn execute(&self, view: &mut Fork) {
        match *self {
            AnchoringMessage::Signature(ref msg) => msg.execute(view),
            AnchoringMessage::UpdateLatest(ref msg) => msg.execute(view),
        }
    }

    fn info(&self) -> Value {
        to_value(self).unwrap()
    }
}

fn verify_anchoring_tx_prev_hash<T>(tx: &AnchoringTx, anchoring_schema: &AnchoringSchema<T>) -> bool
    where T: AsRef<Snapshot>
{
    // If tx has `prev_tx_chain` should be used it instead of `prev_hash`.
    let prev_txid = tx.payload().prev_tx_chain.unwrap_or_else(|| tx.prev_hash());
    // Get `AnchoringConfig` for prev_tx
    let anchoring_cfg = {
        let cfg_height = anchoring_schema
            .known_txs()
            .get(&prev_txid)
            .and_then(|tx| {
                          let height = match TxKind::from(tx) {
                              TxKind::Anchoring(tx) => tx.payload().block_height,
                              TxKind::FundingTx(_) => 0,
                              TxKind::Other(tx) => panic!("Incorrect lect content={:#?}", tx),
                          };
                          Some(height)
                      });

        if let Some(height) = cfg_height {
            anchoring_schema.anchoring_config_by_height(height)
        } else {
            warn!("Prev lect is unknown txid={:?}", prev_txid);
            return false;
        }
    };

    let prev_lects_count = {
        let mut prev_lects_count = 0;
        for key in &anchoring_cfg.validators {
            if let Some(prev_lect_idx) = anchoring_schema.find_lect_position(key, &prev_txid) {
                let prev_lect = anchoring_schema
                    .lects(key)
                    .get(prev_lect_idx)
                    .expect(&format!("Lect with index {} is absent in lects table for validator \
                                     {}",
                                    prev_lect_idx,
                                    key.to_hex()));
                assert_eq!(prev_txid,
                           prev_lect.tx().id(),
                           "Inconsistent reference to previous lect in Exonum");

                prev_lects_count += 1;
            }
        }
        prev_lects_count
    };
    prev_lects_count >= anchoring_cfg.majority_count()
}

fn verify_anchoring_tx_payload<T>(tx: &AnchoringTx, schema: &Schema<T>) -> bool
    where T: AsRef<Snapshot>
{
    let payload = tx.payload();
    schema.block_hashes_by_height().get(payload.block_height) == Some(payload.block_hash)
}

fn verify_funding_tx(tx: &FundingTx, anchoring_cfg: &AnchoringConfig) -> bool {
    tx == anchoring_cfg.funding_tx()
}
