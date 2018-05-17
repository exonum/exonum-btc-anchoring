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
use bitcoin::util::address::Address;
use btc_transaction_utils::multisig::{RedeemScript, RedeemScriptBuilder, RedeemScriptError};
use btc_transaction_utils::p2wsh;

use btc::{PublicKey, Transaction};
use rpc::BitcoinRpcConfig;

/// Returns sufficient number of keys for the given validators number.
pub fn byzantine_quorum(total: usize) -> usize {
    ::exonum::node::state::State::byzantine_majority_count(total)
}

/// Consensus parameters in the BTC anchoring.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GlobalConfig {
    /// Type of the used BTC network.
    #[serde(with = "NetworkRef")]
    pub network: Network,
    /// Redeem script for actual configuration.
    pub redeem_script: RedeemScript,
    /// Interval in blocks between anchored blocks.
    pub anchoring_interval: u64,
    /// Fee per byte in satoshi.
    pub transaction_fee: u64,
    /// Funding transaction.
    pub funding_transaction: Option<Transaction>,
}

impl GlobalConfig {
    pub fn new(
        network: Network,
        keys: impl IntoIterator<Item = PublicKey>,
    ) -> Result<GlobalConfig, RedeemScriptError> {
        // TODO implement blank constructor.
        let mut builder = RedeemScriptBuilder::with_quorum(0);
        // Collects keys and computes total count.
        let total = keys.into_iter().fold(0, |total, public_key| {
            builder.public_key(public_key.0);
            total + 1
        });
        // Finalizes script.
        let redeem_script = builder.quorum(byzantine_quorum(total)).to_script()?;

        Ok(GlobalConfig {
            network,
            anchoring_interval: 5_000,
            transaction_fee: 100,
            redeem_script,
            funding_transaction: None,
        })
    }

    pub fn anchoring_address(&self) -> Address {
        p2wsh::address(&self.redeem_script, self.network)
    }
}

/// Local part of anchoring service configuration stored on a local machine.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct LocalConfig {
    /// Rpc configuration. Must exist if node is validator.
    /// Otherwise node can only check `lect` payload without any checks with `bitcoind`.
    pub rpc: Option<BitcoinRpcConfig>,

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
    use bitcoin::network::constants::Network;
    use btc_transaction_utils::test_data::secp_gen_keypair;
    use serde_json;

    use super::GlobalConfig;

    #[test]
    fn test_global_config() {
        let public_keys = (0..4)
            .map(|_| secp_gen_keypair().0.into())
            .collect::<Vec<_>>();

        let config = GlobalConfig::new(Network::Bitcoin, public_keys).unwrap();
        assert_eq!(config.redeem_script.content().quorum, 3);

        let json = serde_json::to_value(&config).unwrap();
        let config2: GlobalConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config2, config);
    }
}
