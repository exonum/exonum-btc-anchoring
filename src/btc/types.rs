use std::ops::Deref;
use std::fmt;

pub use bitcoin::blockdata::transaction::Transaction as RawTransaction;
pub use bitcoin::util::address::{Privkey as RawPrivkey, Address as RawAddress};
pub use bitcoin::blockdata::script::Script as RawScript;
use bitcoin::blockdata::script::Builder;
use bitcoin::util::hash::Sha256dHash;
use bitcoin::util::base58::{FromBase58, ToBase58, Error as FromBase58Error};

pub use secp256k1::key::PublicKey as RawPublicKey;
use secp256k1::Secp256k1;

use exonum::crypto::{HexValue, FromHexError, hash, Hash};
use exonum::messages::Field;
use exonum::storage::StorageValue;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TxId(Sha256dHash);
#[derive(Clone, PartialEq)]
pub struct PrivateKey(pub RawPrivkey);
#[derive(Debug, Clone, PartialEq)]
pub struct PublicKey(pub RawPublicKey);
#[derive(Clone, PartialEq)]
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
implement_serde_base58check! {Address}
implement_serde_base58check! {PrivateKey}

impl TxId {
    pub fn from_slice(s: &[u8]) -> Option<TxId> {
        if s.len() == 32 {
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
        ::exonum::crypto::ToHex::to_hex(&array.as_slice())
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

impl<'a> Field<'a> for &'a TxId {
    fn field_size() -> usize {
        32
    }

    fn read(buffer: &'a [u8], from: usize, _: usize) -> &'a TxId {
        unsafe { ::std::mem::transmute(&buffer[from]) }
    }

    fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
        buffer[from..to].copy_from_slice(self.as_ref());
    }
}

impl StorageValue for RedeemScript {
    fn serialize(self) -> Vec<u8> {
        self.0.into_vec()
    }

    fn deserialize(v: Vec<u8>) -> RedeemScript {
        RedeemScript(RawScript::from(v))
    }

    fn hash(&self) -> Hash {
        hash(self.0.clone().into_vec().as_ref())
    }
}
