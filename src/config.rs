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

pub use crate::proto::{AnchoringKeys, Config};

use bitcoin::network::constants::Network;
use btc_transaction_utils::{
    multisig::{RedeemScript, RedeemScriptBuilder, RedeemScriptError},
    p2wsh,
};
use exonum::{
    crypto::PublicKey,
    helpers::{Height, ValidateInput},
};
use failure::ensure;

use crate::btc::{self, Address};

impl Default for Config {
    fn default() -> Self {
        Self {
            network: Network::Testnet,
            anchoring_keys: vec![],
            anchoring_interval: 5_000,
            transaction_fee: 10,
        }
    }
}

impl Config {
    /// Current limit on the number of keys in a redeem script on the Bitcoin network.
    const MAX_NODES_COUNT: usize = 20;
    /// Minimal fee in satoshis for Bitcoin transaction.
    const MIN_TOTAL_TX_FEE: u64 = 1000;
    /// Minimal total transaction size in according of
    /// https://bitcoin.stackexchange.com/questions/1195/how-to-calculate-transaction-size-before-sending-legacy-non-segwit-p2pkh-p2sh     
    const MIN_TX_LEN: u64 = 10 + 146 + 33 + 81;
    /// Minimal enough transaction fee per byte.
    const MIN_TX_FEE: u64 = Self::MIN_TOTAL_TX_FEE / Self::MIN_TX_LEN + 1; // Round up.

    /// Creates Bitcoin anchoring config instance with default parameters for the
    /// given Bitcoin network and public keys of participants.
    pub fn with_public_keys(
        network: Network,
        keys: impl IntoIterator<Item = AnchoringKeys>,
    ) -> Result<Self, RedeemScriptError> {
        let anchoring_keys = keys.into_iter().collect::<Vec<_>>();
        if anchoring_keys.is_empty() {
            return Err(RedeemScriptError::NotEnoughPublicKeys);
        }

        Ok(Self {
            network,
            anchoring_keys,
            ..Self::default()
        })
    }

    /// Tries to find bitcoin public key corresponding with the given service key.
    pub fn find_bitcoin_key(&self, service_key: &PublicKey) -> Option<(u16, btc::PublicKey)> {
        self.anchoring_keys.iter().enumerate().find_map(|(n, x)| {
            if &x.service_key == service_key {
                Some((n as u16, x.bitcoin_key))
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
        RedeemScriptBuilder::with_public_keys(self.anchoring_keys.iter().map(|x| x.bitcoin_key.0))
            .quorum(self.byzantine_quorum())
            .to_script()
            .unwrap()
    }

    /// Computes the P2WSH output corresponding to the actual redeem script.
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

    /// Returns sufficient number of votes for the given anchoring nodes number.
    pub fn byzantine_quorum(&self) -> usize {
        exonum::helpers::byzantine_quorum(self.anchoring_keys.len())
    }
}

impl ValidateInput for Config {
    type Error = failure::Error;

    fn validate(&self) -> Result<(), Self::Error> {
        ensure!(
            !self.anchoring_keys.is_empty(),
            "The list of anchoring keys must not be empty."
        );
        ensure!(
            self.anchoring_keys.len() <= Self::MAX_NODES_COUNT,
            "Too many anchoring nodes: amount of anchoring nodes should be less or equal than the {}.",
            Self::MAX_NODES_COUNT
        );
        ensure!(
            self.anchoring_interval > 0,
            "Anchoring interval should be greater than zero."
        );
        ensure!(
            self.transaction_fee > Self::MIN_TX_FEE,
            "Transaction fee should be greater than {}",
            Self::MIN_TX_FEE
        );

        // Verify that the redeem script is suitable.
        RedeemScriptBuilder::with_public_keys(self.anchoring_keys.iter().map(|x| x.bitcoin_key.0))
            .quorum(self.byzantine_quorum())
            .to_script()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use exonum::{
        crypto,
        helpers::{Height, ValidateInput},
    };

    use bitcoin::network::constants::Network;
    use btc_transaction_utils::test_data::secp_gen_keypair;

    use crate::proto::AnchoringKeys;

    use super::Config;

    fn gen_anchoring_keys(network: bitcoin::Network, count: usize) -> Vec<AnchoringKeys> {
        (0..count)
            .map(|_| AnchoringKeys {
                bitcoin_key: secp_gen_keypair(network).0.into(),
                service_key: crypto::gen_keypair().0,
            })
            .collect::<Vec<_>>()
    }

    #[test]
    fn config_serde() {
        let public_keys = gen_anchoring_keys(Network::Bitcoin, 4);

        let config = Config::with_public_keys(Network::Bitcoin, public_keys).unwrap();
        assert_eq!(config.redeem_script().content().quorum, 3);

        let json = serde_json::to_value(&config).unwrap();
        let config2: Config = serde_json::from_value(json).unwrap();
        assert_eq!(config2, config);
    }

    #[test]
    fn config_anchoring_height() {
        let public_keys = gen_anchoring_keys(Network::Bitcoin, 4);

        let mut config = Config::with_public_keys(Network::Bitcoin, public_keys).unwrap();
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

    // TODO test validation of the Bitcoin anchoring config

    #[test]
    fn config_validate_errors() {
        let test_cases = [
            (
                Config::default(),
                "The list of anchoring keys must not be empty",
            ),
            (
                Config {
                    anchoring_keys: gen_anchoring_keys(bitcoin::Network::Regtest, 30),
                    ..Config::default()
                },
                "Too many anchoring nodes: amount of anchoring nodes should be less or equal",
            ),
            (
                Config {
                    anchoring_keys: gen_anchoring_keys(bitcoin::Network::Regtest, 4),
                    anchoring_interval: 0,
                    ..Config::default()
                },
                "Anchoring interval should be greater than zero",
            ),
            (
                Config {
                    anchoring_keys: gen_anchoring_keys(bitcoin::Network::Regtest, 4),
                    transaction_fee: 0,
                    ..Config::default()
                },
                "Transaction fee should be greater than",
            ),
            (
                Config {
                    anchoring_keys: gen_anchoring_keys(bitcoin::Network::Regtest, 4),
                    transaction_fee: 3,
                    ..Config::default()
                },
                "Transaction fee should be greater than",
            ),
        ];

        for (config, expected_err) in &test_cases {
            let actual_err = config.validate().unwrap_err().to_string();
            assert!(actual_err.contains(expected_err), actual_err);
        }
    }
}
