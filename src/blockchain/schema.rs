// Copyright 2019 The Exonum Team
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

//! Information schema for the btc anchoring service.

use btc_transaction_utils::multisig::RedeemScript;
use exonum::{
    blockchain::Schema,
    crypto::Hash,
    helpers::Height,
    merkledb::{Entry, ObjectAccess, ObjectHash, ProofListIndex, ProofMapIndex, RefMut},
};
use log::{error, trace};

use crate::{
    btc::{BtcAnchoringTransactionBuilder, BuilderError, Transaction},
    config::Config,
};

use super::{data_layout::*, BtcAnchoringState};

/// Information schema for `exonum-btc-anchoring`.
#[derive(Debug)]
pub struct BtcAnchoringSchema<'a, T> {
    instance_name: &'a str,
    access: T,
}

impl<'a, T: ObjectAccess> BtcAnchoringSchema<'a, T> {
    /// Constructs schema for the given database `snapshot`.
    pub fn new(instance_name: &'a str, access: T) -> Self {
        Self {
            instance_name,
            access,
        }
    }

    fn index_name(&self, suffix: &str) -> String {
        [self.instance_name, ".", suffix].concat()
    }

    /// Returns table that contains complete chain of the anchoring transactions.
    pub fn anchoring_transactions_chain(&self) -> RefMut<ProofListIndex<T, Transaction>> {
        self.access
            .get_object(self.index_name("transactions_chain"))
    }

    /// Returns the table that contains already spent funding transactions.
    pub fn spent_funding_transactions(&self) -> RefMut<ProofMapIndex<T, Hash, Transaction>> {
        self.access
            .get_object(self.index_name("spent_funding_transactions"))
    }

    /// Returns the table that contains signatures for the given transaction input.
    pub fn transaction_signatures(&self) -> RefMut<ProofMapIndex<T, TxInputId, InputSignatures>> {
        self.access
            .get_object(self.index_name("transaction_signatures"))
    }

    /// Returns a list of hashes of Exonum blocks headers.
    pub fn anchored_blocks(&self) -> RefMut<ProofListIndex<T, Hash>> {
        self.access.get_object(self.index_name("anchored_blocks"))
    }

    /// Returns an actual anchoring configuration entry.
    pub fn actual_config_entry(&self) -> RefMut<Entry<T, Config>> {
        self.access.get_object(self.index_name("actual_config"))
    }

    /// Returns a following anchoring configuration entry.
    pub fn following_config_entry(&self) -> RefMut<Entry<T, Config>> {
        self.access.get_object(self.index_name("following_config"))
    }

    /// May contain unspent funding transaction for the actual configuration.
    pub fn unspent_funding_transaction_entry(&self) -> RefMut<Entry<T, Transaction>> {
        self.access
            .get_object(self.index_name("unspent_funding_transaction"))
    }

