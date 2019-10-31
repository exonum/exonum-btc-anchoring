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

//! Collections of helpers for synchronization with the Bitcoin network.

use bitcoincore_rpc::RpcApi;
use jsonrpc::Error as JsonRpcError;

use crate::btc;

/// Status of the transaction in the Bitcoin network.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TransactionStatus {
    /// Transaction is unknown in the Bitcoin network.
    Unknown,
    /// The transaction is not committed, but presented in the Bitcoin node memory pool.
    Mempool,
    /// The transaction was completed to the Bitcoin blockchain with the specified number
    /// of confirmations.
    Committed(u32),
}

impl TransactionStatus {
    /// Checks that this transaction is unknown in the Bitcoin network.
    pub fn is_unknown(self) -> bool {
        if let TransactionStatus::Unknown = self {
            true
        } else {
            false
        }
    }

    /// Returns number of transaction confirmations in Bitcoin blockchain.
    pub fn confirmations(self) -> Option<u32> {
        if let TransactionStatus::Committed(confirmations) = self {
            Some(confirmations)
        } else {
            None
        }
    }
}

/// Describes communication with the Bitcoin network node.
pub trait BitcoinRelay {
    /// Error type for the current Bitcoin relay implementation.
    type Error;
    /// Sends a raw transaction to the Bitcoin network node.
    fn send_transaction(&self, transaction: &btc::Transaction)
        -> Result<btc::Sha256d, Self::Error>;
    /// Gets status for the transaction with the specified identifier.
    fn transaction_status(&self, id: btc::Sha256d) -> Result<TransactionStatus, Self::Error>;
}

impl BitcoinRelay for bitcoincore_rpc::Client {
    type Error = bitcoincore_rpc::Error;

    fn send_transaction(
        &self,
        transaction: &btc::Transaction,
    ) -> Result<btc::Sha256d, Self::Error> {
        self.send_raw_transaction(transaction.to_string())
            .map(btc::Sha256d)
    }

    fn transaction_status(&self, id: btc::Sha256d) -> Result<TransactionStatus, Self::Error> {
        match self.get_raw_transaction_verbose(id.as_ref(), None) {
            Ok(info) => {
                let status = match info.confirmations {
                    None => TransactionStatus::Mempool,
                    Some(num) => TransactionStatus::Committed(num),
                };
                Ok(status)
            }
            // TODO Write more graceful error handling. [ECR-3222]
            Err(bitcoincore_rpc::Error::JsonRpc(JsonRpcError::Rpc(_))) => {
                Ok(TransactionStatus::Unknown)
            }
            Err(e) => Err(e),
        }
    }
}
