use std::ops::Deref;
use std::fmt;

pub use bitcoin::blockdata::transaction::Transaction as RawTransaction;
pub use bitcoin::util::address::{Privkey as RawPrivkey, Address as RawAddress};
pub use bitcoin::blockdata::script::Script as RawScript;
use bitcoin::blockdata::script::Builder;
use bitcoin::util::hash::Sha256dHash;
use bitcoin::util::base58::{FromBase58, ToBase58, Error as FromBase58Error};

use secp256k1::key::PublicKey as RawPublicKey;
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
#[derive(Clone, Debug, PartialEq)]
pub struct RedeemScript(pub RawScript);
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction(pub RawTransaction);

macro_rules! implement_wrapper {
    ($from:ident, $to:ident) => (
        impl Deref for $to {
            type Target = $from;

            fn deref(&self) -> &$from {
                &self.0
            }
        }

        impl From<$from> for $to {
            fn from(p: $from) -> $to {
                $to(p)
            }
        }

        impl From<$to> for $from {
            fn from(p: $to) -> $from {
                p.0
            }
        }

        impl PartialEq<$from> for $to {
            fn eq(&self, other: &$from) -> bool {
                self.0.eq(other)
            }
            fn ne(&self, other: &$from) -> bool {
                self.0.ne(other)
            }
        }
    )
}

macro_rules! implement_base58_wrapper {
    ($from:ident, $to:ident) => (
        impl ToBase58 for $to {
            fn base58_layout(&self) -> Vec<u8> {
                self.0.base58_layout()
            }
        }

        impl FromBase58 for $to {
            fn from_base58_layout(data: Vec<u8>) -> Result<$to, FromBase58Error> {
                $from::from_base58_layout(data).map(|x| $to(x))
            }
        }

        impl fmt::Debug for $to {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "\"{}({})\"", stringify!($to), self.to_base58check())
            }
        }
    )
}

implement_wrapper! {Sha256dHash, TxId}
implement_wrapper! {RawPublicKey, PublicKey}
implement_wrapper! {RawAddress, Address}
implement_wrapper! {RawPrivkey, PrivateKey}
implement_wrapper! {RawScript, RedeemScript}
implement_wrapper! {RawTransaction, Transaction}

implement_base58_wrapper! {RawAddress, Address}
implement_base58_wrapper! {RawPrivkey, PrivateKey}

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
        let context = Secp256k1::new();
        let array = self.0.serialize_vec(&context, true);
        ::exonum::crypto::ToHex::to_hex(&array.as_slice())
    }

    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
        let context = Secp256k1::new();
        let bytes = Vec::<u8>::from_hex(v.as_ref())?;
        match RawPublicKey::from_slice(&context, bytes.as_ref()) {
            Ok(key) => Ok(PublicKey(key)),
            Err(_) => Err(FromHexError::InvalidHexLength)
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