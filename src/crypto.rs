use std::ops::Deref;
use std::fmt;

use bitcoin::util::hash::Sha256dHash;
use bitcoin::util::address::{Privkey, Address};
use bitcoin::util::base58::{FromBase58, ToBase58, Error as FromBase58Error};
use secp256k1::key::PublicKey;
use secp256k1::Secp256k1;

use exonum::crypto::{HexValue, FromHexError, ToHex};
use exonum::messages::Field;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TxId(Sha256dHash);
#[derive(Clone, PartialEq)]
pub struct BitcoinPrivateKey(pub Privkey);
#[derive(Debug, Clone, PartialEq)]
pub struct BitcoinPublicKey(pub PublicKey);
#[derive(Clone, PartialEq)]
pub struct BitcoinAddress(pub Address);

implement_wrapper! {Sha256dHash, TxId}
implement_wrapper! {PublicKey, BitcoinPublicKey}
implement_wrapper! {Address, BitcoinAddress}
implement_wrapper! {Privkey, BitcoinPrivateKey}

implement_base58_wrapper! {Address, BitcoinAddress}
implement_base58_wrapper! {Privkey, BitcoinPrivateKey}

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

impl HexValue for BitcoinPublicKey {
    fn to_hex(&self) -> String {
        let context = Secp256k1::new();
        self.0.serialize_vec(&context, true).to_hex()
    }

    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
        let context = Secp256k1::new();
        let bytes = Vec::<u8>::from_hex(v.as_ref())?;
        match PublicKey::from_slice(&context, bytes.as_ref()) {
            Ok(key) => Ok(BitcoinPublicKey(key)),
            Err(_) => Err(FromHexError::InvalidHexLength)
        }
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