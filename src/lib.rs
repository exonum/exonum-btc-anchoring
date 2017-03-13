extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate bitcoinrpc;
#[macro_use]
extern crate exonum;
extern crate bitcoin;
extern crate secp256k1;
extern crate byteorder;
#[macro_use]
extern crate log;
#[cfg(test)]
extern crate env_logger;
#[macro_use]
extern crate derive_error;

extern crate rand;
extern crate tempdir;

#[macro_use]
mod macros;

mod service;
pub mod config;
pub mod transactions;
pub mod client;
pub mod btc;
pub mod error;

#[cfg(feature="sandbox_tests")]
pub mod sandbox;
#[cfg(test)]
mod tests;

use bitcoin::blockdata::script::{Script, Builder};
use bitcoin::network::constants::Network;

use exonum::crypto::{FromHexError, ToHex, FromHex};

use btc::HexValueEx;
pub use service::{AnchoringService, AnchoringHandler, collect_signatures};
pub use service::schema::{AnchoringSchema, ANCHORING_SERVICE, MsgAnchoringSignature,
                          MsgAnchoringUpdateLatest};
pub use transactions::{AnchoringTx, FundingTx, BitcoinTx, TxKind};
pub use client::{AnchoringRpc, RpcClient};
pub use btc::RedeemScript;

pub const SATOSHI_DIVISOR: f64 = 100_000_000.0;
// TODO add feature for bitcoin network
pub const BITCOIN_NETWORK: Network = Network::Testnet;

pub type BitcoinSignature = Vec<u8>;

impl HexValueEx for Script {
    fn to_hex(&self) -> String {
        self.clone().into_vec().to_hex()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError> {
        let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
        Ok(Builder::from(bytes).into_script())
    }
}

pub fn majority_count(cnt: u8) -> u8 {
    cnt * 2 / 3 + 1
}
