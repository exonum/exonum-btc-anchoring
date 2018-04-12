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

//! Module contains some wrappers over types from `Bitcoin` crate.

mod types;
mod redeem_script;
mod private_key;
mod public_key;
pub mod payload;
pub mod transactions;

use rand;
use rand::Rng;

use secp256k1::Secp256k1;
use secp256k1::key;

use exonum::encoding::serialize::FromHexError;

#[doc(hidden)]
/// For test purpose only
pub use self::types::{Address, PrivateKey, PublicKey, RawTransaction, RedeemScript, Signature,
                      TxId};
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
