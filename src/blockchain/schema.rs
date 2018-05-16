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

use exonum::blockchain::Schema;
use exonum::crypto::{Hash, PublicKey};
use exonum::storage::{Fork, ProofListIndex, ProofMapIndex, Snapshot};

use super::data_layout::*;
use btc::Transaction;

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
pub struct AnchoringSchema<T> {
    snapshot: T,
}

impl<T: AsRef<Snapshot>> AnchoringSchema<T> {
    /// Constructs schema for the given database `snapshot`.
    pub fn new(snapshot: T) -> Self {
        AnchoringSchema { snapshot }
    }

    pub fn anchoring_transactions_chain(&self) -> ProofListIndex<&T, Transaction> {
        ProofListIndex::new(TRANSACTIONS_CHAIN, &self.snapshot)
    }

    pub fn spent_funding_transactions(&self) -> ProofMapIndex<&T, Hash, Transaction> {
        ProofMapIndex::new(SPENT_FUNDING_TRANSACTIONS, &self.snapshot)
    }

    pub fn transaction_signatures(
        &self,
        validator: &PublicKey,
    ) -> ProofMapIndex<&T, TxInputId, InputSignatures> {
        ProofMapIndex::new_in_family(TRANSACTION_SIGNATURES, validator, &self.snapshot)
    }

    /// Returns hashes of the stored tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        let mut table_hashes = vec![
            self.anchoring_transactions_chain().merkle_root(),
            self.spent_funding_transactions().merkle_root(),
        ];

        let transaction_signatures = Schema::new(&self.snapshot)
            .actual_configuration()
            .validator_keys
            .into_iter()
            .map(|keys| self.transaction_signatures(&keys.service_key).merkle_root());
        table_hashes.extend(transaction_signatures);

        table_hashes
    }
}

impl<'a> AnchoringSchema<&'a mut Fork> {
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
        validator: &PublicKey,
    ) -> ProofMapIndex<&mut Fork, TxInputId, InputSignatures> {
        ProofMapIndex::new_in_family(TRANSACTION_SIGNATURES, validator, &mut self.snapshot)
    }
}
