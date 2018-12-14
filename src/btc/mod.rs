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

//! Collection of wrappers for the rust-bitcoin crate.

pub use self::payload::Payload;
pub use self::transaction::{BtcAnchoringTransactionBuilder, BuilderError, Transaction};

use bitcoin::network::constants::Network;
use bitcoin::util::address;
use bitcoin::util::privkey;
use btc_transaction_utils;
use hex::{self, FromHex, ToHex};

use rand::{self, Rng};
use secp256k1;
use std::ops::Deref;

#[macro_use]
mod macros;

pub(crate) mod payload;
pub(crate) mod transaction;

/// Bitcoin ECDSA private key wrapper.
#[derive(Clone, From, Into, PartialEq, Eq)]
pub struct Privkey(pub privkey::Privkey);

/// Secp256k1 public key wrapper, used for verification of signatures.
#[derive(Debug, Clone, Copy, From, Into, PartialEq, Eq)]
pub struct PublicKey(pub secp256k1::PublicKey);

/// Bitcoin address wrapper.
#[derive(Debug, Clone, From, Into, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address(pub address::Address);

/// Bitcoin input signature wrapper.
#[derive(Debug, Clone, PartialEq, Into, From)]
pub struct InputSignature(pub btc_transaction_utils::InputSignature);

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

impl FromHex for PublicKey {
    type Error = ::failure::Error;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        let bytes = hex::decode(hex)?;
        let context = secp256k1::Secp256k1::without_caps();
        let inner = secp256k1::PublicKey::from_slice(&context, &bytes)?;
        Ok(PublicKey(inner))
    }
}

impl ToHex for PublicKey {
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

impl Deref for Address {
    type Target = address::Address;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromHex for InputSignature {
    type Error = ::failure::Error;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        let bytes = hex::decode(hex)?;
        let context = secp256k1::Secp256k1::without_caps();
        let inner = btc_transaction_utils::InputSignature::from_bytes(&context, bytes)?;
        Ok(InputSignature(inner))
    }
}

impl ToHex for InputSignature {
    fn write_hex<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        self.0.as_ref().write_hex(w)
    }

    fn write_hex_upper<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        self.0.as_ref().write_hex_upper(w)
    }
}

impl AsRef<btc_transaction_utils::InputSignature> for InputSignature {
    fn as_ref(&self) -> &btc_transaction_utils::InputSignature {
        &self.0
    }
}

impl From<InputSignature> for Vec<u8> {
    fn from(f: InputSignature) -> Self {
        f.0.into()
    }
}

impl_string_conversions_for_hex! { PublicKey }
impl_string_conversions_for_hex! { InputSignature }

impl_serde_str! { Privkey }
impl_serde_str! { PublicKey }
impl_serde_str! { Address }
impl_serde_str! { InputSignature }

/// Generates public and secret keys for Bitcoin node
/// using given random number generator.
pub fn gen_keypair_with_rng<R: Rng>(network: Network, rng: &mut R) -> (PublicKey, Privkey) {
    let context = secp256k1::Secp256k1::new();
    let sk = secp256k1::key::SecretKey::new(&context, rng);

    let priv_key = privkey::Privkey::from_secret_key(sk, true, network);
    let pub_key = secp256k1::PublicKey::from_secret_key(&context, &sk);
    (pub_key.into(), priv_key.into())
}

/// Same as [`gen_keypair_with_rng`](fn.gen_keypair_with_rng.html)
/// but it uses default random number generator.
pub fn gen_keypair(network: Network) -> (PublicKey, Privkey) {
    let mut rng = rand::thread_rng();
    gen_keypair_with_rng(network, &mut rng)
}
