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
use bitcoin::util::base58::ToBase58;

use exonum::messages::Message;
use exonum::crypto::HexValue;
use sandbox::sandbox_tests_helper::{SandboxState, add_one_height_with_transactions};

use anchoring_service::sandbox::Request;
use anchoring_sandbox::anchoring_sandbox;
use anchoring_sandbox::helpers::*;

// We commit a new configuration and take actions to transfer tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_anchoring_transfer_config_normal() {
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
            params: [0, 9999999, ["2NDp1mLnHacJSVRw6nQMJVLisn7daJ2zzMo"]],
            response: []
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

    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures);

    let lects = (0..4)
        .map(|id| gen_service_tx_lect(&sandbox, id, &transfer_tx, 2).raw().clone())
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);
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
                                          block_hash_on_height(&sandbox, 10),
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
        .map(|id| gen_service_tx_lect(&sandbox, id, &anchored_tx, 3))
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);
}

// We commit a new configuration and take actions to transfer tx chain to the new address
// problems: 
//  - we losing transferring tx, but we have time to recovering it
// result: success
#[test]
fn test_anchoring_transfer_config_lost_lect() {
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
        (gen_update_config_tx(&sandbox, 18, cfg.clone()), cfg)
    };

    // Check insufficient confirmations case
    let anchored_tx = anchoring_state.latest_anchored_tx().clone();
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
            params: [0, 9999999, ["2NDp1mLnHacJSVRw6nQMJVLisn7daJ2zzMo"]],
            response: []
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx.raw().clone()]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                          0,
                                          block_hash_on_height(&sandbox, 0),
                                          &[],
                                          &following_multisig.1);
    let transfer_tx = anchoring_state.latest_anchored_tx().clone();

    sandbox.broadcast(signatures[0].clone());
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
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures);

    let lects = (0..4)
        .map(|id| gen_service_tx_lect(&sandbox, id, &transfer_tx, 2).raw().clone())
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: []
        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    let lects = (0..4)
        .map(|id| gen_service_tx_lect(&sandbox, id, &anchored_tx, 3).raw().clone())
        .collect::<Vec<_>>();

    sandbox.broadcast(lects[0].clone());
    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 1],
            response: {
                "hash":&anchored_tx.txid(),"hex":&anchored_tx.to_hex(),"confirmations": 200,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        },
        request! {
            method: "listunspent",
            params: [0, 9999999, ["2NDp1mLnHacJSVRw6nQMJVLisn7daJ2zzMo"]],
            response: []
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);

    // new transfer transaction
    anchoring_state.latest_anchored_tx = Some((anchored_tx.clone(), Vec::new()));
    let (_, signatures) = anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                          10,
                                          block_hash_on_height(&sandbox, 10),
                                          &[],
                                          &following_multisig.1);
    let transfer_tx_2 = anchoring_state.latest_anchored_tx().clone();
    debug!("transfer_tx={:#?}", transfer_tx_2);

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: ["a5573f424f14b19e6a2910f5050527700f9f66f4a979be5d0339703a74e6d81e", 1],
            response: {
                "hash":&transfer_tx_2.txid(),"hex":&transfer_tx_2.to_hex(),"confirmations": 0,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures);

    let lect = gen_service_tx_lect(&sandbox, 0, &transfer_tx_2, 4);
    sandbox.broadcast(lect);
}

// We commit a new configuration and take actions to transfer tx chain to the new address
// problems: 
//  - we losing transferring tx and we have no time to recovering it
// result: wait until someone push transfer tx manually
#[test]
fn test_anchoring_transfer_config_lost_lect_waiting() {
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
        (gen_update_config_tx(&sandbox, 17, cfg.clone()), cfg)
    };

    // Check insufficient confirmations case
    let anchored_tx = anchoring_state.latest_anchored_tx().clone();
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
            params: [0, 9999999, ["2NDp1mLnHacJSVRw6nQMJVLisn7daJ2zzMo"]],
            response: []
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx.raw().clone()]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                          0,
                                          block_hash_on_height(&sandbox, 0),
                                          &[],
                                          &following_multisig.1);
    let transfer_tx = anchoring_state.latest_anchored_tx().clone();

    sandbox.broadcast(signatures[0].clone());
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
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures);

    let lects = (0..4)
        .map(|id| gen_service_tx_lect(&sandbox, id, &transfer_tx, 2).raw().clone())
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: []
        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    let lects = (0..4)
        .map(|id| gen_service_tx_lect(&sandbox, id, &anchored_tx, 3).raw().clone())
        .collect::<Vec<_>>();

    sandbox.broadcast(lects[0].clone());
    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 1],
            response: {
                "hash":&anchored_tx.txid(),"hex":&anchored_tx.to_hex(),"confirmations": 200,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        },
        request! {
            method: "listunspent",
            params: [0, 9999999, ["2NDp1mLnHacJSVRw6nQMJVLisn7daJ2zzMo"]],
            response: []
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);

    // new transfer transaction
    anchoring_state.latest_anchored_tx = Some((anchored_tx.clone(), Vec::new()));
    let (_, signatures) = anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                          10,
                                          block_hash_on_height(&sandbox, 10),
                                          &[],
                                          &following_multisig.1);
    let transfer_tx_2 = anchoring_state.latest_anchored_tx().clone();
    debug!("transfer_tx={:#?}", transfer_tx_2);

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&transfer_tx_2.txid(), 1],
            response: {
                "hash":&transfer_tx_2.txid(),"hex":&transfer_tx_2.to_hex(),"confirmations": 0,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures);

    let lect = gen_service_tx_lect(&sandbox, 0, &transfer_tx_2, 4);
    sandbox.broadcast(lect.clone());
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[lect]);

    // waiting for lect update
    client.expect(vec![Request {
                           method: "getrawtransaction",
                           params: vec![transfer_tx_2.txid().to_json(), 1.to_json()],
                           response: Err(RpcError::NoInformation("Unable to find tx".to_string())),
                       }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    client.expect(vec![Request {
                           method: "getrawtransaction",
                           params: vec![transfer_tx_2.txid().to_json(), 1.to_json()],
                           response: Err(RpcError::NoInformation("Unable to find tx".to_string())),
                       }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    // We lost our transfer_tx_2
    client.expect(vec![Request {
                           method: "getrawtransaction",
                           params: vec![transfer_tx_2.txid().to_json(), 1.to_json()],
                           response: Err(RpcError::NoInformation("Unable to find tx".to_string())),
                       },
                       request! {
                            method: "listunspent",
                            params: [0, 9999999, ["2NDp1mLnHacJSVRw6nQMJVLisn7daJ2zzMo"]],
                            response: []
                        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    let lects = (0..4)
        .map(|id| gen_service_tx_lect(&sandbox, id, &anchored_tx, 5).raw().clone())
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);
    // Wait until someone push transfer tx manually
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);    
}