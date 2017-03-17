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

use bitcoin::util::base58::ToBase58;

use exonum::messages::{RawTransaction, Message};
use exonum::crypto::HexValue;
use sandbox::sandbox_tests_helper::{SandboxState, add_one_height_with_transactions};
use sandbox::sandbox::Sandbox;

use anchoring_service::sandbox::Request;
use anchoring_service::transactions::FundingTx;
use anchoring_service::config::AnchoringConfig;

use anchoring_sandbox::{CHECK_LECT_FREQUENCY, AnchoringSandboxState, anchoring_sandbox};
use anchoring_sandbox::helpers::*;

fn gen_following_cfg(sandbox: &Sandbox,
                     anchoring_state: &mut AnchoringSandboxState,
                     from_height: u64,
                     funds: Option<FundingTx>)
                     -> (RawTransaction, AnchoringConfig) {
    let (_, anchoring_addr) = anchoring_state.genesis.redeem_script();

    let mut cfg = anchoring_state.genesis.clone();
    let mut priv_keys = anchoring_state.priv_keys(&anchoring_addr);
    cfg.validators.swap(0, 3);
    priv_keys.swap(0, 3);
    if let Some(funds) = funds {
        cfg.funding_tx = funds;
    }

    let following_addr = cfg.redeem_script().1;
    for (id, ref mut node) in anchoring_state.nodes.iter_mut().enumerate() {
        node.private_keys.insert(following_addr.to_base58check(), priv_keys[id].clone());
    }
    anchoring_state.handler().add_private_key(&following_addr, priv_keys[0].clone());
    (gen_update_config_tx(sandbox, from_height, cfg.clone()), cfg)
}

