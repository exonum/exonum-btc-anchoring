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
use btc_transaction_utils::multisig::{RedeemScript, RedeemScriptBuilder, RedeemScriptError};
use btc_transaction_utils::p2wsh;

use std::collections::HashMap;

use btc::{Address, Privkey, PublicKey, Transaction};
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

#[derive(Serialize, Deserialize)]
#[serde(remote = "Network")]
#[serde(rename_all = "snake_case")]
enum NetworkRef {
    Bitcoin,
    Testnet,
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
        p2wsh::address(&self.redeem_script, self.network).into()
    }
}

/// Local part of anchoring service configuration stored on a local machine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalConfig {
    /// Rpc configuration. Must exist if node is validator.
    /// Otherwise node can only check `lect` payload without any checks with `bitcoind`.
    pub rpc: Option<BitcoinRpcConfig>,
    /// Set of private keys for each anchoring address.
    #[serde(with = "flatten_keypairs")]
    pub private_keys: HashMap<Address, Privkey>,
}

mod flatten_keypairs {
    use btc::{Address, Privkey};

    use std::collections::HashMap;

    /// The structure for storing the anchoring address and private key. The structure is needed to
    /// convert data from the toml-file into memory.
    #[derive(Deserialize, Serialize)]
    struct BitcoinKeypair {
        /// Bitcoin address.
        address: Address,
        /// Corresponding private key.
        private_key: Privkey,
    }

    pub fn serialize<S>(
        keys: &HashMap<Address, Privkey>,
        ser: S,
    ) -> ::std::result::Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        use serde::Serialize;

        let keypairs = keys.iter()
            .map(|(address, private_key)| BitcoinKeypair {
                address: address.clone(),
                private_key: private_key.clone(),
            })
            .collect::<Vec<_>>();
        keypairs.serialize(ser)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<Address, Privkey>, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        use serde::Deserialize;
        Vec::<BitcoinKeypair>::deserialize(deserializer).map(|keypairs| {
            keypairs
                .into_iter()
                .map(|keypair| (keypair.address, keypair.private_key))
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::network::constants::Network;
    use btc_transaction_utils::test_data::secp_gen_keypair;

    use super::{GlobalConfig, LocalConfig};
    use rpc::BitcoinRpcConfig;

    #[test]
    fn test_global_config() {
        let public_keys = (0..4)
            .map(|_| secp_gen_keypair().0.into())
            .collect::<Vec<_>>();

        let config = GlobalConfig::new(Network::Bitcoin, public_keys).unwrap();
        assert_eq!(config.redeem_script.content().quorum, 3);

        let json = ::serde_json::to_value(&config).unwrap();
        let config2: GlobalConfig = ::serde_json::from_value(json).unwrap();
        assert_eq!(config2, config);
    }

    #[test]
    fn test_local_config() {
        let cfg_str = r#"
            [rpc]
            host = "http://localhost"
            [[private_keys]]
            address = 'bc1qxfhtyn4l3hztytwvd4h6l9ah8qgz3ycfa86mq85qnqdff5kdzg2sdv6e82'
            private_key = 'L58cq7TgbA6RpJ1KGsj9h5sfXuAeY6GqA197Qrpepw3boRdXqYBS'
        "#;

        let local_config: LocalConfig = ::toml::from_str(cfg_str).unwrap();
        assert_eq!(
            local_config.rpc.unwrap(),
            BitcoinRpcConfig {
                host: String::from("http://localhost"),
                username: None,
                password: None,
            }
        );
        assert!(local_config.private_keys.len() == 1);
    }
}
