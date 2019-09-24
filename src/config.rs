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

//! BTC anchoring configuration data types.

pub use crate::proto::Config as GlobalConfig;

use bitcoin::network::constants::Network;
use btc_transaction_utils::{
    multisig::{RedeemScript, RedeemScriptBuilder, RedeemScriptError},
    p2wsh,
};
use exonum::{crypto::PublicKey, helpers::Height};
use serde_derive::{Deserialize, Serialize};

use std::collections::HashMap;

use crate::{
    btc::{self, Address, PrivateKey},
    proto::AnchoringKeys,
    rpc::BitcoinRpcConfig,
};

/// Returns sufficient number of keys for the given validators number.
pub fn byzantine_quorum(total: usize) -> usize {
    exonum::node::state::State::byzantine_majority_count(total)
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            network: Network::Testnet,
            anchoring_keys: vec![],
            anchoring_interval: 5_000,
            transaction_fee: 10,
            funding_transaction: None,
        }
    }
}

// TODO implement ValidateInput.

impl GlobalConfig {
    /// Creates global configuration instance with default parameters for the
    /// given Bitcoin network and public keys of participants.
    pub fn with_public_keys(
        network: Network,
        keys: impl IntoIterator<Item = AnchoringKeys>,
    ) -> Result<Self, RedeemScriptError> {
        let anchoring_keys = keys.into_iter().collect::<Vec<_>>();
        if anchoring_keys.is_empty() {
            Err(RedeemScriptError::NotEnoughPublicKeys)?;
        }

        Ok(Self {
            network,
            anchoring_keys,
            ..Self::default()
        })
    }

    /// Try to find bitcoin public key corresponding with the given service key.
    pub fn find_bitcoin_key(&self, service_key: &PublicKey) -> Option<(usize, btc::PublicKey)> {
        self.anchoring_keys.iter().enumerate().find_map(|(n, x)| {
            if &x.service_key == service_key {
                Some((n, x.bitcoin_key))
            } else {
                None
            }
        })
    }

    /// Returns the corresponding Bitcoin address.
    pub fn anchoring_address(&self) -> Address {
        p2wsh::address(&self.redeem_script(), self.network).into()
    }

    /// Returns the corresponding redeem script.
    pub fn redeem_script(&self) -> RedeemScript {
        let quorum = byzantine_quorum(self.anchoring_keys.len());
        RedeemScriptBuilder::with_public_keys(self.anchoring_keys.iter().map(|x| x.bitcoin_key.0))
            .quorum(quorum)
            .to_script()
            .unwrap()
    }

    /// Compute the P2WSH output corresponding to the actual redeem script.
    pub fn anchoring_out_script(&self) -> bitcoin::Script {
        self.redeem_script().as_ref().to_v0_p2wsh()
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
    pub private_keys: HashMap<btc::PublicKey, PrivateKey>,
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
    use crate::btc::{PrivateKey, PublicKey};

    use serde_derive::{Deserialize, Serialize};

    use std::collections::HashMap;

    /// The structure for storing the bitcoin keypair.
    /// It is required for reading data from the .toml file into memory.
    #[derive(Deserialize, Serialize)]
    struct BitcoinKeypair {
        /// Bitcoin public key.
        public_key: PublicKey,
        /// Corresponding private key.
        private_key: PrivateKey,
    }

    pub fn serialize<S>(
        keys: &HashMap<PublicKey, PrivateKey>,
        ser: S,
    ) -> ::std::result::Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        use serde::Serialize;

        let keypairs = keys
            .iter()
            .map(|(&public_key, private_key)| BitcoinKeypair {
                public_key,
                private_key: private_key.clone(),
            })
            .collect::<Vec<_>>();
        keypairs.serialize(ser)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<PublicKey, PrivateKey>, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        use serde::Deserialize;
        Vec::<BitcoinKeypair>::deserialize(deserializer).map(|keypairs| {
            keypairs
                .into_iter()
                .map(|keypair| (keypair.public_key, keypair.private_key))
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use exonum::{crypto, helpers::Height};

    use bitcoin::network::constants::Network;
    use btc_transaction_utils::test_data::secp_gen_keypair;

    use super::{GlobalConfig, LocalConfig};
    use crate::{proto::AnchoringKeys, rpc::BitcoinRpcConfig};

    #[test]
    fn test_global_config() {
        let public_keys = (0..4)
            .map(|_| AnchoringKeys {
                bitcoin_key: secp_gen_keypair(Network::Bitcoin).0.into(),
                service_key: crypto::gen_keypair().0,
            })
            .collect::<Vec<_>>();

        let config = GlobalConfig::with_public_keys(Network::Bitcoin, public_keys).unwrap();
        assert_eq!(config.redeem_script().content().quorum, 3);

        let json = serde_json::to_value(&config).unwrap();
        let config2: GlobalConfig = ::serde_json::from_value(json).unwrap();
        assert_eq!(config2, config);
    }

    #[test]
    fn test_local_config() {
        let cfg_str = r#"
            [rpc]
            host = "http://localhost"
            [[private_keys]]
            public_key = '03c1e6b6c221b4794df136c26def65b084455bbd2f3b3b80f8aae8629acbdf5cde'
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
            .map(|_| AnchoringKeys {
                bitcoin_key: secp_gen_keypair(Network::Bitcoin).0.into(),
                service_key: crypto::gen_keypair().0,
            })
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