    /// Returns hashes of the stored tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.anchoring_transactions_chain().object_hash(),
            self.spent_funding_transactions().object_hash(),
            self.transaction_signatures().object_hash(),
            self.anchored_blocks().object_hash(),
        ]
    }

    /// Returns the actual anchoring configuration.
    pub fn actual_configuration(&self) -> Config {
        self.actual_config_entry().get().unwrap()
    }

    /// Returns the nearest following configuration if it exists.
    pub fn following_configuration(&self) -> Option<Config> {
        self.following_config_entry().get()
    }

    /// Returns the list of signatures for the given transaction input.
    pub fn input_signatures(
        &self,
        input: &TxInputId,
        redeem_script: &RedeemScript,
    ) -> InputSignatures {
        self.transaction_signatures()
            .get(input)
            .unwrap_or_else(|| InputSignatures::new(redeem_script.content().public_keys.len()))
    }

    /// Returns the actual state of anchoring.
    pub fn actual_state(&self) -> BtcAnchoringState {
        let actual_configuration = self.actual_configuration();
        if let Some(following_configuration) = self.following_configuration() {
            if actual_configuration.redeem_script() != following_configuration.redeem_script() {
                return BtcAnchoringState::Transition {
                    actual_configuration,
                    following_configuration,
                };
            }
        }

        BtcAnchoringState::Regular {
            actual_configuration,
        }
    }

    /// Returns the proposal of next anchoring transaction for the given anchoring state.
    pub fn proposed_anchoring_transaction(
        &self,
        actual_state: &BtcAnchoringState,
    ) -> Option<Result<(Transaction, Vec<Transaction>), BuilderError>> {
        let config = actual_state.actual_configuration();
        let unspent_anchoring_transaction = self.anchoring_transactions_chain().last();
        let unspent_funding_transaction = self.unspent_funding_transaction_entry().get();

        let mut builder = BtcAnchoringTransactionBuilder::new(&config.redeem_script());
        // First anchoring transaction doesn't have previous.
        if let Some(tx) = unspent_anchoring_transaction {
            let tx_id = tx.id();

            // Checks that latest anchoring transaction isn't a transition.
            if actual_state.is_transition() {
                let current_script_pubkey = &tx.0.output[0].script_pubkey;
                let outgoing_script_pubkey = &actual_state.script_pubkey();
                if current_script_pubkey == outgoing_script_pubkey {
                    trace!("Awaiting for new configuration to become an actual.");
                    return None;
                } else {
                    trace!(
                        "Transition from {} to {}.",
                        actual_state.actual_configuration().anchoring_address(),
                        actual_state.output_address(),
                    );
                    builder.transit_to(actual_state.script_pubkey());
                }
            }

            // TODO Re-implement recovery business logic [ECR-3581]
            if let Err(e) = builder.prev_tx(tx) {
                if unspent_funding_transaction.is_none() {
                    return Some(Err(e));
                }
                error!("Anchoring is broken: '{}'. Will try to recover", e);
                builder.recover(tx_id);
            }
        }

        if let Some(tx) = unspent_funding_transaction {
            if let Err(e) = builder.additional_funds(tx) {
                return Some(Err(e));
            }
        }

        // Adds corresponding payload.
        let latest_anchored_height = self.latest_anchored_height();
        let anchoring_height = actual_state.following_anchoring_height(latest_anchored_height);

        let anchoring_block_hash =
            Schema::new(self.access.clone()).block_hash_by_height(anchoring_height)?;

        builder.payload(anchoring_height, anchoring_block_hash);
        builder.fee(config.transaction_fee);

        // Creates anchoring proposal.
        Some(builder.create())
    }

    /// Return the proposal of the next anchoring transaction for the actual anchoring state.
    pub fn actual_proposed_anchoring_transaction(
        &self,
    ) -> Option<Result<(Transaction, Vec<Transaction>), BuilderError>> {
        let actual_state = self.actual_state();
        self.proposed_anchoring_transaction(&actual_state)
    }

    /// Returns the height of the latest anchored block.
    pub fn latest_anchored_height(&self) -> Option<Height> {
        let tx = self.anchoring_transactions_chain().last()?;
        Some(
            tx.anchoring_metadata()
                .expect("Expected payload in the anchoring transaction")
                .1
                .block_height,
        )
    }

    /// Add a finalized transaction to the tail of the anchoring transactions.
    pub fn push_anchoring_transaction(&self, tx: Transaction) {
        // An unspent funding transaction is always unconditionally added to the anchoring
        // transaction proposal, so we can simply move it to the list of spent.
        if let Some(tx) = self.unspent_funding_transaction_entry().take() {
            self.spent_funding_transactions().put(&tx.id(), tx);
        }
        // Special case if we have an active following configuration.
        if let Some(config) = self.following_configuration() {
            // Check that the anchoring transaction is correct.
            let tx_out_script = tx
                .anchoring_metadata()
                .expect("Unable to find metadata in the anchoring transaction.")
                .0;
            // If a following config is exist, then the anchoring transaction's output should have
            // same script as in the following config.
            assert!(
                config.anchoring_out_script() == *tx_out_script,
                "Malformed output address in the anchoring transaction"
            );
            // If preconditions are correct, just reassign the config as an actual.
            self.following_config_entry().remove();
            self.set_actual_config(config);
        }
        self.anchoring_transactions_chain().push(tx);
    }

    /// Set a new anchoring configuration parameters.
    pub fn set_actual_config(&self, config: Config) {
        // TODO remove this special case.
        if let Some(tx) = config
            .funding_transaction
            .clone()
            .filter(|tx| !self.spent_funding_transactions().contains(&tx.id()))
        {
            self.unspent_funding_transaction_entry().set(tx);
        }
        self.actual_config_entry().set(config);
    }
}
