use serde_json::value::ToJson;

use exonum::messages::Message;

use sandbox::sandbox_tests_helper::{SandboxState, add_one_height_with_transactions};

use anchoring_service::sandbox::{SandboxClient, Request};
use anchoring_service::HexValue;

use {RpcError, anchoring_sandbox, gen_sandbox_anchoring_config};
use helpers::*;

#[test]
fn test_rpc_getnewaddress() {
    let client = SandboxClient::default();
    client.expect(vec![request! {
                           method: "getnewaddress",
                           params: ["maintain"],
                           response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
                       }]);
    let addr = client.getnewaddress("maintain").unwrap();
    assert_eq!(addr, "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY");
}

#[test]
#[should_panic(expected = "expected response for method=getnewaddress")]
fn test_rpc_expected_request() {
    let client = SandboxClient::default();
    client.getnewaddress("useroid").unwrap();
}

#[test]
#[should_panic(expected = "assertion failed")]
fn test_rpc_wrong_request() {
    let client = SandboxClient::default();
    client.expect(vec![request! {
                           method: "getnewaddress",
                           params: ["maintain"],
                           response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
                       }]);
    client.getnewaddress("useroid").unwrap();
}

#[test]
#[should_panic(expected = "assertion failed")]
fn test_rpc_uneexpected_request() {
    let client = SandboxClient::default();
    client.expect(vec![request! {
                           method: "getnewaddress",
                           params: ["maintain"],
                           response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
                       },
                       request! {
                           method: "getnewaddress",
                           params: ["maintain2"],
                           response: "mmoXxKhBwnhtFiAMvxJ82CKCBia751mzfY"
                       }]);
    client.getnewaddress("useroid").unwrap();
    client.expect(vec![request! {
                           method: "getnewaddress",
                           params: ["maintain"],
                           response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
                       }]);
}

#[test]
fn test_rpc_validateaddress() {
    let client = SandboxClient::default();
    client.expect(vec![request! {
                           method: "validateaddress",
                           params: ["n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb"],
                           response: {
                               "account":"node_0","address":"n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb","hdkeypath":"m/0'/0'/1023'","hdmasterkeyid":"e2aabb596d105e11c1838c0b6bede91e1f2a95ee","iscompressed":true,"ismine":true,"isscript":false,"isvalid":true,"iswatchonly":false,"pubkey":"0394a06ac465776c110cb43d530663d7e7df5684013075988917f02ff007edd364","scriptPubKey":"76a914e7588549f0c4149e7949cd7ea933cfcdde45f8c888ac"
                           }
                       }]);
    client.validateaddress("n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb").unwrap();
}

#[test]
fn test_generate_anchoring_config() {
    let mut client = SandboxClient::default();
    gen_sandbox_anchoring_config(&mut client);
}

#[test]
fn test_anchoring_sandbox() {
    anchoring_sandbox();
}

#[test]
fn test_anchoring_genesis_block() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox();
    let sandbox_state = SandboxState::new();
    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
}

#[test]
fn test_anchoring_update_lect_normal() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox();
    let sandbox_state = SandboxState::new();

    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);
}

#[test]
fn test_anchoring_update_lect_different() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox();
    let sandbox_state = SandboxState::new();
    anchor_first_block_lect_different(&sandbox, &client, &sandbox_state, &mut anchoring_state);
}

#[test]
fn test_anchoring_first_block_lect_lost() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox();
    let sandbox_state = SandboxState::new();
    anchor_first_block_lect_lost(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    assert_eq!(anchoring_state.latest_anchored_tx, None);
}

#[test]
fn test_anchoring_second_block_normal() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox();
    let sandbox_state = SandboxState::new();
    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_second_block_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);
}

#[test]
fn test_anchoring_second_block_additional_funds() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox();
    let sandbox_state = SandboxState::new();

    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 999999, ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"]],
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
    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                         10,
                                                         sandbox.last_hash(),
                                                         &[funds],
                                                         "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                                                         6000);

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

    let signatures = signatures.into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures);

    sandbox.broadcast(gen_service_tx_lect(&sandbox, 0, &anchored_tx, &anchored_tx.prev_hash()));
}

#[test]
fn test_anchoring_second_block_lect_lost() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox();
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

    let lost_txid = anchoring_state.latest_anchored_tx().id();
    let txs = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox,
                                id,
                                &prev_anchored_tx,
                                &lost_txid).raw().clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(txs[0].clone());

    // Trying to resend lost lect tx
    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 999999, ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"]],
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
