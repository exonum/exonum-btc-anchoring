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

use std::borrow::Cow;
use std::ops::Deref;
use std::fmt;

pub use bitcoin::blockdata::transaction::Transaction as RawTransaction;
pub use bitcoin::util::address::{Address as RawAddress, Privkey as RawPrivkey};
pub use bitcoin::blockdata::script::Script as RawScript;
use bitcoin::blockdata::script::Builder;
use bitcoin::util::hash::Sha256dHash;
use bitcoin::util::base58::{Error as FromBase58Error, FromBase58, ToBase58};

pub use secp256k1::key::PublicKey as RawPublicKey;
use secp256k1::Secp256k1;

use exonum::crypto::{hash, CryptoHash, Hash};
use exonum::encoding::serialize::{encode_hex, FromHex, FromHexError, ToHex};
use exonum::encoding::Field;
use exonum::storage::{StorageKey, StorageValue};

use super::HexValueEx;

#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct TxId(Sha256dHash);
#[derive(Clone, PartialEq)]
pub struct PrivateKey(pub RawPrivkey);
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct PublicKey(pub RawPublicKey);
#[derive(Clone, PartialEq, Eq)]
pub struct Address(pub RawAddress);
#[derive(Debug, Clone, PartialEq)]
pub struct RedeemScript(pub RawScript);

pub type Signature = Vec<u8>;

implement_wrapper! {Sha256dHash, TxId}
implement_wrapper! {RawPublicKey, PublicKey}
implement_wrapper! {RawAddress, Address}
implement_wrapper! {RawPrivkey, PrivateKey}
implement_wrapper! {RawScript, RedeemScript}

implement_base58_wrapper! {RawAddress, Address}
implement_base58_wrapper! {RawPrivkey, PrivateKey}

implement_serde_hex! {PublicKey}
implement_serde_hex! {RedeemScript}
implement_serde_hex! {TxId}
implement_serde_base58check! {Address}
implement_serde_base58check! {PrivateKey}

implement_pod_as_ref_field! { TxId }

const TXID_SIZE: usize = 32;

impl TxId {
    pub fn from_slice(s: &[u8]) -> Option<TxId> {
        if s.len() == TXID_SIZE {
            Some(TxId(Sha256dHash::from(s)))
        } else {
            None
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0[..].as_ref()
    }
}

// TODO replace by more clear solution
impl FromHex for TxId {
    type Error = FromHexError;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Self::Error> {
        let bytes = Vec::<u8>::from_hex(v)?;
        if bytes.len() != 32 {
            return Err(FromHexError::InvalidStringLength);
        }
        // Convert to big endian. (i.e. reversed vs sha256sum output)
        let bytes = bytes.into_iter().rev().collect::<Vec<_>>();
        Ok(TxId(Sha256dHash::from(bytes.as_slice())))
    }
}

impl ToHex for TxId {
    fn write_hex<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        let out = self.0.be_hex_string();
        w.write_str(&out)
    }

    fn write_hex_upper<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        let out = self.0.be_hex_string().to_uppercase();
        w.write_str(&out)
    }
}

impl FromHex for PublicKey {
    type Error = FromHexError;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Self::Error> {
        let context = Secp256k1::without_caps();
        let bytes = Vec::<u8>::from_hex(v)?;
        match RawPublicKey::from_slice(&context, bytes.as_ref()) {
            Ok(key) => Ok(PublicKey(key)),
            Err(_) => Err(FromHexError::InvalidStringLength),
        }
    }
}

impl ToHex for PublicKey {
    fn write_hex<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.serialize().as_ref().write_hex(w)
    }

    fn write_hex_upper<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.serialize().as_ref().write_hex_upper(w)
    }
}

impl FromHex for RedeemScript {
    type Error = FromHexError;

    fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Self::Error> {
        let bytes = Vec::<u8>::from_hex(v)?;
        Ok(RedeemScript::from(Builder::from(bytes).into_script()))
    }
}

impl ToHex for RedeemScript {
    fn write_hex<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.0.clone().into_vec().write_hex(w)
    }

    fn write_hex_upper<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        self.0.clone().into_vec().write_hex_upper(w)
    }
}

impl HexValueEx for RawScript {
    fn to_hex(&self) -> String {
        encode_hex(self.clone().into_vec())
    }
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError> {
        let bytes = Vec::<u8>::from_hex(v.as_ref())?;
        Ok(Builder::from(bytes).into_script())
    }
}

impl StorageValue for RedeemScript {
    fn into_bytes(self) -> Vec<u8> {
        self.0.into_vec()
    }

    fn from_bytes(v: Cow<[u8]>) -> RedeemScript {
        RedeemScript(RawScript::from(v.into_owned()))
    }
}

impl CryptoHash for RedeemScript {
    fn hash(&self) -> Hash {
        hash(self.0.clone().into_vec().as_ref())
    }
}

impl StorageKey for TxId {
    fn size(&self) -> usize {
        TXID_SIZE
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.0[..].as_ref())
    }

    fn read(buffer: &[u8]) -> Self {
        TxId::from_slice(buffer).unwrap()
    }
}