// We commit a new configuration and take actions to transfer tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_anchoring_transfer_config_normal() {
    let _ = ::blockchain_explorer::helpers::init_logger();
    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();

    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);

    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, &mut anchoring_state, 16, None);
    let (_, following_addr) = following_cfg.redeem_script();

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
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[cfg_tx]);

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
            params: [0, 9999999, [following_addr]],
            response: []
        }
    ]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                         0,
                                                         anchored_tx.payload().1,
                                                         &[],
                                                         None,
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
                            method: "importaddress",
                            params: [&following_addr, "multisig", false, false]
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
                        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
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
                                    "confirmations": 0,
                                    "spendable": false,
                                    "solvable": false
                                }
                            ]
                        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    // Update cfg
    anchoring_state.genesis = following_cfg;
    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                         10,
                                                         block_hash_on_height(&sandbox, 10),
                                                         &[],
                                                         None,
                                                         &following_multisig.1);
    let anchored_tx = anchoring_state.latest_anchored_tx();
    sandbox.broadcast(signatures[0].clone());
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures[0..1]);

    let signatures = signatures.into_iter().map(|tx| tx.raw().clone()).collect::<Vec<_>>();
    client.expect(vec![
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

    let lects =
        (0..4).map(|id| gen_service_tx_lect(&sandbox, id, &anchored_tx, 3)).collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![request! {
                            method: "listunspent",
                            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
                            response: [
                                {
                                    "txid": &anchored_tx.txid(),
                                    "vout": 0,
                                    "address": &following_multisig.1.to_base58check(),
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
                            params: [&anchored_tx.txid(), 0],
                            response: &anchored_tx.to_hex()
                        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);
}

// We commit a new configuration with confirmed funding tx
// and take actions to transfer tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_anchoring_transfer_config_with_funding_tx() {
    let _ = ::blockchain_explorer::helpers::init_logger();
    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();

    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);

    let funding_tx = FundingTx::from_hex("01000000019aaf09d7e73a5f9ab394f1358bfb3dbde7b15b983d715f5c98f369a3f0a288a7010000006a473044022025e8ae682e4e681e6819d704edfc9e0d1e9b47eeaf7306f71437b89fd60b7a3502207396e9861df9d6a9481aa7d7cbb1bf03add8a891bead0d07ff942cf82ac104ce01210361ee947a30572b1e9fd92ca6b0dd2b3cc738e386daf1b19321b15cb1ce6f345bfeffffff02e80300000000000017a91476ee0b0e9603920c421f1abbda07623eb0c3f2c287370ed70b000000001976a914c89746247160e12dc7b0b32a5507518a70eabd0a88ac3aae1000").unwrap();
    let (cfg_tx, following_cfg) =
        gen_following_cfg(&sandbox, &mut anchoring_state, 16, Some(funding_tx.clone()));
    let (_, following_addr) = following_cfg.redeem_script();

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
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[cfg_tx]);

    // Check enough confirmations case
    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                         0,
                                                         anchored_tx.payload().1,
                                                         &[],
                                                         None,
                                                         &following_multisig.1);
    let transfer_tx = anchoring_state.latest_anchored_tx().clone();
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
            params: [0, 9999999, [following_addr]],
            response: [
                {
                    "txid": &funding_tx.txid(),
                    "vout": 0,
                    "address": &following_multisig.1.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 0,
                    "spendable": false,
                    "solvable": false
                }
            ]
        }
    ]);
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
                            method: "importaddress",
                            params: [&following_addr, "multisig", false, false]
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
                                },
                                {
                                    "txid": &funding_tx.txid(),
                                    "vout": 0,
                                    "address": &following_multisig.1.to_base58check(),
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
                            params: [&transfer_tx.txid(), 0],
                            response: &transfer_tx.to_hex()
                        },
                       request! {
                            method: "getrawtransaction",
                            params: [&funding_tx.txid(), 0],
                            response: &funding_tx.to_hex()
                        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
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
                                    "confirmations": 0,
                                    "spendable": false,
                                    "solvable": false
                                },
                                {
                                    "txid": &funding_tx.txid(),
                                    "vout": 0,
                                    "address": &following_multisig.1.to_base58check(),
                                    "account": "multisig",
                                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                                    "amount": 0.00010000,
                                    "confirmations": 0,
                                    "spendable": false,
                                    "solvable": false
                                }
                            ]
                        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    // Update cfg
    anchoring_state.genesis = following_cfg;
    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                         10,
                                                         block_hash_on_height(&sandbox, 10),
                                                         &[funding_tx.clone()],
                                                         None,
                                                         &following_multisig.1);
    let anchored_tx = anchoring_state.latest_anchored_tx();
    sandbox.broadcast(signatures[0].clone());
    sandbox.broadcast(signatures[1].clone());
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures[0..2]);

    let signatures = signatures.into_iter().map(|tx| tx.raw().clone()).collect::<Vec<_>>();
    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 1],
            response: {
                "hash":&anchored_tx.txid(),"hex":&anchored_tx.to_hex(),"confirmations": 0,
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
            }
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures[2..]);

    let lects =
        (0..4).map(|id| gen_service_tx_lect(&sandbox, id, &anchored_tx, 3)).collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![request! {
                            method: "listunspent",
                            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
                            response: [
                                {
                                    "txid": &anchored_tx.txid(),
                                    "vout": 0,
                                    "address": &following_multisig.1.to_base58check(),
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
                            params: [&anchored_tx.txid(), 0],
                            response: &anchored_tx.to_hex()
                        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);
}

// We commit a new configuration and take actions to transfer tx chain to the new address
// problems:
//  - we losing transferring tx, but we have time to recovering it
// result: success
#[test]
fn test_anchoring_transfer_config_lost_lect_recover() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();

    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);

    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, &mut anchoring_state, 17, None);
    let (_, following_addr) = following_cfg.redeem_script();

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
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: []
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[cfg_tx]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                         0,
                                                         block_hash_on_height(&sandbox, 0),
                                                         &[],
                                                         None,
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
                            method: "importaddress",
                            params: [&following_addr, "multisig", false, false]
                        },
                       request! {
                            method: "listunspent",
                            params: [0, 9999999, [&following_addr.to_base58check()]],
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
            params: [0, 9999999, [following_addr.to_base58check()]],
            response: []
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);

    // new transfer transaction
    anchoring_state.latest_anchored_tx = Some((anchored_tx.clone(), Vec::new()));
    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                         10,
                                                         block_hash_on_height(&sandbox, 10),
                                                         &[],
                                                         None,
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
}

