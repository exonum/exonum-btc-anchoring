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

use exonum::helpers::Height;

use bitcoin::network::constants::Network;
use btc_transaction_utils::multisig::{RedeemScript, RedeemScriptBuilder, RedeemScriptError};
use btc_transaction_utils::p2wsh;

use std::collections::HashMap;

use crate::btc::{Address, PrivateKey, PublicKey, Transaction};
use crate::rpc::BitcoinRpcConfig;

/// Returns sufficient number of keys for the given validators number.
pub fn byzantine_quorum(total: usize) -> usize {
    ::exonum::node::state::State::byzantine_majority_count(total)
}

/// Consensus parameters in the BTC anchoring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobalConfig {
    /// Type of the used BTC network.
    pub network: Network,
    /// Bitcoin public keys of validators from from which the current anchoring redeem script can be calculated.
    pub public_keys: Vec<PublicKey>,
    /// Interval in blocks between anchored blocks.
    pub anchoring_interval: u64,
    /// Fee per byte in satoshis.
    pub transaction_fee: u64,
    /// Funding transaction.
    pub funding_transaction: Option<Transaction>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            network: Network::Testnet,
            public_keys: vec![],
            anchoring_interval: 5_000,
            transaction_fee: 10,
            funding_transaction: None,
        }
    }
}

impl GlobalConfig {
    /// Creates global configuration instance with default parameters for the
    /// given Bitcoin network and public keys of participants.
    pub fn with_public_keys(
        network: Network,
        keys: impl IntoIterator<Item = PublicKey>,
    ) -> Result<Self, RedeemScriptError> {
        let public_keys = keys.into_iter().collect::<Vec<_>>();
        if public_keys.is_empty() {
            Err(RedeemScriptError::NotEnoughPublicKeys)?;
        }

        Ok(Self {
            network,
            public_keys,
            ..Self::default()
        })
    }

    /// Returns the corresponding Bitcoin address.
    pub fn anchoring_address(&self) -> Address {
        p2wsh::address(&self.redeem_script(), self.network).into()
    }

    /// Returns the corresponding redeem script.
    pub fn redeem_script(&self) -> RedeemScript {
        let quorum = byzantine_quorum(self.public_keys.len());
        RedeemScriptBuilder::with_public_keys(self.public_keys.iter().map(|x| x.0))
            .quorum(quorum)
            .to_script()
            .unwrap()
    }

    /// Returns the latest height below the given height which must be anchored.
    pub fn previous_anchoring_height(&self, current_height: Height) -> Height {
        Height(current_height.0 - current_height.0 % self.anchoring_interval)
    }

    /// Returns the nearest height above the given height which must be anchored.
    pub fn following_anchoring_height(&self, current_height: Height) -> Height {
        Height(self.previous_anchoring_height(current_height).0 + self.anchoring_interval)
    }
}

/// Local part of anchoring service configuration stored on the local machine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalConfig {
    /// Bitcoin RPC client configuration, which used to send an anchoring transactions
    /// to the Bitcoin network.
    pub rpc: Option<BitcoinRpcConfig>,
    /// Set of private keys for each anchoring address.
    #[serde(with = "flatten_keypairs")]
    pub private_keys: HashMap<Address, PrivateKey>,
}

/// BTC anchoring configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// Public part of the configuration stored in the blockchain.
    pub global: GlobalConfig,
    /// Local part of the configuration stored on the local machine.
    pub local: LocalConfig,
}

mod flatten_keypairs {
    use crate::btc::{Address, PrivateKey};

    use std::collections::HashMap;

    /// The structure for storing the anchoring address and the private key.
    /// It is required for reading data from the .toml file into memory.
    #[derive(Deserialize, Serialize)]
    struct BitcoinKeypair {
        /// Bitcoin address.
        address: Address,
        /// Corresponding private key.
        private_key: PrivateKey,
    }

    pub fn serialize<S>(
        keys: &HashMap<Address, PrivateKey>,
        ser: S,
    ) -> ::std::result::Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        use serde::Serialize;

        let keypairs = keys
            .iter()
            .map(|(address, private_key)| BitcoinKeypair {
                address: address.clone(),
                private_key: private_key.clone(),
            })
            .collect::<Vec<_>>();
        keypairs.serialize(ser)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<Address, PrivateKey>, D::Error>
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
    use exonum::helpers::Height;

    use bitcoin::network::constants::Network;
    use btc_transaction_utils::test_data::secp_gen_keypair;

    use super::{GlobalConfig, LocalConfig};
    use crate::rpc::BitcoinRpcConfig;

    #[test]
    fn test_global_config() {
        let public_keys = (0..4)
            .map(|_| secp_gen_keypair(Network::Bitcoin).0.into())
            .collect::<Vec<_>>();

        let config = GlobalConfig::with_public_keys(Network::Bitcoin, public_keys).unwrap();
        assert_eq!(config.redeem_script().content().quorum, 3);

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

    #[test]
    fn test_global_config_anchoring_height() {
        let public_keys = (0..4)
            .map(|_| secp_gen_keypair(Network::Bitcoin).0.into())
            .collect::<Vec<_>>();

        let mut config = GlobalConfig::with_public_keys(Network::Bitcoin, public_keys).unwrap();
        config.anchoring_interval = 1000;

        assert_eq!(config.previous_anchoring_height(Height(0)), Height(0));
        assert_eq!(config.previous_anchoring_height(Height(999)), Height(0));
        assert_eq!(config.previous_anchoring_height(Height(1000)), Height(1000));
        assert_eq!(config.previous_anchoring_height(Height(1001)), Height(1000));

        assert_eq!(config.following_anchoring_height(Height(0)), Height(1000));
        assert_eq!(config.following_anchoring_height(Height(999)), Height(1000));
        assert_eq!(
            config.following_anchoring_height(Height(1000)),
            Height(2000)
        );
        assert_eq!(
            config.following_anchoring_height(Height(1001)),
            Height(2000)
        );
    }
}
