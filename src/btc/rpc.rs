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

use exonum::crypto::Hash;

use bitcoin::util::address::Address;
use failure::Fail;

use super::Transaction;

/// Short information about bitcoin transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct TransactionInfo {
    /// Transaction content.
    pub content: Transaction,
    /// Number of confirmations.
    pub confirmations: u64,
}

/// Information provider about the Bitcoin network.
pub trait BtcRelay: Send + Sync {
    /// Error type for a specific implementation.
    type Error: Fail;

    /// Sends funds to the given address.
    fn send_to_address(&self, addr: &Address, satoshis: u64) -> Result<Transaction, Self::Error>;
    /// Retrieves information about transaction with the given id.
    fn transaction_info(&self, id: &str) -> Result<TransactionInfo, Self::Error>;
    /// Sends raw transaction to the bitcoin network.
    fn send_transaction(&self, transaction: &Transaction) -> Result<Hash, Self::Error>;
}
