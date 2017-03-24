//! # Introduction
//!
//! Private blockchain infrastructure necessitates additional measures for
//! accountability of the blockchain validators.
//! In public PoW blockchains (e.g., Bitcoin), accountability is purely economic
//! and is based on game theory and equivocation or retroactive modifications being economically costly.
//! Not so in private blockchains, where these two behaviors
//! are a real threat per any realistic threat model that assumes
//! that the blockchain is of use not only to the system validators,
//! but also to third parties.
//!
//! This crate implements a protocol for blockchain anchoring onto the Bitcoin Blockchain
//! that utilizes the native Bitcoin capabilities of creating multisig(p2sh) transactions.
//! This transactions contains metadata from exonum blockchain (block's hash on corresponding height)
//! and forms a chain.
//!
//! Anchors produced using threshold ECDSA signatures.
//! To create a threshold signature, the validators initiate a Byzantine fault-tolerant computation
//! which results in a single ECDSA signature over the predetermined message keyed
//! by a public key which may be deterministically computed in advance based on public keys of the validators.
//! 
//! You can read the details in [specification](http://exonum.com/doc/anchoring-spec)
//!
//! # Examples
//!
//! Run testnet in the single process
//!
//! ```rust,no_run
//! extern crate exonum;
//! extern crate anchoring_service;
//! extern crate blockchain_explorer;
//! extern crate tempdir;
//!
//! use std::thread;
//! use std::env;
//!
//! use tempdir::TempDir;
//!
//! use exonum::blockchain::Blockchain;
//! use exonum::node::Node;
//! use exonum::storage::{LevelDB, LevelDBOptions};
//! use blockchain_explorer::helpers::generate_testnet_config;
//! use anchoring_service::{AnchoringRpcConfig, AnchoringRpc, AnchoringService, BitcoinNetwork,
//!                         gen_anchoring_testnet_config};
//!
//! fn main() {
//!     // Init crypto engine and pretty logger.
//!     exonum::crypto::init();
//!     blockchain_explorer::helpers::init_logger().unwrap();
//!
//!     // Get rpc config from env variables
//!     let rpc_config = AnchoringRpcConfig {
//!         host: env::var("ANCHORING_HOST")
//!             .expect("Env variable ANCHORING_HOST needs to be setted")
//!             .parse()
//!             .unwrap(),
//!         username: env::var("ANCHORING_USER").ok(),
//!         password: env::var("ANCHORING_PASSWORD").ok(),
//!     };
//!
//!     // Blockchain params
//!     let count = 4;
//!     let start_port = 4000;
//!     let total_funds = 10000;
//!     let tmpdir_handle = TempDir::new("exonum_anchoring").unwrap();
//!     let destdir = tmpdir_handle.path();
//!
//!     // Generate blockchain configuration
//!     let client = AnchoringRpc::new(rpc_config.clone());
//!     let (anchoring_genesis, anchoring_nodes) =
//!         gen_anchoring_testnet_config(&client, BitcoinNetwork::Testnet, count, total_funds);
//!     let node_cfgs = generate_testnet_config(count, start_port);
//!
//!     // Create testnet threads
//!     let node_threads = {
//!         let mut node_threads = Vec::new();
//!         for idx in 0..count as usize {
//!             // Create anchoring service for node[idx]
//!             let service = AnchoringService::new(AnchoringRpc::new(rpc_config.clone()),
//!                                                 anchoring_genesis.clone(),
//!                                                 anchoring_nodes[idx].clone());
//!             // Create database for node[idx]
//!             let db = {
//!                 let mut options = LevelDBOptions::new();
//!                 let path = destdir.join(idx.to_string());
//!                 options.create_if_missing = true;
//!                 LevelDB::new(&path, options).expect("Unable to create database")
//!             };
//!             // Create node[idx]
//!             let blockchain = Blockchain::new(db, vec![Box::new(service)]);
//!             let mut node = Node::new(blockchain, node_cfgs[idx].clone());
//!             let node_thread = thread::spawn(move || {
//!                                                 // Run it in separate thread
//!                                                 node.run().expect("Unable to run node");
//!                                             });
//!             node_threads.push(node_thread);
//!         }
//!         node_threads
//!     };
//!
//!     for node_thread in node_threads {
//!         node_thread.join().unwrap();
//!     }
//! }
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
pub use btc::{Network as BitcoinNetwork, gen_btc_keypair, gen_btc_keypair_with_rng};
pub use client::AnchoringRpc;
pub use service::{AnchoringService, AnchoringHandler};
pub use service::schema::{AnchoringSchema, ANCHORING_SERVICE, MsgAnchoringSignature,
                          MsgAnchoringUpdateLatest};
pub use service::config::{AnchoringConfig, AnchoringNodeConfig, AnchoringRpcConfig,
                          gen_anchoring_testnet_config_with_rng, gen_anchoring_testnet_config};
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
