//! Module contains some wrappers over types from `Bitcoin` crate.

mod types;
mod redeem_script;
mod address;
mod private_key;
mod public_key;
pub mod transactions;

use rand;
use rand::Rng;

use secp256k1::Secp256k1;
use secp256k1::key;

use exonum::crypto::FromHexError;

#[doc(hidden)]
/// For test purpose only
pub use self::types::{Address, PrivateKey, PublicKey, TxId, RedeemScript, RawTransaction,
                      Signature};
pub use bitcoin::network::constants::Network;

#[doc(hidden)]
/// For test purpose only
pub trait HexValueEx: Sized {
    fn to_hex(&self) -> String;
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError>;
}

/// Same as [`gen_btc_keypair_with_rng`](fn.gen_btc_keypair_with_rng.html)
/// but it uses default random number generator.
pub fn gen_btc_keypair(network: Network) -> (PublicKey, PrivateKey) {
    let mut rng = rand::thread_rng();
    gen_btc_keypair_with_rng(network, &mut rng)
}

/// Generates public and secret keys for Bitcoin node
/// using given random number generator.
pub fn gen_btc_keypair_with_rng<R: Rng>(network: Network, rng: &mut R) -> (PublicKey, PrivateKey) {
    let context = Secp256k1::new();
    let sk = key::SecretKey::new(&context, rng);

    let priv_key = PrivateKey::from_key(network, sk, true);
    let pub_key = PublicKey::from_secret_key(&context, &sk).unwrap();
    (pub_key, priv_key)
}
