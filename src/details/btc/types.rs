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

use exonum::crypto::{FromHexError, Hash, HexValue, hash};
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
}

impl AsRef<[u8]> for TxId {
    fn as_ref(&self) -> &[u8] {
        self.0[..].as_ref()
    }
}

// TODO replace by more clear solution
impl HexValue for TxId {
    fn to_hex(&self) -> String {
        self.be_hex_string()
    }

    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
        match Sha256dHash::from_hex(v.as_ref()) {
            Ok(hash) => Ok(TxId::from(hash)),
            Err(_) => Err(FromHexError::InvalidHexLength),
        }
    }
}

impl HexValue for PublicKey {
    fn to_hex(&self) -> String {
        let context = Secp256k1::without_caps();
        let array = self.0.serialize_vec(&context, true);
        ::exonum::encoding::serialize::ToHex::to_hex(&array.as_slice())
    }

    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
        let context = Secp256k1::without_caps();
        let bytes = Vec::<u8>::from_hex(v.as_ref())?;
        match RawPublicKey::from_slice(&context, bytes.as_ref()) {
            Ok(key) => Ok(PublicKey(key)),
            Err(_) => Err(FromHexError::InvalidHexLength),
        }
    }
}

impl HexValue for RedeemScript {
    fn to_hex(&self) -> String {
        self.0.clone().into_vec().to_hex()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
        let bytes = Vec::<u8>::from_hex(v.as_ref())?;
        Ok(RedeemScript::from(Builder::from(bytes).into_script()))
    }
}

impl HexValueEx for RawScript {
    fn to_hex(&self) -> String {
        self.clone().into_vec().to_hex()
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

    fn hash(&self) -> Hash {
        hash(self.0.clone().into_vec().as_ref())
    }
}

impl StorageKey for TxId {
    fn size(&self) -> usize {
        TXID_SIZE
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.as_ref())
    }

    fn read(buffer: &[u8]) -> Self {
        TxId::from_slice(buffer).unwrap()
    }
}
