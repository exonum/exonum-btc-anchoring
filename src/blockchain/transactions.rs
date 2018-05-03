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

use bitcoin::blockdata::transaction::SigHashType;

use exonum::crypto::CryptoHash;
use exonum::blockchain::{ExecutionResult, Schema, Transaction};
use exonum::messages::Message;
use exonum::storage::{Fork, Snapshot};
use exonum::helpers::Height;

use blockchain::dto::{MsgAnchoringSignature, MsgAnchoringUpdateLatest};
use blockchain::schema::AnchoringSchema;
use blockchain::consensus_storage::AnchoringConfig;
use details::btc;
use details::btc::transactions::{AnchoringTx, BitcoinTx, FundingTx, TxKind};
use super::Error as ValidateError;

impl MsgAnchoringSignature {
    pub fn verify_content(&self) -> bool {
        // Do not verify signatures other than SigHashType::All
        let sighash_type_all = SigHashType::All.as_u32() as u8;
        if self.signature().last() != Some(&sighash_type_all) {
            warn!(
                "Received msg with incorrect signature type, content={:#?}",
                self
            );
            return false;
        }
        let tx = self.tx();
        // Checks that the signature is provided for an existing anchoring tx input
        if tx.input.len() as u32 <= self.input() {
            warn!(
                "Received msg for non-existing input index, content={:#?}",
                self
            );
            return false;
        }
        // Checks that inputs do not contain witness data.
        for input in &tx.input {
            if !input.witness.is_empty() {
                warn!(
                    "Received msg with non-empty input scriptSigs, content={:#?}",
                    self
                );
                return false;
            }
        }
        true
    }

    pub fn validate(&self, view: &Fork) -> Result<(), ValidateError> {
        let core_schema = Schema::new(&view);
        let anchoring_schema = AnchoringSchema::new(&view);

        let tx = self.tx();
        let prev_txid = tx.input[self.input() as usize].prev_hash.into();
        let id = self.validator().0 as usize;
        let actual_cfg = core_schema.actual_configuration();
        // Verify from field
        if actual_cfg.validator_keys.get(id).map(|k| k.service_key) != Some(*self.from()) {
            return Err(ValidateError::MsgFromNonValidator);
        }

        // Verify signature
        let anchoring_cfg = anchoring_schema.actual_anchoring_config();
        if let Some(pub_key) = anchoring_cfg.anchoring_keys.get(id) {
            let (redeem_script, addr) = anchoring_cfg.redeem_script();
            let tx_script_pubkey = tx.script_pubkey();
            // Use following address if it exists
            let addr = if let Some(following) = anchoring_schema.following_anchoring_config() {
                following.redeem_script().1
            } else {
                addr
            };
            if tx_script_pubkey != &addr.script_pubkey() {
                return Err(ValidateError::MsgWithIncorrectAddress);
            }
            verify_anchoring_tx_payload(&tx, &core_schema)?;
            // Checks whether funding tx is suitable as prev tx because they are not added to
            // the known_txs automatically.
            let prev_tx = if anchoring_cfg.funding_tx().id() == prev_txid {
                anchoring_cfg.funding_tx().clone().0
            } else {
                anchoring_schema
                    .known_txs()
                    .get(&prev_txid)
                    .ok_or_else(|| ValidateError::LectWithIncorrectContent)?
                    .0
            };
            if !tx.verify_input(
                &redeem_script,
                self.input(),
                &prev_tx,
                pub_key,
                self.signature(),
            ) {
                return Err(ValidateError::SignatureIncorrect);
            }
            Ok(())
        } else {
            return Err(ValidateError::MsgFromNonValidator);
        }
    }
}

impl Transaction for MsgAnchoringSignature {
    fn verify(&self) -> bool {
        self.verify_signature(self.from()) && self.verify_content()
    }

    fn execute(&self, fork: &mut Fork) -> ExecutionResult {
        self.validate(fork)?;
        let mut anchoring_schema = AnchoringSchema::new(fork);
        anchoring_schema
            .add_known_signature(self.clone())
            .map_err(Into::into)
    }
}

