extern crate exonum;
extern crate anchoring_btc_service;
extern crate tempdir;

use std::thread;
use std::env;

use tempdir::TempDir;

use exonum::blockchain::Blockchain;
use exonum::node::Node;
use exonum::storage::{LevelDB, LevelDBOptions};
use exonum::helpers::{generate_testnet_config, init_logger};

use anchoring_btc_service::{AnchoringRpc, AnchoringRpcConfig, AnchoringService, BitcoinNetwork,
                            gen_anchoring_testnet_config};

fn main() {
    // Init crypto engine and pretty logger.
    exonum::crypto::init();
    init_logger().unwrap();

    // Get rpc config from env variables
    let rpc_config = AnchoringRpcConfig {
        host: env::var("ANCHORING_RELAY_HOST")
            .expect("Env variable ANCHORING_RELAY_HOST needs to be setted")
            .parse()
            .unwrap(),
        username: env::var("ANCHORING_USER").ok(),
        password: env::var("ANCHORING_PASSWORD").ok(),
    };

    // Blockchain params
    let count = 4;
    // Inner exonum network start port (4000, 4001, 4002, ..)
    let start_port = 4000;
    let total_funds = 10000;
    let tmpdir_handle = TempDir::new("exonum_anchoring").unwrap();
    let destdir = tmpdir_handle.path();

    // Generate blockchain configuration
    let client = AnchoringRpc::new(rpc_config.clone());
    let (anchoring_common, anchoring_nodes) =
        gen_anchoring_testnet_config(&client, BitcoinNetwork::Testnet, count, total_funds);
    let node_cfgs = generate_testnet_config(count, start_port);

    // Create testnet threads
    let node_threads = {
        let mut node_threads = Vec::new();
        for idx in 0..count as usize {
            // Create anchoring service for node[idx]
            let service = AnchoringService::new(anchoring_common.clone(),
                                                anchoring_nodes[idx].clone());
            // Create database for node[idx]
            let db = {
                let mut options = LevelDBOptions::new();
                let path = destdir.join(idx.to_string());
                options.create_if_missing = true;
                LevelDB::new(&path, options).expect("Unable to create database")
            };
            // Create node[idx]
            let blockchain = Blockchain::new(db, vec![Box::new(service)]);
            let node_cfg = node_cfgs[idx].clone();
            let node_thread = thread::spawn(move || {
                                                // Run it in separate thread
                                                let mut node = Node::new(blockchain, node_cfg);
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
