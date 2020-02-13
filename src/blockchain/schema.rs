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

use exonum::{blockchain::Schema as CoreSchema, helpers::Height};
use exonum_derive::FromAccess;
use exonum_merkledb::{
    access::{Access, FromAccess, Prefixed},
    Entry, Fork, ProofListIndex, ProofMapIndex,
};
use log::{error, trace};

use crate::{
    btc::{self, BtcAnchoringTransactionBuilder, BuilderError, Sha256d, Transaction},
    config::Config,
    proto::BinaryMap,
};

use super::{data_layout::*, BtcAnchoringState};

/// A set of signatures for a transaction input ordered by the anchoring node identifiers.
pub type InputSignatures = BinaryMap<u16, btc::InputSignature>;
/// A set of funding transaction confirmations.
pub type TransactionConfirmations = BinaryMap<btc::PublicKey, ()>;

/// Information schema for `exonum-btc-anchoring`.
#[derive(Debug, FromAccess)]
pub struct Schema<T: Access> {
    /// Complete chain of the anchoring transactions.
    pub transactions_chain: ProofListIndex<T::Base, Transaction>,
    /// Already spent funding transactions.
    pub(crate) spent_funding_transactions: ProofMapIndex<T::Base, Sha256d, Transaction>,
    /// Signatures for the given transaction input.
    pub(crate) transaction_signatures: ProofMapIndex<T::Base, TxInputId, InputSignatures>,
    /// Actual anchoring configuration entry.
    pub(crate) actual_config: Entry<T::Base, Config>,
    /// Following anchoring configuration entry.
    pub(crate) following_config: Entry<T::Base, Config>,
    /// Confirmations for the corresponding funding transaction.
    pub(crate) unconfirmed_funding_transactions:
        ProofMapIndex<T::Base, Sha256d, TransactionConfirmations>,
    /// Entry that may contain an unspent funding transaction for the
    /// actual configuration.
    pub(crate) unspent_funding_transaction: Entry<T::Base, Transaction>,
}

impl<T: Access> Schema<T> {
    /// Returns a new schema instance.
    pub fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }

    /// Returns an actual anchoring configuration.
    pub fn actual_config(&self) -> Config {
        self.actual_config.get().expect(
            "Actual configuration of anchoring is absent. \
             If this error occurs, inform the service authors about it.",
        )
    }

    /// Returns the nearest following configuration if it exists.
    pub fn following_config(&self) -> Option<Config> {
        self.following_config.get()
    }

    /// Returns the list of signatures for the given transaction input.
    pub fn input_signatures(&self, input: &TxInputId) -> InputSignatures {
        self.transaction_signatures.get(input).unwrap_or_default()
    }

    /// Returns an unspent funding transaction for the actual configurations if it exists.
    pub fn unspent_funding_transaction(&self) -> Option<Transaction> {
        self.unspent_funding_transaction.get()
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
        core_schema: CoreSchema<impl Access>,
        actual_state: &BtcAnchoringState,
    ) -> Option<Result<(Transaction, Vec<Transaction>), BuilderError>> {
        let config = actual_state.actual_config();
        let unspent_anchoring_transaction = self.transactions_chain.last();
        let unspent_funding_transaction = self.unspent_funding_transaction.get();

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
        let anchoring_block_hash = core_schema.block_hash_by_height(anchoring_height)?;

        builder.payload(anchoring_height, anchoring_block_hash);
        builder.fee(config.transaction_fee);

        // Create anchoring proposal.
        Some(builder.create())
    }

    /// Returns the proposal of the next anchoring transaction for the actual anchoring state.
    pub fn actual_proposed_anchoring_transaction(
        &self,
        core_schema: CoreSchema<impl Access>,
    ) -> Option<Result<(Transaction, Vec<Transaction>), BuilderError>> {
        let actual_state = self.actual_state();
        self.proposed_anchoring_transaction(core_schema, &actual_state)
    }

    /// Returns the height of the latest anchored block.
    pub fn latest_anchored_height(&self) -> Option<Height> {
        let tx = self.transactions_chain.last()?;
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
}

impl Schema<Prefixed<&Fork>> {
    /// Adds a finalized transaction to the tail of the anchoring transactions.
    pub(crate) fn push_anchoring_transaction(&mut self, tx: Transaction) {
        // An unspent funding transaction is always unconditionally added to the anchoring
        // transaction proposal, so we can simply move it to the list of spent.
        if let Some(funding_transaction) = self.unspent_funding_transaction.take() {
            self.spent_funding_transactions
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
            self.following_config.remove();
            self.actual_config.set(config);
        }
        self.transactions_chain.push(tx);
    }

    /// Sets the given transaction as the current unspent funding transaction.
    pub(crate) fn set_funding_transaction(&mut self, transaction: btc::Transaction) {
        debug_assert!(
            !self.spent_funding_transactions.contains(&transaction.id()),
            "Funding transaction must be unspent."
        );
        // Remove confirmations for this transaction to avoid attack of re-setting
        // this transaction as funding.
        self.unconfirmed_funding_transactions
            .put(&transaction.id(), TransactionConfirmations::default());
        self.unspent_funding_transaction.set(transaction);
    }
}
