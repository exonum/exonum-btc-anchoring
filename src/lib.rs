//! # Introduction
//!
//! Private blockchain infrastructure necessitates additional measures for
//! accountability of the blockchain validators.
//! In public `PoW` blockchains (e.g., `Bitcoin`), accountability is purely economic and is
//! based on game theory and equivocation or retroactive modifications being economically costly.
//! Not so in private blockchains, where these two behaviors
//! are a real threat per any realistic threat model that assumes
//! that the blockchain is of use not only to the system validators,
//! but also to third parties.
//!
//! This crate implements a protocol for blockchain anchoring onto the `Bitcoin` blockchain
//! that utilizes the native `Bitcoin` capabilities of creating multisig([p2sh][1]) transactions.
//! This transactions contains metadata from `Exonum` blockchain (block's hash on corresponding
//! height) and forms a chain.
//!
//! You can read the details in [specification][2].
//!
//! [1]: https://bitcoin.org/en/glossary/p2sh-multisig
//! [2]: http://exonum.com/doc/anchoring-spec
//!
//! # Examples
//!
//! Run testnet in the single process
//!
//! ```rust,no_run
//! extern crate exonum;
//! extern crate anchoring_btc_service;
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
//! use exonum::helpers::{generate_testnet_config, init_logger};
//! use anchoring_btc_service::{AnchoringRpcConfig, AnchoringRpc, AnchoringService, BitcoinNetwork,
//!                             gen_anchoring_testnet_config};
//!
//! fn main() {
//!     // Init crypto engine and pretty logger.
//!     exonum::crypto::init();
//!     init_logger().unwrap();
//!
//!     // Get rpc config from env variables
//!     let rpc_config = AnchoringRpcConfig {
//!         host: env::var("ANCHORING_RELAY_HOST")
//!             .expect("Env variable ANCHORING_RELAY_HOST needs to be setted")
//!             .parse()
//!             .unwrap(),
//!         username: env::var("ANCHORING_USER").ok(),
//!         password: env::var("ANCHORING_PASSWORD").ok(),
//!     };
//!
//!     // Blockchain params
//!     let count = 4;
//!     // Inner exonum network start port (4000, 4001, 4002, ..)
//!     let start_port = 4000;
//!     let total_funds = 10000;
//!     let tmpdir_handle = TempDir::new("exonum_anchoring").unwrap();
//!     let destdir = tmpdir_handle.path();
//!
//!     // Generate blockchain configuration
//!     let client = AnchoringRpc::new(rpc_config.clone());
//!     let (anchoring_common, anchoring_nodes) =
//!         gen_anchoring_testnet_config(&client, BitcoinNetwork::Testnet, count, total_funds);
//!     let node_cfgs = generate_testnet_config(count, start_port);
//!
//!     // Create testnet threads
//!     let node_threads = {
//!         let mut node_threads = Vec::new();
//!         for idx in 0..count as usize {
//!             // Create anchoring service for node[idx]
//!             let service = AnchoringService::new(anchoring_common.clone(),
//!                                                 anchoring_nodes[idx].clone());
//!             // Create database for node[idx]
//!             let db = {
//!                 let mut options = LevelDBOptions::new();
//!                 let path = destdir.join(idx.to_string());
//!                 options.create_if_missing = true;
//!                 LevelDB::new(&path, options).expect("Unable to create database")
//!             };
//!             // Create node[idx]
//!            let blockchain = Blockchain::new(db, vec![Box::new(service)]);
//!            let node_cfg = node_cfgs[idx].clone();
//!            let node_thread = thread::spawn(move || {
//!                                                // Run it in separate thread
//!                                                let mut node = Node::new(blockchain, node_cfg);
//!                                                node.run_handler()
//!                                                    .expect("Unable to run node");
//!                                            });
//!            node_threads.push(node_thread);
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
#[macro_use]
extern crate derive_error;

extern crate rand;
extern crate tempdir;

#[doc(hidden)]
pub mod details;
#[doc(hidden)]
pub mod blockchain;
#[doc(hidden)]
pub mod local_storage;
#[doc(hidden)]
pub mod service;
#[doc(hidden)]
pub mod handler;
#[doc(hidden)]
pub mod error;

pub use details::btc::{Network as BitcoinNetwork, gen_btc_keypair, gen_btc_keypair_with_rng};
pub use details::rpc::{AnchoringRpc, AnchoringRpcConfig};
pub use blockchain::consensus_storage::AnchoringConfig;
pub use local_storage::AnchoringNodeConfig;
pub use service::{ANCHORING_SERVICE_ID, AnchoringService, gen_anchoring_testnet_config,
                  gen_anchoring_testnet_config_with_rng};
pub use handler::AnchoringHandler;
pub use error::Error;

pub fn majority_count(cnt: u8) -> u8 {
    cnt * 2 / 3 + 1
}
