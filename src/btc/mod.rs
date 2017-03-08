mod types;
mod redeem_script;
mod address;
pub mod regtest;

use exonum::crypto::FromHexError;

pub use self::types::{Address, PrivateKey, PublicKey, TxId, RedeemScript, Transaction,
                      RawTransaction};

pub trait HexValueEx: Sized {
    fn to_hex(&self) -> String;
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError>;
}
