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

use exonum::{
    blockchain::Schema,
    crypto::Hash,
    helpers::Height,
    merkledb::{Entry, IndexAccess, ObjectHash, ProofListIndex, ProofMapIndex},
};
use log::{error, trace};

use crate::{
    btc::{self, BtcAnchoringTransactionBuilder, BuilderError, Sha256d, Transaction},
    config::Config,
    proto::BinaryMap,
};

use super::{data_layout::*, BtcAnchoringState};

/// A set of signatures for a transaction input ordered by the validators identifiers.
pub type InputSignatures = BinaryMap<u16, btc::InputSignature>;

/// Information schema for `exonum-btc-anchoring`.
#[derive(Debug)]
pub struct BtcAnchoringSchema<'a, T> {
    instance_name: &'a str,
    access: T,
}

impl<'a, T: IndexAccess> BtcAnchoringSchema<'a, T> {
    /// Constructs a schema for the given database `access` object.
    pub fn new(instance_name: &'a str, access: T) -> Self {
        Self {
            instance_name,
            access,
        }
    }

    fn index_name(&self, suffix: &str) -> String {
        [self.instance_name, ".", suffix].concat()
    }

    /// Returns a table that contains complete chain of the anchoring transactions.
    pub fn anchoring_transactions_chain(&self) -> ProofListIndex<T, Transaction> {
        ProofListIndex::new(self.index_name("transactions_chain"), self.access.clone())
    }

    /// Returns a table that contains already spent funding transactions.
    pub fn spent_funding_transactions(&self) -> ProofMapIndex<T, Sha256d, Transaction> {
        ProofMapIndex::new(
            self.index_name("spent_funding_transactions"),
            self.access.clone(),
        )
    }

    /// Returns a table that contains signatures for the given transaction input.
    pub fn transaction_signatures(&self) -> ProofMapIndex<T, TxInputId, InputSignatures> {
        ProofMapIndex::new(
            self.index_name("transaction_signatures"),
            self.access.clone(),
        )
    }

    /// Returns an actual anchoring configuration entry.
    pub fn actual_config_entry(&self) -> Entry<T, Config> {
        Entry::new(self.index_name("actual_config"), self.access.clone())
    }

    /// Returns a following anchoring configuration entry.
    pub fn following_config_entry(&self) -> Entry<T, Config> {
        Entry::new(self.index_name("following_config"), self.access.clone())
    }

    /// Returns an entry that may contain an unspent funding transaction for the
    /// actual configuration.
    pub fn unspent_funding_transaction_entry(&self) -> Entry<T, Transaction> {
        Entry::new(
            self.index_name("unspent_funding_transaction"),
            self.access.clone(),
        )
    }

    /// Returns object hashes of the stored tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.anchoring_transactions_chain().object_hash(),
            self.spent_funding_transactions().object_hash(),
            self.transaction_signatures().object_hash(),
        ]
    }

    /// Returns an actual anchoring configuration.
    pub fn actual_config(&self) -> Config {
        self.actual_config_entry().get().expect(
            "Actual configuration of anchoring is absent. \
             If this error occurs, inform the service authors about it.",
        )
    }

    /// Returns the nearest following configuration if it exists.
    pub fn following_config(&self) -> Option<Config> {
        self.following_config_entry().get()
    }

    /// Returns the list of signatures for the given transaction input.
    pub fn input_signatures(&self, input: &TxInputId) -> InputSignatures {
        self.transaction_signatures().get(input).unwrap_or_default()
    }

    /// Returns an actual state of anchoring.
    pub fn actual_state(&self) -> BtcAnchoringState {
        let actual_configuration = self.actual_config();
        if let Some(following_configuration) = self.following_config() {
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

    /// Returns the proposal of the next anchoring transaction for the given anchoring state.
    pub fn proposed_anchoring_transaction(
        &self,
        actual_state: &BtcAnchoringState,
    ) -> Option<Result<(Transaction, Vec<Transaction>), BuilderError>> {
        let config = actual_state.actual_config();
        let unspent_anchoring_transaction = self.anchoring_transactions_chain().last();
        let unspent_funding_transaction = self.unspent_funding_transaction_entry().get();

        let mut builder = BtcAnchoringTransactionBuilder::new(&config.redeem_script());
        // First anchoring transaction doesn't have previous.
        if let Some(tx) = unspent_anchoring_transaction {
            let tx_id = tx.id();

            // Check that latest anchoring transaction isn't a transition.
            if actual_state.is_transition() {
                let current_script_pubkey = &tx.0.output[0].script_pubkey;
                let outgoing_script_pubkey = &actual_state.script_pubkey();
                if current_script_pubkey == outgoing_script_pubkey {
                    trace!(
                        "Waiting for the moment when the following configuration \
                         becomes actual."
                    );
                    return None;
                } else {
                    trace!(
                        "Transition from {} to {}.",
                        actual_state.actual_config().anchoring_address(),
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

        // Add corresponding payload.
        let latest_anchored_height = self.latest_anchored_height();
        let anchoring_height = actual_state.following_anchoring_height(latest_anchored_height);

        let anchoring_block_hash =
            Schema::new(self.access.clone()).block_hash_by_height(anchoring_height)?;

        builder.payload(anchoring_height, anchoring_block_hash);
        builder.fee(config.transaction_fee);

        // Create anchoring proposal.
        Some(builder.create())
    }

    /// Returns the proposal of the next anchoring transaction for the actual anchoring state.
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
                .expect(
                    "Expected payload in the anchoring transaction. \
                     If this error occurs, inform the service authors about it.",
                )
                .1
                .block_height,
        )
    }

    /// Adds a finalized transaction to the tail of the anchoring transactions.
    pub fn push_anchoring_transaction(&self, tx: Transaction) {
        // An unspent funding transaction is always unconditionally added to the anchoring
        // transaction proposal, so we can simply move it to the list of spent.
        if let Some(funding_transaction) = self.unspent_funding_transaction_entry().take() {
            self.spent_funding_transactions()
                .put(&funding_transaction.id(), funding_transaction);
        }
        // Special case if we have an active following configuration.
        if let Some(config) = self.following_config() {
            // Check that the anchoring transaction is correct.
            let tx_out_script = tx
                .anchoring_metadata()
                .expect(
                    "Unable to find metadata in the anchoring transaction. \
                     If this error occurs, inform the service authors about it.",
                )
                .0;
            // If there is a following config, then the anchoring transaction's output should have
            // same script as in the following config.
            // Otherwise, this is a critical error in the logic of the anchoring.
            assert_eq!(
                config.anchoring_out_script(),
                *tx_out_script,
                "Malformed output address in the anchoring transaction. \
                 If this error occurs, inform the service authors about it."
            );
            // If preconditions are correct, just reassign the config as an actual.
            self.following_config_entry().remove();
            self.set_actual_config(config);
        }
        self.anchoring_transactions_chain().push(tx);
    }

    /// Sets a new anchoring configuration parameters.
    pub fn set_actual_config(&self, config: Config) {
        // TODO remove this special case. [ECR-3603]
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
