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

use exonum::blockchain::{Schema, ValidatorKeys};
use exonum::crypto::{Hash, PublicKey};
use exonum::storage::{ProofListIndex, ProofMapIndex, Snapshot};

use super::data_layout::*;

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

    pub fn anchoring_transactions_chain(&self) -> ProofListIndex<&T, Vec<u8>> {
        ProofListIndex::new(TRANSACTIONS_CHAIN, &self.snapshot)
    }

    pub fn spent_funding_transactions(&self) -> ProofMapIndex<&T, Hash, Vec<u8>> {
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
        let table_hashes = vec![
            self.anchoring_transactions_chain().merkle_root(),
            self.spent_funding_transactions().merkle_root(),
        ];
        // TODO extend by the transaction_signatures
        table_hashes
    }
}
