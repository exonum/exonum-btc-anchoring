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

use std::default::Default;
use std::collections::BTreeMap;

use details::rpc::AnchoringRpcConfig;
use details::btc;
use observer::AnchoringObserverConfig;

/// Private part of anchoring service configuration stored on a local machine.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AnchoringNodeConfig {
    /// Rpc configuration. Must exist if node is validator.
    /// Otherwise node can only check `lect` payload without any checks with `bitcoind`.
    pub rpc: Option<AnchoringRpcConfig>,
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
