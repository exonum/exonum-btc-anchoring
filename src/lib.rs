//! # Exonum anchoring service
//!
//! The part of Exonum blockchain.
//!
//! Simple testnet example
//!
//! # Examples
//! ```
//! extern crate anchoring_service;
//! extern crate configuration_service;
//! ```
//! 

#![crate_type = "lib"]
#![crate_type = "rlib"]
#![crate_type = "dylib"]
#![crate_name = "anchoring_service"]

#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

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

#[doc(hidden)]
/// For test purpose only
pub mod service;
#[doc(hidden)]
/// For test purpose only
pub mod transactions;
#[doc(hidden)]
/// For test purpose only
pub mod client;
#[doc(hidden)]
/// For test purpose only
pub mod btc;
#[doc(hidden)]
/// For test purpose only
pub mod error;

#[cfg(feature="sandbox_tests")]
pub mod sandbox;
#[cfg(test)]
mod tests;

use bitcoin::blockdata::script::{Script, Builder};

use exonum::crypto::{FromHexError, ToHex, FromHex};

use btc::HexValueEx;
pub use btc::{Network, RawNetwork, gen_keypair, gen_keypair_with_rng};
pub use client::AnchoringRpc;
pub use service::{AnchoringService, AnchoringHandler};
pub use service::schema::{AnchoringSchema, ANCHORING_SERVICE, MsgAnchoringSignature,
                          MsgAnchoringUpdateLatest};
pub use service::config::{AnchoringConfig, AnchoringNodeConfig, AnchoringRpcConfig,
                          testnet_generate_anchoring_config_with_rng,
                          testnet_generate_anchoring_config};
pub use error::Error;

const SATOSHI_DIVISOR: f64 = 100_000_000.0;

impl HexValueEx for Script {
    fn to_hex(&self) -> String {
        self.clone().into_vec().to_hex()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError> {
        let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
        Ok(Builder::from(bytes).into_script())
    }
}

/// Returns 2/3+1 of the given number in accordance with the Byzantine fault tolerance  algorithm.
pub fn majority_count(cnt: u8) -> u8 {
    cnt * 2 / 3 + 1
}
