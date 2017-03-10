mod types;
mod redeem_script;
mod address;
mod private_key;
mod public_key;
pub mod regtest;

use rand;
use rand::Rng;

use bitcoin::network::constants::Network;
use secp256k1::Secp256k1;
use secp256k1::key;

use exonum::crypto::FromHexError;

pub use self::types::{Address, PrivateKey, PublicKey, TxId, RedeemScript, Transaction,
                      RawTransaction};

pub trait HexValueEx: Sized {
    fn to_hex(&self) -> String;
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError>;
}

pub fn gen_keypair(network: Network) -> (PublicKey, PrivateKey) {
    let mut rng = rand::thread_rng();
    gen_keypair_with_rng(network, &mut rng)
}

pub fn gen_keypair_with_rng<R: Rng>(network: Network, rng: &mut R) -> (PublicKey, PrivateKey) {
    let context = Secp256k1::new();
    let sk = key::SecretKey::new(&context, rng);

    let priv_key = PrivateKey::from_key(network, sk, true);
    let pub_key = PublicKey::from_secret_key(&context, &sk).unwrap();
    (pub_key, priv_key)
}
