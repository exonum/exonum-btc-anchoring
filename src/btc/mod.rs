// Copyright 2018 The Exonum Team
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

pub use self::payload::{Payload, PayloadBuilder};
pub use self::transaction::{BtcAnchoringTransactionBuilder, BuilderError, Transaction};

use bitcoin::network::constants::Network;
use bitcoin::util::address;
use bitcoin::util::privkey;
use rand::{self, Rng};
use secp256k1;
use std::ops::Deref;

#[macro_use]
mod macros;

pub mod payload;
pub mod transaction;

/// A Bitcoin ECDSA private key.
#[derive(Clone, From, Into, PartialEq, Eq)]
pub struct Privkey(pub privkey::Privkey);

/// A Secp256k1 public key, used for verification of signatures.
#[derive(Debug, Clone, Copy, From, Into, PartialEq, Eq)]
pub struct PublicKey(pub secp256k1::PublicKey);

/// A Bitcoin address
#[derive(Debug, Clone, From, Into, PartialEq)]
pub struct Address(pub address::Address);

impl ToString for Privkey {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl ::std::str::FromStr for Privkey {
    type Err = <privkey::Privkey as ::std::str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        privkey::Privkey::from_str(s).map(From::from)
    }
}

impl ::std::fmt::Debug for Privkey {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.debug_struct("Privkey").finish()
    }
}

impl ::exonum::encoding::serialize::FromHex for PublicKey {
    type Error = ::failure::Error;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        let bytes = ::exonum::encoding::serialize::decode_hex(hex)?;
        let context = secp256k1::Secp256k1::without_caps();
        let inner = secp256k1::PublicKey::from_slice(&context, &bytes)?;
        Ok(PublicKey(inner))
    }
}

impl ::exonum::encoding::serialize::ToHex for PublicKey {
    fn write_hex<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        let bytes = self.0.serialize();
        bytes.as_ref().write_hex(w)
    }

    fn write_hex_upper<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        let bytes = self.0.serialize();
        bytes.as_ref().write_hex_upper(w)
    }
}

impl ::std::str::FromStr for Address {
    type Err = <address::Address as ::std::str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = address::Address::from_str(s)?;
        Ok(Address(inner))
    }
}

impl ::std::fmt::Display for Address {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.0.to_string())
    }
}

impl Eq for Address {}

impl PartialOrd for Address {
    fn partial_cmp(&self, other: &Address) -> Option<::std::cmp::Ordering> {
        Some(self.to_string().cmp(&other.to_string()))
    }
}

impl Ord for Address {
    fn cmp(&self, other: &Address) -> ::std::cmp::Ordering {
        // TODO: Add `Ord` to the underlying crates.
        self.to_string().cmp(&other.to_string())
    }
}

#[cfg_attr(feature = "cargo-clippy", allow(derive_hash_xor_eq))]
impl ::std::hash::Hash for Address {
    fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
        // TODO: Add `Hash` to the underlying crates.
        self.to_string().hash(state)
    }
}

impl Deref for Address {
    type Target = address::Address;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl_string_conversions_for_hex! { PublicKey }

impl_serde_str! { Privkey }
impl_serde_str! { PublicKey }
impl_serde_str! { Address }

/// Generates public and secret keys for Bitcoin node
/// using given random number generator.
pub fn gen_keypair_with_rng<R: Rng>(network: Network, rng: &mut R) -> (PublicKey, Privkey) {
    let context = secp256k1::Secp256k1::new();
    let sk = secp256k1::key::SecretKey::new(&context, rng);

    let priv_key = privkey::Privkey::from_secret_key(sk, true, network);
    let pub_key = secp256k1::PublicKey::from_secret_key(&context, &sk).unwrap();
    (pub_key.into(), priv_key.into())
}

/// Same as [`gen_btc_keypair_with_rng`](fn.gen_btc_keypair_with_rng.html)
/// but it uses default random number generator.
pub fn gen_keypair(network: Network) -> (PublicKey, Privkey) {
    let mut rng = rand::thread_rng();
    gen_keypair_with_rng(network, &mut rng)
}
