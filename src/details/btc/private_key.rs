use bitcoin::network::constants::Network;
use secp256k1::key;

use super::types::{PrivateKey, RawPrivkey};

impl PrivateKey {
    pub fn from_key(network: Network, sk: key::SecretKey, compressed: bool) -> PrivateKey {
        let raw = RawPrivkey::from_key(network, sk, compressed);
        PrivateKey::from(raw)
    }
}
