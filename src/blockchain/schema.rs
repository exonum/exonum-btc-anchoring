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
    blockchain::{Schema, StoredConfiguration},
    crypto::Hash,
    helpers::Height,
};
use exonum_merkledb::{ObjectAccess, ObjectHash, ProofListIndex, ProofMapIndex, RefMut};
use log::{error, trace};
use serde_json;

use crate::{
    btc::{BtcAnchoringTransactionBuilder, BuilderError, Transaction},
    config::GlobalConfig,
    BTC_ANCHORING_SERVICE_NAME,
};

use super::{data_layout::*, BtcAnchoringState};

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
    access: T,
}

impl<T: ObjectAccess> BtcAnchoringSchema<T> {
    /// Constructs schema for the given database `snapshot`.
    pub fn new(access: T) -> Self {
        Self { access }
    }

    /// Returns table that contains complete chain of the anchoring transactions.
    pub fn anchoring_transactions_chain(&self) -> RefMut<ProofListIndex<T, Transaction>> {
        self.access.get_object(TRANSACTIONS_CHAIN)
    }

    /// Returns the table that contains already spent funding transactions.
    pub fn spent_funding_transactions(&self) -> RefMut<ProofMapIndex<T, Hash, Transaction>> {
        self.access.get_object(SPENT_FUNDING_TRANSACTIONS)
    }

    /// Returns the table that contains signatures for the given transaction input.
    pub fn transaction_signatures(&self) -> RefMut<ProofMapIndex<T, TxInputId, InputSignatures>> {
        self.access.get_object(TRANSACTION_SIGNATURES)
    }

    /// Returns a list of hashes of Exonum blocks headers.
    pub fn anchored_blocks(&self) -> RefMut<ProofListIndex<T, Hash>> {
        self.access.get_object(ANCHORED_BLOCKS)
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
    pub fn actual_configuration(&self) -> GlobalConfig {
        let actual_configuration = Schema::new(self.access.clone()).actual_configuration();
        Self::parse_config(&actual_configuration)
            .expect("Actual BTC anchoring configuration is absent")
    }

    /// Returns the nearest following configuration if it exists.
    pub fn following_configuration(&self) -> Option<GlobalConfig> {
        let following_configuration = Schema::new(self.access.clone()).following_configuration()?;
        Self::parse_config(&following_configuration)
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
        let unspent_funding_transaction = self.unspent_funding_transaction();

        let mut builder = BtcAnchoringTransactionBuilder::new(&config.redeem_script());
        // First anchoring transaction doesn't have previous.
        if let Some(tx) = unspent_anchoring_transaction {
            let tx_id = tx.id();

            // Checks that latest anchoring transaction isn't a transition.
            if actual_state.is_transition() {
                let current_script_pubkey = &tx.0.output[0].script_pubkey;
                let outgoing_script_pubkey = &actual_state.script_pubkey();
                if current_script_pubkey == outgoing_script_pubkey {
                    trace!("Awaiting for new configuration to become actual.");
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
