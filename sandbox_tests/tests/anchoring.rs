#[macro_use]
extern crate exonum;
extern crate sandbox;
extern crate anchoring_service;
#[macro_use]
extern crate anchoring_sandbox;
extern crate serde;
extern crate serde_json;
extern crate bitcoin;
extern crate bitcoinrpc;
extern crate secp256k1;
extern crate blockchain_explorer;
#[macro_use]
extern crate log;

use serde_json::value::ToJson;

use exonum::crypto::HexValue;
use sandbox::sandbox_tests_helper::{SandboxState, add_one_height_with_transactions};

use anchoring_service::sandbox::{Request};
use anchoring_sandbox::{RpcError, anchoring_sandbox};
use anchoring_sandbox::helpers::*;

#[test]
fn test_anchoring_genesis_block() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();
    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
}

#[test]
fn test_anchoring_update_lect_normal() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();

    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);
}

#[test]
fn test_anchoring_update_lect_different() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();
    anchor_first_block_lect_different(&sandbox, &client, &sandbox_state, &mut anchoring_state);
}

#[test]
fn test_anchoring_first_block_lect_lost() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();
    anchor_first_block_lect_lost(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    assert_eq!(anchoring_state.latest_anchored_tx, None);
}

#[test]
fn test_anchoring_second_block_normal() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();
    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_second_block_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);
}

#[test]
fn test_anchoring_second_block_additional_funds() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();

    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"]],
            response: [
                {
                    "txid": "fea0a60f7146e7facf5bb382b80dafb762175bf0d4b6ac4e59c09cd4214d1491",
                    "vout": 0,
                    "address": "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 1,
                    "spendable": false,
                    "solvable": false
                },
                {
                    "txid": "a03b10b17fc8b86dd0b1b6ebcc3bc3c6dd4b7173302ef68628f5ed768dbd7049",
                    "vout": 0,
                    "address": "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 75,
                    "spendable": false,
                    "solvable": false
                }
            ]
        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    let funds = anchoring_state.genesis.funding_tx.clone();
    let (_, anchoring_addr) = anchoring_state.genesis.redeem_script();
    let (_, signatures) = anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                                           10,
                                                                           block_hash_on_height(&sandbox, 10),
                                                                           &[funds],
                                                                           &anchoring_addr);

    sandbox.broadcast(signatures[0].clone());
    sandbox.broadcast(signatures[1].clone());

    let anchored_tx = anchoring_state.latest_anchored_tx();
    client.expect(vec![Request {
                           method: "getrawtransaction",
                           params: vec![anchored_tx.txid().to_json(), 1.to_json()],
                           response: Err(RpcError::NoInformation("Unable to find tx".to_string())),
                       },
                       request! {
                           method: "sendrawtransaction",
                           params: [&anchored_tx.to_hex()]
                       }]);

    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures);
    sandbox.broadcast(gen_service_tx_lect(&sandbox, 0, &anchored_tx, 2));
}

#[test]
fn test_anchoring_second_block_lect_lost() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();

    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);

    let prev_anchored_tx = anchoring_state.latest_anchored_tx().clone();
    let prev_tx_signatures = anchoring_state.latest_anchored_tx_signatures().to_vec();

    anchor_second_block_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    for _ in 0..5 {
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    }

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"]],
            response: [
                {
                    "txid": &prev_anchored_tx.txid(),
                    "vout": 0,
                    "address": "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 0,
                    "spendable": false,
                    "solvable": false
                }
            ]
        },
                       request! {
            method: "getrawtransaction",
            params: [&prev_anchored_tx.txid(), 0],
            response: &prev_anchored_tx.to_hex()
        }]);

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    let txs = (0..4)
        .map(|id| gen_service_tx_lect(&sandbox, id, &prev_anchored_tx, 3))
        .collect::<Vec<_>>();
    sandbox.broadcast(txs[0].clone());

    // Trying to resend lost lect tx
    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"]],
            response: [
                {
                    "txid": &prev_anchored_tx.txid(),
                    "vout": 0,
                    "address": "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 0,
                    "spendable": false,
                    "solvable": false
                }
            ]
        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &txs);

    anchoring_state.latest_anchored_tx = Some((prev_anchored_tx, prev_tx_signatures));
}
