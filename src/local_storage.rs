use std::default::Default;
use std::collections::BTreeMap;

use details::rpc::AnchoringRpcConfig;
use details::btc;

/// Private part of anchoring service configuration stored on a local machine.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AnchoringNodeConfig {
    /// Rpc configuration. Must be exist if node is validator 
    /// otherwise node can only check `lect` payload without any checks with `bitcoind`.
    pub rpc: Option<AnchoringRpcConfig>,
    /// Set of private keys for each anchoring address
    pub private_keys: BTreeMap<String, btc::PrivateKey>,
    /// Frequency of lect check in blocks
    pub check_lect_frequency: u64,
}

impl AnchoringNodeConfig {
    /// Creates blank configuration from given rpc config.
    pub fn new(rpc: Option<AnchoringRpcConfig>) -> AnchoringNodeConfig {
        AnchoringNodeConfig {
            rpc: rpc,
            ..Default::default()
        }
    }
}

impl Default for AnchoringNodeConfig {
    fn default() -> AnchoringNodeConfig {
        AnchoringNodeConfig {
            rpc: None,
            private_keys: BTreeMap::new(),
            check_lect_frequency: 30,
        }
    }
}