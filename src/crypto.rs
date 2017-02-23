use std::ops::Deref;

use bitcoin::util::hash::Sha256dHash;

use exonum::crypto::{HexValue, FromHexError};

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

impl HexValue for TxId {
    fn to_hex(&self) -> String {
        let mut bytes = self.0[..].to_vec();
        bytes.reverse(); // FIXME what about big endianless architectures?
        bytes.to_hex()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> Result<Self, FromHexError> {
        match Sha256dHash::from_hex(v.as_ref()) {
            Ok(hash) => Ok(TxId::from(hash)),
            Err(_) => Err(FromHexError::InvalidHexLength)
        }
        
    }
}