impl MsgAnchoringUpdateLatest {
    pub fn validate(&self, view: &Fork) -> Result<(btc::PublicKey, BitcoinTx), ValidateError> {
        let anchoring_schema = AnchoringSchema::new(view);
        let core_schema = Schema::new(view);

        let tx = self.tx();
        let id = self.validator().0 as usize;
        // Verify lect with actual cfg
        let actual_cfg = core_schema.actual_configuration();

        if actual_cfg.validator_keys.get(id).map(|k| k.service_key) != Some(*self.from()) {
            return Err(ValidateError::MsgFromNonValidator);
        }

        let anchoring_cfg = anchoring_schema.actual_anchoring_config();
        let key = &anchoring_cfg.anchoring_keys[id];
        match TxKind::from(tx.clone()) {
            TxKind::Anchoring(tx) => {
                verify_anchoring_tx_payload(&tx, &core_schema)?;
                verify_anchoring_tx_prev_hash(&tx, &anchoring_schema)?;
            }
            TxKind::FundingTx(tx) => {
                let anchoring_cfg = anchoring_schema.genesis_anchoring_config();
                verify_funding_tx(&tx, &anchoring_cfg)?;
            }
            TxKind::Other(_) => return Err(ValidateError::LectWithIncorrectContent),
        }

        if anchoring_schema.lects(key).len() != self.lect_count() {
            return Err(ValidateError::LectWithWrongCount);
        }

        Ok((*key, tx))
    }
}

impl Transaction for MsgAnchoringUpdateLatest {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, view: &mut Fork) -> ExecutionResult {
        let (key, tx) = self.validate(view)?;
        AnchoringSchema::new(view).add_lect(&key, tx, self.hash());
        Ok(())
    }
}

fn verify_anchoring_tx_prev_hash<T>(
    tx: &AnchoringTx,
    anchoring_schema: &AnchoringSchema<T>,
) -> Result<(), ValidateError>
where
    T: AsRef<Snapshot>,
{
    // If tx has `prev_tx_chain` should be used it instead of `prev_hash`.
    let prev_txid = tx.payload().prev_tx_chain.unwrap_or_else(|| tx.prev_hash());
    // Get `AnchoringConfig` for prev_tx
    let anchoring_cfg = {
        let prev_tx = anchoring_schema
            .known_txs()
            .get(&prev_txid)
            .ok_or_else(|| ValidateError::LectWithoutQuorum)?;
        let cfg_height = match TxKind::from(prev_tx) {
            TxKind::Anchoring(tx) => Ok(tx.payload().block_height),
            TxKind::FundingTx(_) => Ok(Height::zero()),
            TxKind::Other(_) => Err(ValidateError::LectWithIncorrectContent),
        }?;
        anchoring_schema.anchoring_config_by_height(cfg_height)
    };

    let prev_lects_count = {
        let mut prev_lects_count = 0;
        for key in &anchoring_cfg.anchoring_keys {
            if let Some(prev_lect_idx) = anchoring_schema.find_lect_position(key, &prev_txid) {
                let prev_lect = anchoring_schema
                    .lects(key)
                    .get(prev_lect_idx)
                    .expect(&format!(
                        "Lect with \
                         index {} is \
                         absent in \
                         lects table \
                         for validator \
                         {}",
                        prev_lect_idx,
                        key.to_string()
                    ));
                assert_eq!(
                    prev_txid,
                    prev_lect.tx().id(),
                    "Inconsistent reference to previous lect in Exonum"
                );

                prev_lects_count += 1;
            }
        }
        prev_lects_count
    };
    if prev_lects_count >= anchoring_cfg.majority_count() {
        Ok(())
    } else {
        Err(ValidateError::LectWithoutQuorum)
    }
}

fn verify_anchoring_tx_payload<T>(tx: &AnchoringTx, schema: &Schema<T>) -> Result<(), ValidateError>
where
    T: AsRef<Snapshot>,
{
    let payload = tx.payload();
    if schema.block_hashes_by_height().get(payload.block_height.0) == Some(payload.block_hash) {
        Ok(())
    } else {
        Err(ValidateError::MsgWithIncorrectPayload)
    }
}

fn verify_funding_tx(tx: &FundingTx, anchoring_cfg: &AnchoringConfig) -> Result<(), ValidateError> {
    if tx == anchoring_cfg.funding_tx() {
        Ok(())
    } else {
        Err(ValidateError::LectWithIncorrectFunding)
    }
}
