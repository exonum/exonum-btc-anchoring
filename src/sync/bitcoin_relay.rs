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

use crate::btc;

/// Describes communication with the Bitcoin network node.
pub trait BitcoinRelay {
    /// Error type for the current Bitcoin relay implementation.
    type Error;
    /// Sends a raw transaction to the Bitcoin network node.
    fn send_transaction(&self, transaction: &btc::Transaction)
        -> Result<btc::Sha256d, Self::Error>;
    /// Gets the number of transaction confirmations with the specified identifier.
    fn transaction_confirmations(&self, id: btc::Sha256d) -> Result<Option<u32>, Self::Error>;
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

    fn transaction_confirmations(&self, id: btc::Sha256d) -> Result<Option<u32>, Self::Error> {
        let result = match self.get_raw_transaction_verbose(id.as_ref(), None) {
            Ok(result) => result,
            // TODO Write more graceful error handling. [ECR-3222]
            Err(bitcoincore_rpc::Error::JsonRpc(_)) => return Ok(None),
            Err(e) => return Err(e),
        };

        Ok(result.confirmations)
    }
}
