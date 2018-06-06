// Copyright 2017 The Exonum Team
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

use std::borrow::Cow;

use serde::{Deserialize, Deserializer};
use serde_json;

use exonum::crypto::{hash, CryptoHash, Hash};
use exonum::helpers::Height;
use exonum::storage::StorageValue;

use details::btc;
use details::btc::transactions::FundingTx;

/// Public part of anchoring service configuration stored in blockchain.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct AnchoringConfig {
    /// Validators' public keys from which the current anchoring address can be calculated.
    pub anchoring_keys: Vec<btc::PublicKey>,
    /// The transaction that funds anchoring address.
    /// If the anchoring transactions chain is empty, it will be the first transaction in the chain.
    /// Note: you must specify a suitable transaction before the network launching.
    pub funding_tx: Option<FundingTx>,
    /// Fee for each transaction in chain.
    pub fee: u64,
    /// The frequency in blocks with which the generation of new anchoring
    /// transactions in the chain occurs.
    pub frequency: u64,
    /// The minimum number of confirmations in bitcoin network for the transition to a
    /// new anchoring address.
    pub utxo_confirmations: u64,
    /// The current bitcoin network type.
    #[serde(serialize_with = "btc_network_to_str", deserialize_with = "btc_network_from_str")]
    pub network: btc::Network,
}

impl Default for AnchoringConfig {
    fn default() -> AnchoringConfig {
        AnchoringConfig {
            anchoring_keys: vec![],
            funding_tx: None,
            fee: 1000,
            frequency: 500,
            utxo_confirmations: 5,
            network: btc::Network::Testnet,
        }
    }
}

impl AnchoringConfig {
    /// Creates anchoring configuration for the given `anchoring_keys` without funding transaction.
    /// This is usable for deploying procedure when the network participants exchange
    /// the public configuration before launching.
    /// Do not forget to send funding transaction to the final multisig address
    /// and add it to the final configuration.
    pub fn new<I>(network: btc::Network, anchoring_keys: I) -> AnchoringConfig
    where
        I: IntoIterator<Item = btc::PublicKey>,
    {
        AnchoringConfig {
            anchoring_keys: anchoring_keys.into_iter().collect(),
            network,
            ..Default::default()
        }
    }

    /// Creates default anchoring configuration from given public keys and funding transaction
    /// which were created earlier by other way.
    pub fn new_with_funding_tx<I>(
        network: btc::Network,
        anchoring_keys: I,
        tx: FundingTx,
    ) -> AnchoringConfig
    where
        I: IntoIterator<Item = btc::PublicKey>,
    {
        AnchoringConfig {
            anchoring_keys: anchoring_keys.into_iter().collect(),
            funding_tx: Some(tx),
            network,
            ..Default::default()
        }
    }

    #[doc(hidden)]
    /// Creates compressed `RedeemScript` from public keys in config.
    pub fn redeem_script(&self) -> (btc::RedeemScript, btc::Address) {
        let majority_count = self.majority_count();
        let redeem_script = btc::RedeemScriptBuilder::with_public_keys(
            self.anchoring_keys.iter().map(|x| x.0),
        ).quorum(majority_count as usize)
            .to_script()
            .unwrap();
        let addr = btc::Address::from_script(&redeem_script, self.network);
        (redeem_script, addr)
    }

    #[doc(hidden)]
    /// Returns the latest height below the given `height` which needs to be anchored.
    pub fn latest_anchoring_height(&self, height: Height) -> Height {
        Height(height.0 - height.0 % self.frequency as u64)
    }

    #[doc(hidden)]
    pub fn majority_count(&self) -> u8 {
        ::majority_count(self.anchoring_keys.len() as u8)
    }

    /// Returns the funding transaction.
    ///
    /// # Panics
    ///
    /// If funding transaction is not specified.
    pub fn funding_tx(&self) -> &FundingTx {
        self.funding_tx.as_ref().expect(
            "You need to specify suitable \
             funding_tx",
        )
    }
}

fn btc_network_to_str<S>(network: &btc::Network, ser: S) -> Result<S::Ok, S::Error>
where
    S: ::serde::Serializer,
{
    ser.serialize_str(&network.to_string())
}

fn btc_network_from_str<'de, D>(deserializer: D) -> Result<btc::Network, D::Error>
where
    D: Deserializer<'de>,
{
    const VARIANTS: &[&str] = &["bitcoin", "testnet", "regtest"];
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<btc::Network>()
        .map_err(|_| ::serde::de::Error::unknown_variant(&s, VARIANTS))
}

impl StorageValue for AnchoringConfig {
    fn into_bytes(self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        serde_json::from_slice(value.as_ref()).unwrap()
    }
}

impl CryptoHash for AnchoringConfig {
    fn hash(&self) -> Hash {
        hash(serde_json::to_vec(&self).unwrap().as_slice())
    }
}
