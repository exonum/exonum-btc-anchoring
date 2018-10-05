// Copyright 2018 The Exonum Team
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

use exonum::blockchain::{Schema, StoredConfiguration};
use exonum::crypto::Hash;
use exonum::helpers::Height;
use exonum::storage::{Fork, ProofListIndex, ProofMapIndex, Snapshot};

use btc_transaction_utils::multisig::RedeemScript;
use serde_json;

use btc::{BtcAnchoringTransactionBuilder, BuilderError, Transaction};
use config::GlobalConfig;
use BTC_ANCHORING_SERVICE_NAME;

use super::data_layout::*;
use super::BtcAnchoringState;

/// Defines `&str` constants with given name and value.
macro_rules! define_names {
    (
        $(
            $name:ident => $value:expr;
        )+
    ) => (
        $(const $name: &str = concat!("btc_anchoring.", $value);)*
    )
}

define_names!(
    TRANSACTIONS_CHAIN => "transactions_chain";
    TRANSACTION_SIGNATURES => "transaction_signatures";
    SPENT_FUNDING_TRANSACTIONS => "spent_funding_transactions";
    ANCHORED_BLOCKS => "anchored_blocks";
);

/// Information schema for `exonum-btc-anchoring`.
#[derive(Debug)]
pub struct BtcAnchoringSchema<T> {
    snapshot: T,
}

impl<T: AsRef<dyn Snapshot>> BtcAnchoringSchema<T> {
    /// Constructs schema for the given database `snapshot`.
    pub fn new(snapshot: T) -> Self {
        BtcAnchoringSchema { snapshot }
    }

    /// Returns table that contains complete chain of the anchoring transactions.
    pub fn anchoring_transactions_chain(&self) -> ProofListIndex<&T, Transaction> {
        ProofListIndex::new(TRANSACTIONS_CHAIN, &self.snapshot)
    }

    /// Returns the table that contains already spent funding transactions.
    pub fn spent_funding_transactions(&self) -> ProofMapIndex<&T, Hash, Transaction> {
        ProofMapIndex::new(SPENT_FUNDING_TRANSACTIONS, &self.snapshot)
    }

    /// Returns the table that contains signatures for the given transaction input.
    pub fn transaction_signatures(&self) -> ProofMapIndex<&T, TxInputId, InputSignatures> {
        ProofMapIndex::new(TRANSACTION_SIGNATURES, &self.snapshot)
    }

    /// Returns a list of hashes of Exonum blocks headers.
    pub fn anchored_blocks(&self) -> ProofListIndex<&T, Hash> {
        ProofListIndex::new(ANCHORED_BLOCKS, &self.snapshot)
    }

    /// Returns hashes of the stored tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.anchoring_transactions_chain().merkle_root(),
            self.spent_funding_transactions().merkle_root(),
            self.transaction_signatures().merkle_root(),
            self.anchored_blocks().merkle_root(),
        ]
    }

    /// Returns the actual anchoring configuration.
    pub fn actual_configuration(&self) -> GlobalConfig {
        let actual_configuration = Schema::new(&self.snapshot).actual_configuration();
        Self::parse_config(&actual_configuration)
            .expect("Actual BTC anchoring configuration is absent")
    }

    /// Returns the nearest following configuration if it exists.
    pub fn following_configuration(&self) -> Option<GlobalConfig> {
        let following_configuration = Schema::new(&self.snapshot).following_configuration()?;
        Self::parse_config(&following_configuration)
    }

    /// Returns the list of signatures for the given transaction input.
    pub fn input_signatures(
        &self,
        input: &TxInputId,
        redeem_script: &RedeemScript,
    ) -> InputSignatures {
        self.transaction_signatures().get(input).unwrap_or_else(|| {
            InputSignatures::new(redeem_script.content().public_keys.len() as u16)
        })
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
        let unspent_funding_transaction = self.unspent_funding_transaction();

        let mut builder = BtcAnchoringTransactionBuilder::new(&config.redeem_script());
        // First anchoring transaction doesn't have previous.
        if let Some(tx) = unspent_anchoring_transaction {
            let tx_id = tx.id();

            // Checks that latest anchoring transaction isn't a transition.
            if actual_state.is_transition() {
                let current_address = &tx.0.output[0].script_pubkey;
                let outgoing_address = &actual_state.script_pubkey();
                if current_address == outgoing_address {
                    trace!("Awaiting for new configuration to become actual.");
                    return None;
                } else {
                    trace!(
                        "Transition from {} to {}.",
                        current_address,
                        outgoing_address
                    );
                    builder.transit_to(actual_state.script_pubkey());
                }
            }

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

        let anchoring_block_hash = Schema::new(&self.snapshot)
            .block_hash_by_height(anchoring_height)
            .unwrap();

        builder.payload(anchoring_height, anchoring_block_hash);
        builder.fee(config.transaction_fee);

        // Creates anchoring proposal
        Some(builder.create())
    }

    /// Returns the proposal of next anchoring transaction for the actual anchoring state.
    pub fn actual_proposed_anchoring_transaction(
        &self,
    ) -> Option<Result<(Transaction, Vec<Transaction>), BuilderError>> {
        let actual_state = self.actual_state();
        self.proposed_anchoring_transaction(&actual_state)
    }

    /// Returns the unspent funding transaction if it is exist.
    pub fn unspent_funding_transaction(&self) -> Option<Transaction> {
        let tx_candidate = self.actual_configuration().funding_transaction?;
        let txid = tx_candidate.id();
        if self.spent_funding_transactions().contains(&txid) {
            None
        } else {
            Some(tx_candidate)
        }
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

    fn parse_config(configuration: &StoredConfiguration) -> Option<GlobalConfig> {
        configuration
            .services
            .get(BTC_ANCHORING_SERVICE_NAME)
            .cloned()
            .map(|value| serde_json::from_value(value).expect("Unable to parse configuration"))
    }
}

impl<'a> BtcAnchoringSchema<&'a mut Fork> {
    /// Mutable variant of the [`anchoring_transactions_chain`][1] index.
    ///
    /// [1]: struct.AnchoringSchema.html#method.anchoring_transactions_chain_mut
    pub fn anchoring_transactions_chain_mut(&mut self) -> ProofListIndex<&mut Fork, Transaction> {
        ProofListIndex::new(TRANSACTIONS_CHAIN, &mut self.snapshot)
    }

    /// Mutable variant of the [`spent_funding_transactions`][1] index.
    ///
    /// [1]: struct.AnchoringSchema.html#method.spent_funding_transactions
    pub fn spent_funding_transactions_mut(
        &mut self,
    ) -> ProofMapIndex<&mut Fork, Hash, Transaction> {
        ProofMapIndex::new(SPENT_FUNDING_TRANSACTIONS, &mut self.snapshot)
    }

    /// Mutable variant of the [`anchored_blocks`][1] index.
    ///
    /// [1]: struct.AnchoringSchema.html#method.anchored_blocks
    pub fn transaction_signatures_mut(
        &mut self,
    ) -> ProofMapIndex<&mut Fork, TxInputId, InputSignatures> {
        ProofMapIndex::new(TRANSACTION_SIGNATURES, &mut self.snapshot)
    }

    /// Mutable variant of the [`anchored_blocks`][1] index.
    ///
    /// [1]: struct.AnchoringSchema.html#method.anchored_blocks
    pub fn anchored_blocks_mut(&mut self) -> ProofListIndex<&mut Fork, Hash> {
        ProofListIndex::new("btc_anchoring.anchored_blocks", &mut self.snapshot)
    }
}
