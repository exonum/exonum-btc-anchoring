use std::ops::Deref;

use bitcoin::util::hash::Sha256dHash;

use exonum::crypto::{HexValue, FromHexError};
use exonum::messages::Field;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TxId(Sha256dHash);

impl TxId {
    pub fn new(bytes: [u8; 32]) -> TxId {
        TxId(Sha256dHash::from_data(bytes.as_ref()))
    }
}

impl From<Sha256dHash> for TxId {
    fn from(hash: Sha256dHash) -> TxId {
        TxId(hash)
    }
}

impl From<TxId> for Sha256dHash {
    fn from(hash: TxId) -> Sha256dHash {
        hash.0
    }
}

impl Deref for TxId {
    type Target = Sha256dHash;

    fn deref(&self) -> &Sha256dHash {
        &self.0
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