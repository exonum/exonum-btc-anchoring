use std::collections::BTreeMap;

use details::rpc::AnchoringRpcConfig;
use details::btc;

/// Private part of anchoring service configuration stored on a local machine.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AnchoringNodeConfig {
    /// Rpc configuration
    pub rpc: AnchoringRpcConfig,
    /// Set of private keys for each anchoring address
    pub private_keys: BTreeMap<String, btc::PrivateKey>,
    /// Frequency of lect check in blocks
    pub check_lect_frequency: u64,
}

impl AnchoringNodeConfig {
    /// Creates blank configuration from given rpc config.
    pub fn new(rpc: AnchoringRpcConfig) -> AnchoringNodeConfig {
        AnchoringNodeConfig {
            rpc: rpc,
            private_keys: BTreeMap::new(),
            check_lect_frequency: 30,
        }
    }
}
y