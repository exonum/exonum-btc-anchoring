// Copyright 2019 The Exonum Team
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

pub use btc_transaction_utils::test_data::{secp_gen_keypair, secp_gen_keypair_with_rng};

pub use self::{
    payload::Payload,
    transaction::{BtcAnchoringTransactionBuilder, BuilderError, Transaction},
};

use bitcoin::{network::constants::Network, util::address};
use bitcoin_hashes::sha256d;
use btc_transaction_utils;
use derive_more::{From, Into};
use exonum::crypto;
use hex::{self, FromHex, ToHex};
use rand::Rng;

use std::ops::Deref;

#[macro_use]
mod macros;

pub(crate) mod payload;
pub(crate) mod transaction;

/// Bitcoin ECDSA private key wrapper.
#[derive(Clone, From, Into, PartialEq, Eq)]
pub struct PrivateKey(pub bitcoin::PrivateKey);

/// Secp256k1 public key wrapper, used for verification of signatures.
#[derive(Debug, Clone, Copy, From, Into, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicKey(pub bitcoin::PublicKey);

/// Bitcoin address wrapper.
#[derive(Debug, Clone, From, Into, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address(pub address::Address);

/// Bitcoin input signature wrapper.
#[derive(Debug, Clone, PartialEq, Into, From)]
pub struct InputSignature(pub btc_transaction_utils::InputSignature);

impl ToString for PrivateKey {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl std::str::FromStr for PrivateKey {
    type Err = <bitcoin::PrivateKey as ::std::str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        bitcoin::PrivateKey::from_str(s).map(From::from)
    }
}

impl std::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.debug_struct("PrivateKey").finish()
    }
}

impl FromHex for PublicKey {
    type Error = ::failure::Error;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        let bytes = hex::decode(hex)?;
        let inner = bitcoin::PublicKey::from_slice(&bytes)?;
        Ok(Self(inner))
    }
}

impl ToHex for PublicKey {
    fn encode_hex<T: std::iter::FromIterator<char>>(&self) -> T {
        let mut bytes = Vec::default();
        self.0.write_into(&mut bytes);
        bytes.encode_hex()
    }

    fn encode_hex_upper<T: std::iter::FromIterator<char>>(&self) -> T {
        let mut bytes = Vec::default();
        self.0.write_into(&mut bytes);
        bytes.encode_hex_upper()
    }
}

impl std::str::FromStr for Address {
    type Err = <address::Address as ::std::str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = address::Address::from_str(s)?;
        Ok(Self(inner))
    }
}

impl std::fmt::Display for Address {
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
        let inner = btc_transaction_utils::InputSignature::from_bytes(bytes)?;
        Ok(Self(inner))
    }
}

impl ToHex for InputSignature {
    fn encode_hex<T: std::iter::FromIterator<char>>(&self) -> T {
        self.0.as_ref().encode_hex()
    }

    fn encode_hex_upper<T: std::iter::FromIterator<char>>(&self) -> T {
        self.0.as_ref().encode_hex_upper()
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

impl_serde_str! { PrivateKey }
impl_serde_str! { PublicKey }
impl_serde_str! { Address }
impl_serde_str! { InputSignature }

/// Generates Bitcoin keypair using the given random number generator.
pub fn gen_keypair_with_rng<R: Rng>(rng: &mut R, network: Network) -> (PublicKey, PrivateKey) {
    let (pk, sk) = secp_gen_keypair_with_rng(rng, network);
    (PublicKey(pk), PrivateKey(sk))
}

/// Same as [`gen_keypair_with_rng`](fn.gen_keypair_with_rng.html)
/// but it uses a default random number generator.
pub fn gen_keypair(network: Network) -> (PublicKey, PrivateKey) {
    let (pk, sk) = secp_gen_keypair(network);
    (PublicKey(pk), PrivateKey(sk))
}

/// Converts Bitcoin sha256d type to the Exonum hash type.
///
/// This types have same data layout, and so we can convert the Bitcoin hash type to use in the
/// Merkledb without "newtype" wrapper.
pub(crate) fn sha256d_to_exonum_hash(hash: sha256d::Hash) -> crypto::Hash {
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&hash[..]);
    bytes.reverse();
    crypto::Hash::new(bytes)
}

/// Converts Exonum hash type back to the Bitcoin sha256d.
pub(crate) fn exonum_hash_to_sha256d(hash: crypto::Hash) -> sha256d::Hash {
    use bitcoin_hashes::Hash;
    sha256d::Hash::from_slice(hash.as_ref()).unwrap()
}
