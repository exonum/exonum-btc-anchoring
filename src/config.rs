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

//! BTC anchoring configuration data types.

use bitcoin::network::constants::Network;

use btc::{PublicKey, Transaction};

/// Consensus parameters in the BTC anchoring.
#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Type of the used BTC network.
    #[serde(with = "NetworkRef")]
    pub network: Network,
    /// Interval in blocks between anchored blocks.
    pub anchoring_interval: u64,
    /// Bitcoin public keys of validators.
    pub validator_keys: Vec<PublicKey>,
    /// Funding transaction.
    pub funding_transaction: Option<Transaction>, 
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Network")]
#[serde(rename_all = "snake_case")]
enum NetworkRef {
    Bitcoin,
    Testnet,
}

#[cfg(test)]
mod tests {
    use super::GlobalConfig;
}