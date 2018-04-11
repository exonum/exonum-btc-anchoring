// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate exonum;
extern crate exonum_btc_anchoring;
extern crate tempdir;

use std::thread;
use std::env;

use tempdir::TempDir;

use exonum::node::Node;
use exonum::storage::{RocksDB, DbOptions};
use exonum::helpers::{generate_testnet_config, init_logger};

use exonum_btc_anchoring::{gen_anchoring_testnet_config, AnchoringRpcConfig, AnchoringService,
                           BitcoinNetwork, RpcClient};

fn main() {
    // Init crypto engine and pretty logger.
    exonum::crypto::init();
    init_logger().unwrap();

    // Get rpc config from env variables
    let rpc_config = AnchoringRpcConfig {
        host: env::var("ANCHORING_RELAY_HOST")
            .expect("Env variable ANCHORING_RELAY_HOST needs to be set")
            .parse()
            .unwrap(),
        username: env::var("ANCHORING_USER").ok(),
        password: env::var("ANCHORING_PASSWORD").ok(),
    };

    // Blockchain params
    let count = 4;
    // Inner exonum network start port (4000, 4001, 4002, ..)
    let start_port = 4000;
    let total_funds = 10_000;
    let tmpdir_handle = TempDir::new("exonum_anchoring").unwrap();
    let destdir = tmpdir_handle.path();

    // Generate blockchain configuration
    let client = RpcClient::from(rpc_config.clone());
    let (anchoring_common, anchoring_nodes) =
        gen_anchoring_testnet_config(&client, BitcoinNetwork::Testnet, count, total_funds);
    let node_cfgs = generate_testnet_config(count, start_port);

    // Create testnet threads
    let node_threads = {
        let mut node_threads = Vec::new();
        for idx in 0..count as usize {
            // Create anchoring service for node[idx]
            let service =
                AnchoringService::new(anchoring_common.clone(), anchoring_nodes[idx].clone());
            // Create database for node[idx]
            let db = {
                let mut options = DbOptions {
                    create_if_missing: true,
                    ..DbOptions::default()
                };
                let path = destdir.join(idx.to_string());                
                RocksDB::open(&path, &options).expect("Unable to create database")
            };
            // Create node[idx]
            let node_cfg = node_cfgs[idx].clone();
            let node_thread = thread::spawn(move || {
                // Run it in separate thread
                let node = Node::new(db, vec![Box::new(service)], node_cfg);
                node.run_handler().expect("Unable to run node");
            });
            node_threads.push(node_thread);
        }
        node_threads
    };

    for node_thread in node_threads {
        node_thread.join().unwrap();
    }
}
