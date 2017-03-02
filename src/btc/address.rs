use bitcoin::network::constants::Network;

use super::types::{Address, RawScript, RawAddress};

impl Address {
    pub fn from_script(script: &RawScript, network: Network) -> Address {
        RawAddress::from_script(network, script).into()
    }
}