use serde_json::value::ToJson;
use bitcoin::util::base58::{FromBase58, ToBase58};

use exonum::messages::Message;
use exonum::crypto::HexValue;

use sandbox::sandbox_tests_helper::{SandboxState, add_one_height_with_transactions};

use anchoring_service::sandbox::{SandboxClient, Request};
use anchoring_service::btc;

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
    anchoring_sandbox(&[]);
}

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
    let (_, signatures) = anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                         10,
                                                         sandbox.last_hash(),
                                                         &[funds],
                                                         &btc::Address::from_base58check("2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu").unwrap());

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

    let lost_txid = anchoring_state.latest_anchored_tx().id();
    let txs = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &prev_anchored_tx, &lost_txid)
                .raw()
                .clone()
        })
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

#[test]
fn test_anchoring_second_block_transfer_config() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let second_addr = "2NDp1mLnHacJSVRw6nQMJVLisn7daJ2zzMo";
    let second_keys = [(second_addr,
                        vec!["cRUKB8Nrhxwd5Rh6rcX3QK1h7FosYPw5uzEsuPpzLcDNErZCzSaj",
                             "cMk66oMazTgquBVaBLHzDi8FMgAaRN3tSf6iZykf9bCh3D3FsLX1",
                             "cT2S5KgUQJ41G6RnakJ2XcofvoxK68L9B44hfFTnH4ddygaxi7rc",
                             "cVC9eJN5peJemWn1byyWcWDevg6xLNXtACjHJWmrR5ynsCu8mkQE"])];

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&second_keys);
    let sandbox_state = SandboxState::new();

    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);

    let (tx, following_cfg) = {
        let mut cfg = anchoring_state.genesis.clone();
        cfg.validators.swap(0, 3);
        (gen_update_config_tx(&sandbox, 15, cfg.clone()), cfg)
    };

    // Check insufficient confirmations case
    let anchored_tx = anchoring_state.latest_anchored_tx().clone();
    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 1],
            response: {
                "hash":&anchored_tx.txid(),"hex":&anchored_tx.to_hex(),"confirmations": 10,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx.raw().clone()]);

    // Check enough confirmations case
    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 1],
            response: {
                "hash":&anchored_tx.txid(),"hex":&anchored_tx.to_hex(),"confirmations": 100,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        },
        request! {
            method: "listunspent",
            params: [0, 9999999, ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"]],
            response: [
                {
                    "txid": &anchored_tx.txid(),
                    "vout": 0,
                    "address": "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 100,
                    "spendable": false,
                    "solvable": false
                }
            ]
        }
    ]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                          0,
                                          anchored_tx.payload().1,
                                          &[],
                                          &following_multisig.1);
    let transfer_tx = anchoring_state.latest_anchored_tx().clone();

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    sandbox.broadcast(signatures[0].clone());

    let iter = signatures.into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();

    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&transfer_tx.txid(), 1],
            response: {
                "hash":&transfer_tx.txid(),"hex":&transfer_tx.to_hex(),"confirmations": 0,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        }
    ]);

    add_one_height_with_transactions(&sandbox, &sandbox_state, &iter);

    let lects = (0..4)
        .map(|id| gen_service_tx_lect(&sandbox, id, &transfer_tx, &anchored_tx.id()).raw().clone())
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);

    let block_hash = sandbox.last_hash();

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                {
                    "txid": &transfer_tx.txid(),
                    "vout": 0,
                    "address": &following_multisig.1.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 1,
                    "spendable": false,
                    "solvable": false
                }
            ]
        },
                       request! {
            method: "getrawtransaction",
            params: [&transfer_tx.txid(), 0],
            response: &transfer_tx.to_hex()
        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&transfer_tx.txid(), 1],
            response: {
                "hash":&transfer_tx.txid(),"hex":&transfer_tx.to_hex(),"confirmations": 0,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&transfer_tx.txid(), 1],
            response: {
                "hash":&transfer_tx.txid(),"hex":&transfer_tx.to_hex(),"confirmations": 10,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    // Update cfg
    anchoring_state.genesis = following_cfg;

    let (_, signatures) = anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                          10,
                                          block_hash,
                                          &[],
                                          &following_multisig.1);
    let anchored_tx = anchoring_state.latest_anchored_tx();

    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&transfer_tx.txid(), 1],
            response: {
                "hash":&transfer_tx.txid(),"hex":&transfer_tx.to_hex(),"confirmations": 50,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        },
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                {
                    "txid": &transfer_tx.txid(),
                    "vout": 0,
                    "address": &following_multisig.1.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 1,
                    "spendable": false,
                    "solvable": false
                }
            ]
        },
    ]);

    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&transfer_tx.txid(), 1],
            response: {
                "hash":&transfer_tx.txid(),"hex":&transfer_tx.to_hex(),"confirmations": 100,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
            }
    }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures[0..1]);

    let signatures = signatures.into_iter().map(|tx| tx.raw().clone()).collect::<Vec<_>>();
    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&transfer_tx.txid(), 1],
            response: {
                "hash":&transfer_tx.txid(),"hex":&transfer_tx.to_hex(),"confirmations": 100,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
            }
        },
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                {
                    "txid": &transfer_tx.txid(),
                    "vout": 0,
                    "address": &following_multisig.1.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 1,
                    "spendable": false,
                    "solvable": false
                }
            ]
        },
        request! {
            method: "getrawtransaction",
            params: [&transfer_tx.txid(), 0],
            response: &transfer_tx.to_hex()
        },
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 1],
            response: {
                "hash":&anchored_tx.txid(),"hex":&anchored_tx.to_hex(),"confirmations": 0,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
            }
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures[1..]);

    let lects = (0..4)
        .map(|id| gen_service_tx_lect(&sandbox, id, &anchored_tx, &transfer_tx.id()))
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);
}