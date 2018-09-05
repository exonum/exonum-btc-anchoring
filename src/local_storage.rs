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

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::collections::BTreeMap;
use std::default::Default;

use details::btc;
use details::rpc::AnchoringRpcConfig;
use handler::observer::AnchoringObserverConfig;

/// Private part of anchoring service configuration stored on a local machine.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AnchoringNodeConfig {
    /// Rpc configuration. Must exist if node is validator.
    /// Otherwise node can only check `lect` payload without any checks with `bitcoind`.
    pub rpc: Option<AnchoringRpcConfig>,
    #[serde(serialize_with = "serialize_map_to_vec", deserialize_with = "deserialize_vec_to_map")]
    /// Set of private keys for each anchoring address.
    pub private_keys: BTreeMap<String, btc::PrivateKey>,
    /// Frequency of lect check in blocks.
    pub check_lect_frequency: u64,
    /// Anchoring observer config.
    pub observer: AnchoringObserverConfig,
}

impl AnchoringNodeConfig {
    /// Creates blank configuration from given rpc config.
    pub fn new(rpc: Option<AnchoringRpcConfig>) -> AnchoringNodeConfig {
        AnchoringNodeConfig {
            rpc,
            ..Default::default()
        }
    }
}

impl Default for AnchoringNodeConfig {
    fn default() -> AnchoringNodeConfig {
        AnchoringNodeConfig {
            rpc: None,
            observer: AnchoringObserverConfig::default(),
            private_keys: BTreeMap::new(),
            check_lect_frequency: 30,
        }
    }
}

/// The structure for storing the anchoring address and private key. The structure is needed to
/// convert data from the toml-file into memory.
#[derive(Deserialize, Serialize)]
struct AnchoringKeypair {
    /// Anchoring address.
    address: String,
    /// Private key.
    private_key: btc::PrivateKey,
}

fn serialize_map_to_vec<S>(
    map: &BTreeMap<String, btc::PrivateKey>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let keypairs = map
        .iter()
        .map(|(address, private_key)| AnchoringKeypair {
            address: address.to_string(),
            private_key: private_key.clone(),
        })
        .collect::<Vec<_>>();

    keypairs.serialize(serializer)
}

fn deserialize_vec_to_map<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, btc::PrivateKey>, D::Error>
where
    D: Deserializer<'de>,
{
    let keypairs: Vec<AnchoringKeypair> = Vec::deserialize(deserializer)?;
    let map = keypairs
        .iter()
        .map(|keypair| (keypair.address.to_string(), keypair.private_key.clone()))
        .collect::<BTreeMap<_, _>>();
    Ok(map)
}