// We commit a new configuration and take actions to transfer tx chain to the new address
// problems:
//  - we losing transferring tx and we have no time to recovering it
// result: we trying to resend tx
#[test]
fn test_anchoring_transfer_config_lost_lect_recover_after_cfg_change() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();

    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);

    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, &mut anchoring_state, 14, None);
    let (_, following_addr) = following_cfg.redeem_script();

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
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: []
        }
    ]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[cfg_tx]);

    let previous_anchored_tx = anchoring_state.latest_anchored_tx().clone();
    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                         0,
                                                         block_hash_on_height(&sandbox, 0),
                                                         &[],
                                                         None,
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
                            method: "importaddress",
                            params: [&following_addr, "multisig", false, false]
                        },
                       request! {
                            method: "listunspent",
                            params: [0, 9999999, [&following_addr.to_base58check()]],
                            response: []
                        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    let prev_lects = (0..4)
        .map(|id| gen_service_tx_lect(&sandbox, id, &previous_anchored_tx, 3).raw().clone())
        .collect::<Vec<_>>();
    sandbox.broadcast(prev_lects[0].clone());
    add_one_height_with_transactions(&sandbox, &sandbox_state, &prev_lects[0..1]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &prev_lects[1..]);

    for _ in 0..3 {
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    }

    client.expect(vec![request! {
                            method: "listunspent",
                            params: [0, 9999999, [following_addr.to_base58check()]],
                            response: []
                        },
                       request! {
                            method: "sendrawtransaction",
                            params: [&transfer_tx.to_hex()]
                        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    for _ in 1..CHECK_LECT_FREQUENCY {
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    }

    client.expect(vec![request! {
                            method: "listunspent",
                            params: [0, 9999999, [following_addr.to_base58check()]],
                            response: [
                                {
                                    "txid": &transfer_tx.txid(),
                                    "vout": 0,
                                    "address": &following_multisig.1.to_base58check(),
                                    "account": "multisig",
                                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                                    "amount": 0.00010000,
                                    "confirmations": 3,
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

    let lects = (0..4)
        .map(|id| gen_service_tx_lect(&sandbox, id, &transfer_tx, 4).raw().clone())
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
    client.expect(vec![request! {
                            method: "listunspent",
                            params: [0, 9999999, [following_addr.to_base58check()]],
                            response: [
                                {
                                    "txid": &transfer_tx.txid(),
                                    "vout": 0,
                                    "address": &following_multisig.1.to_base58check(),
                                    "account": "multisig",
                                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                                    "amount": 0.00010000,
                                    "confirmations": 3,
                                    "spendable": false,
                                    "solvable": false
                                }
                            ]
                        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &lects);

    // Update cfg
    anchoring_state.genesis = following_cfg;
    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                         20,
                                                         block_hash_on_height(&sandbox, 20),
                                                         &[],
                                                         None,
                                                         &following_multisig.1);
    sandbox.broadcast(signatures[0].clone());
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures[0..1]);
}

// We do not commit a new configuration
// problems:
//  - We have no time to create transferring transaction
// result: we create a new anchoring tx chain from scratch
#[test]
fn test_anchoring_transfer_config_lost_lect_new_tx_chain() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox(&[]);
    let sandbox_state = SandboxState::new();

    anchor_first_block(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    anchor_first_block_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);

    let funding_tx = FundingTx::from_hex("01000000019aaf09d7e73a5f9ab394f1358bfb3dbde7b15b983d715f5c98f369a3f0a288a7010000006a473044022025e8ae682e4e681e6819d704edfc9e0d1e9b47eeaf7306f71437b89fd60b7a3502207396e9861df9d6a9481aa7d7cbb1bf03add8a891bead0d07ff942cf82ac104ce01210361ee947a30572b1e9fd92ca6b0dd2b3cc738e386daf1b19321b15cb1ce6f345bfeffffff02e80300000000000017a91476ee0b0e9603920c421f1abbda07623eb0c3f2c287370ed70b000000001976a914c89746247160e12dc7b0b32a5507518a70eabd0a88ac3aae1000").unwrap();
    let (cfg_tx, following_cfg) =
        gen_following_cfg(&sandbox, &mut anchoring_state, 11, Some(funding_tx.clone()));
    let (_, following_addr) = following_cfg.redeem_script();

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
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[cfg_tx]);

    for _ in 0..2 {
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
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    }
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    let previous_anchored_tx = anchoring_state.latest_anchored_tx().clone();
    let following_multisig = following_cfg.redeem_script();

    // Update cfg
    anchoring_state.genesis = following_cfg;
    anchoring_state.latest_anchored_tx = None;
    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(&sandbox,
                                                         10,
                                                         block_hash_on_height(&sandbox, 10),
                                                         &[],
                                                         Some(previous_anchored_tx.id()),
                                                         &following_multisig.1);
    client.expect(vec![request! {
                            method: "importaddress",
                            params: [&following_addr, "multisig", false, false]
                        },
                       request! {
                            method: "listunspent",
                            params: [0, 9999999, [&following_addr.to_base58check()]],
                            response: [
                                {
                                    "txid": &funding_tx.txid(),
                                    "vout": 0,
                                    "address": &following_multisig.1.to_base58check(),
                                    "account": "multisig",
                                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                                    "amount": 0.00010000,
                                    "confirmations": 200,
                                    "spendable": false,
                                    "solvable": false
                                }
                            ]
                        },
                       request! {
                            method: "getrawtransaction",
                            params: [&funding_tx.txid(), 0],
                            response: &funding_tx.to_hex()
                        },
                       request! {
                            method: "listunspent",
                            params: [0, 9999999, [&following_addr.to_base58check()]],
                            response: [
                                {
                                    "txid": &funding_tx.txid(),
                                    "vout": 0,
                                    "address": &following_multisig.1.to_base58check(),
                                    "account": "multisig",
                                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                                    "amount": 0.00010000,
                                    "confirmations": 200,
                                    "spendable": false,
                                    "solvable": false
                                }
                            ]
                        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    sandbox.broadcast(signatures[0].clone());
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures[0..1]);
}
