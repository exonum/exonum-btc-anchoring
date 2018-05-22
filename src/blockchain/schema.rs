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

use exonum::blockchain::{Schema, StoredConfiguration};
use exonum::crypto::Hash;
use exonum::storage::{Fork, ProofListIndex, ProofMapIndex, Snapshot};

use btc_transaction_utils::multisig::RedeemScript;
use serde_json;

use btc::Transaction;
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
);

/// Information schema for `exonum-btc-anchoring`.
#[derive(Debug)]
pub struct BtcAnchoringSchema<T> {
    snapshot: T,
}

impl<T: AsRef<Snapshot>> BtcAnchoringSchema<T> {
    /// Constructs schema for the given database `snapshot`.
    pub fn new(snapshot: T) -> Self {
        BtcAnchoringSchema { snapshot }
    }

    pub fn anchoring_transactions_chain(&self) -> ProofListIndex<&T, Transaction> {
        ProofListIndex::new(TRANSACTIONS_CHAIN, &self.snapshot)
    }

    pub fn spent_funding_transactions(&self) -> ProofMapIndex<&T, Hash, Transaction> {
        ProofMapIndex::new(SPENT_FUNDING_TRANSACTIONS, &self.snapshot)
    }

    pub fn transaction_signatures(&self) -> ProofMapIndex<&T, TxInputId, InputSignatures> {
        ProofMapIndex::new(TRANSACTION_SIGNATURES, &self.snapshot)
    }

    /// Returns hashes of the stored tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.anchoring_transactions_chain().merkle_root(),
            self.spent_funding_transactions().merkle_root(),
            self.transaction_signatures().merkle_root(),
        ]
    }

    /// Returns the actual anchoring configuration.
    pub fn actual_configuration(&self) -> GlobalConfig {
        let actual_configuration = Schema::new(&self.snapshot).actual_configuration();
        Self::parse_config(actual_configuration)
            .expect("Actual BTC anchoring configuration is absent")
    }

    /// Returns the nearest following configuration if it exists.
    pub fn following_configuration(&self) -> Option<GlobalConfig> {
        let following_configuration = Schema::new(&self.snapshot).following_configuration()?;
        Self::parse_config(following_configuration)
    }

    pub fn actual_state(&self) -> BtcAnchoringState {
        let actual_configuration = self.actual_configuration();
        if let Some(following_configuration) = self.following_configuration() {
            BtcAnchoringState::Transition {
                actual_configuration,
                following_configuration,
            }
        } else {
            BtcAnchoringState::Regular {
                actual_configuration,
            }
        }
    }

    pub fn unspent_funding_transaction(&self) -> Option<Transaction> {
        let tx_candidate = self.actual_configuration().funding_transaction?;
        let txid = tx_candidate.id();
        if self.spent_funding_transactions().contains(&txid) {
            None
        } else {
            Some(tx_candidate)
        }
    }

    pub fn expected_input_transactions(&self) -> Vec<Transaction> {
        let unspent_anchoring_tx = self.anchoring_transactions_chain().last();
        let unspent_funding_tx = self.unspent_funding_transaction();

        [unspent_anchoring_tx, unspent_funding_tx]
            .into_iter()
            .filter_map(|x| x.clone())
            .collect::<Vec<_>>()
    }

    pub fn input_signatures(
        &self,
        input: &TxInputId,
        redeem_script: &RedeemScript,
    ) -> InputSignatures {
        self.transaction_signatures().get(input).unwrap_or_else(|| {
            InputSignatures::new(redeem_script.content().public_keys.len() as u16)
        })
    }

    fn parse_config(configuration: StoredConfiguration) -> Option<GlobalConfig> {
        configuration
            .services
            .get(BTC_ANCHORING_SERVICE_NAME)
            .cloned()
            .map(|value| serde_json::from_value(value).expect("Unable to parse configuration"))
    }
}

impl<'a> BtcAnchoringSchema<&'a mut Fork> {
    pub fn anchoring_transactions_chain_mut(&mut self) -> ProofListIndex<&mut Fork, Transaction> {
        ProofListIndex::new(TRANSACTIONS_CHAIN, &mut self.snapshot)
    }

    pub fn spent_funding_transactions_mut(
        &mut self,
    ) -> ProofMapIndex<&mut Fork, Hash, Transaction> {
        ProofMapIndex::new(SPENT_FUNDING_TRANSACTIONS, &mut self.snapshot)
    }

    pub fn transaction_signatures_mut(
        &mut self,
    ) -> ProofMapIndex<&mut Fork, TxInputId, InputSignatures> {
        ProofMapIndex::new(TRANSACTION_SIGNATURES, &mut self.snapshot)
    }
}
