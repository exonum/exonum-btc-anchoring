extern crate exonum;
extern crate sandbox;
extern crate anchoring_btc_service;
#[macro_use]
extern crate anchoring_btc_sandbox;
extern crate serde;
extern crate serde_json;
extern crate bitcoin;
extern crate bitcoinrpc;
extern crate secp256k1;
extern crate rand;
#[macro_use]
extern crate log;

use bitcoin::util::base58::ToBase58;
use bitcoin::network::constants::Network;
use rand::{SeedableRng, StdRng};
use serde_json::value::ToJson;

use exonum::messages::{Message, RawTransaction};
use exonum::crypto::HexValue;
use exonum::storage::StorageValue;
use sandbox::config_updater::TxConfig;

use anchoring_btc_service::details::sandbox::Request;
use anchoring_btc_service::details::btc::transactions::{FundingTx, TransactionBuilder};
use anchoring_btc_service::details::btc;
use anchoring_btc_service::{ANCHORING_SERVICE_ID, AnchoringConfig};

use anchoring_btc_sandbox::AnchoringSandbox;
use anchoring_btc_sandbox::helpers::*;

fn gen_following_cfg(sandbox: &AnchoringSandbox,
                     from_height: u64,
                     funds: Option<FundingTx>)
                     -> (RawTransaction, AnchoringConfig) {
    let anchoring_addr = sandbox.current_addr();

    let mut cfg = sandbox.current_cfg().clone();
    let mut priv_keys = sandbox.priv_keys(&anchoring_addr);
    cfg.validators.swap(0, 3);
    priv_keys.swap(0, 3);
    if let Some(funds) = funds {
        cfg.funding_tx = Some(funds);
    }

    let following_addr = cfg.redeem_script().1;
    for (id, ref mut node) in sandbox.nodes_mut().iter_mut().enumerate() {
        node.private_keys
            .insert(following_addr.to_base58check(), priv_keys[id].clone());
    }

    sandbox
        .handler()
        .add_private_key(&following_addr, priv_keys[0].clone());
    (gen_update_config_tx(sandbox, from_height, cfg.clone()), cfg)
}

fn gen_following_cfg_unchanged_self_key(sandbox: &AnchoringSandbox,
                                        from_height: u64,
                                        funds: Option<FundingTx>)
                                        -> (RawTransaction, AnchoringConfig) {
    let anchoring_addr = sandbox.current_addr();

    let mut cfg = sandbox.current_cfg().clone();
    let mut priv_keys = sandbox.priv_keys(&anchoring_addr);
    cfg.validators.swap(1, 2);
    priv_keys.swap(1, 2);
    if let Some(funds) = funds {
        cfg.funding_tx = Some(funds);
    }

    let following_addr = cfg.redeem_script().1;
    for (id, ref mut node) in sandbox.nodes_mut().iter_mut().enumerate() {
        node.private_keys
            .insert(following_addr.to_base58check(), priv_keys[id].clone());
    }
    sandbox
        .handler()
        .add_private_key(&following_addr, priv_keys[0].clone());
    (gen_update_config_tx(sandbox, from_height, cfg.clone()), cfg)
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_anchoring_transit_config_normal() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        gen_confirmations_request(anchored_tx.clone(), 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![
        gen_confirmations_request(anchored_tx.clone(), 100),
        request! {
            method: "listunspent",
            params: [0, 9999999, [following_addr]],
            response: []
        },
    ]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox
        .gen_anchoring_tx_with_signatures(0,
                                          anchored_tx.payload().block_hash,
                                          &[],
                                          None,
                                          &following_multisig.1);
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&lects);

    for i in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![gen_confirmations_request(transition_tx.clone(), 15 + i)]);
        sandbox.add_height(&[]);
    }

    client.expect(vec![
        gen_confirmations_request(transition_tx.clone(), 30),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                {
                    "txid": &transition_tx.txid(),
                    "vout": 0,
                    "address": &following_multisig.1.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 30,
                    "spendable": false,
                    "solvable": false
                }
            ]
        },
    ]);
    sandbox.add_height(&[]);
    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    let (_, signatures) =
        sandbox.gen_anchoring_tx_with_signatures(10,
                                                 block_hash_on_height(&sandbox, 10),
                                                 &[],
                                                 None,
                                                 &following_multisig.1);
    let anchored_tx = sandbox.latest_anchored_tx();
    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 40)]);
    sandbox.add_height(&signatures[0..1]);

    let signatures = signatures
        .into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    client.expect(vec![
        gen_confirmations_request(transition_tx.clone(), 100),
        gen_confirmations_request(anchored_tx.clone(), 0),
    ]);
    sandbox.add_height(&signatures[1..]);

    let lects = (0..4)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &anchored_tx, 3)
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![
        request! {
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
        },
    ]);
    sandbox.add_height(&lects);
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_anchoring_transit_config_unchanged_self_key() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let (cfg_tx, following_cfg) =
        gen_following_cfg_unchanged_self_key(&sandbox, cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        gen_confirmations_request(anchored_tx.clone(), 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![
        gen_confirmations_request(anchored_tx.clone(), 100),
        request! {
            method: "listunspent",
            params: [0, 9999999, [following_addr]],
            response: []
        },
    ]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox
        .gen_anchoring_tx_with_signatures(0,
                                          anchored_tx.payload().block_hash,
                                          &[],
                                          None,
                                          &following_multisig.1);
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&lects);

    for i in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![gen_confirmations_request(transition_tx.clone(), 15 + i)]);
        sandbox.add_height(&[]);
    }

    client.expect(vec![
        gen_confirmations_request(transition_tx.clone(), 30),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                {
                    "txid": &transition_tx.txid(),
                    "vout": 0,
                    "address": &following_multisig.1.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 30,
                    "spendable": false,
                    "solvable": false
                }
            ]
        },
    ]);
    sandbox.add_height(&[]);
    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    let (_, signatures) =
        sandbox.gen_anchoring_tx_with_signatures(10,
                                                 block_hash_on_height(&sandbox, 10),
                                                 &[],
                                                 None,
                                                 &following_multisig.1);
    let anchored_tx = sandbox.latest_anchored_tx();
    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 40)]);
    sandbox.add_height(&signatures[0..1]);

    let signatures = signatures
        .into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    client.expect(vec![
        gen_confirmations_request(transition_tx.clone(), 100),
        gen_confirmations_request(anchored_tx.clone(), 0),
    ]);
    sandbox.add_height(&signatures[1..]);

    let lects = (0..4)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &anchored_tx, 3)
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![
        request! {
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
        },
    ]);
    sandbox.add_height(&lects);
}

// We commit a new configuration with confirmed funding tx
// and take actions to transit tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_anchoring_transit_config_with_funding_tx() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let funding_tx = FundingTx::from_hex("01000000019aaf09d7e73a5f9ab394f1358bfb3dbde7b15b983d715f\
        5c98f369a3f0a288a7010000006a473044022025e8ae682e4e681e6819d704edfc9e0d1e9b47eeaf7306f71437\
        b89fd60b7a3502207396e9861df9d6a9481aa7d7cbb1bf03add8a891bead0d07ff942cf82ac104ce01210361ee\
        947a30572b1e9fd92ca6b0dd2b3cc738e386daf1b19321b15cb1ce6f345bfeffffff02e80300000000000017a9\
        1476ee0b0e9603920c421f1abbda07623eb0c3f2c287370ed70b000000001976a914c89746247160e12dc7b0b3\
        2a5507518a70eabd0a88ac3aae1000")
            .unwrap();
    let (cfg_tx, following_cfg) =
        gen_following_cfg(&sandbox, cfg_change_height, Some(funding_tx.clone()));
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        gen_confirmations_request(anchored_tx.clone(), 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![
        gen_confirmations_request(anchored_tx.clone(), 100),
        request! {
            method: "listunspent",
            params: [0, 9999999, [following_addr]],
            response: [
                {
                    "txid": &funding_tx.txid(),
                    "vout": 0,
                    "address": &following_addr.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 100,
                    "spendable": true,
                    "solvable": false
                }
            ]
        },
    ]);

    let (_, signatures) = sandbox
        .gen_anchoring_tx_with_signatures(0,
                                          anchored_tx.payload().block_hash,
                                          &[],
                                          None,
                                          &following_addr);
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&lects);

    for i in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![gen_confirmations_request(transition_tx.clone(), 15 + i)]);
        sandbox.add_height(&[]);
    }

    client.expect(vec![
        gen_confirmations_request(transition_tx.clone(), 30),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                {
                    "txid": &transition_tx.txid(),
                    "vout": 0,
                    "address": &following_addr.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 1,
                    "spendable": true,
                    "solvable": false
                },
                {
                    "txid": &funding_tx.txid(),
                    "vout": 0,
                    "address": &following_addr.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 100,
                    "spendable": true,
                    "solvable": false
                }
            ]
        },
    ]);
    sandbox.add_height(&[]);
    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    let (_, signatures) =
        sandbox.gen_anchoring_tx_with_signatures(10,
                                                 block_hash_on_height(&sandbox, 10),
                                                 &[funding_tx.clone()],
                                                 None,
                                                 &following_addr);
    let anchored_tx = sandbox.latest_anchored_tx();
    sandbox.broadcast(signatures[0].clone());
    sandbox.broadcast(signatures[1].clone());
    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 40)]);
    sandbox.add_height(&signatures[0..2]);

    let signatures = signatures
        .into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    client.expect(vec![
        gen_confirmations_request(transition_tx.clone(), 100),
        gen_confirmations_request(anchored_tx.clone(), 0),
    ]);
    sandbox.add_height(&signatures[2..]);

    let lects = (0..4)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &anchored_tx, 3)
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                {
                    "txid": &anchored_tx.txid(),
                    "vout": 0,
                    "address": &following_addr.to_base58check(),
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
        },
    ]);
    sandbox.add_height(&lects);

    assert_eq!(anchored_tx.amount(), 2000);
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - we losing transition tx before following config height
// result: we resend it
#[test]
fn test_anchoring_transit_config_lost_lect_recover_before_cfg_change() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        gen_confirmations_request(anchored_tx.clone(), 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![
        gen_confirmations_request(anchored_tx.clone(), 100),
        request! {
            method: "listunspent",
            params: [0, 9999999, [following_addr]],
            response: []
        },
    ]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox
        .gen_anchoring_tx_with_signatures(0,
                                          anchored_tx.payload().block_hash,
                                          &[],
                                          None,
                                          &following_multisig.1);
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&lects);

    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&transition_tx.txid(), 1],
            error: RpcError::NoInformation("Unable to find tx".to_string()),
        },
        request! {
            method: "sendrawtransaction",
            params: [&transition_tx.to_hex()]
        },
    ]);
    sandbox.add_height(&[]);
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - we losing transition tx and we have no time to recovering it
// result: we trying to resend tx
#[test]
fn test_anchoring_transit_config_lost_lect_recover_after_cfg_change() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        gen_confirmations_request(anchored_tx.clone(), 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![
        gen_confirmations_request(anchored_tx.clone(), 100),
        request! {
            method: "listunspent",
            params: [0, 9999999, [following_addr]],
            response: []
        },
    ]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox
        .gen_anchoring_tx_with_signatures(0,
                                          anchored_tx.payload().block_hash,
                                          &[],
                                          None,
                                          &following_multisig.1);
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&lects);

    for _ in sandbox.current_height()..20 {
        client.expect(vec![gen_confirmations_request(transition_tx.clone(), 20)]);
        sandbox.add_height(&[]);
    }

    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);

    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&transition_tx.txid(), 1],
            error: RpcError::NoInformation("Unable to find tx".to_string()),
        },
        request! {
            method: "sendrawtransaction",
            params: [&transition_tx.to_hex()]
        },
    ]);
    sandbox.add_height(&[]);

    client.expect(vec![
        gen_confirmations_request(transition_tx.clone(), 30),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                {
                    "txid": &transition_tx.txid(),
                    "vout": 0,
                    "address": &following_multisig.1.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 30,
                    "spendable": false,
                    "solvable": false
                }
            ]
        },
    ]);
    sandbox.add_height(&[]);
    let (_, signatures) =
        sandbox.gen_anchoring_tx_with_signatures(20,
                                                 block_hash_on_height(&sandbox, 20),
                                                 &[],
                                                 None,
                                                 &following_multisig.1);
    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 40)]);
    sandbox.add_height(&signatures[0..1]);
}

// We do not commit a new configuration
// problems:
//  - We have no time to create transition transaction
// result: we create a new anchoring tx chain from scratch
#[test]
fn test_anchoring_transit_config_lost_lect_new_tx_chain() {
    let cfg_change_height = 11;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let funding_tx = FundingTx::from_hex("01000000019aaf09d7e73a5f9ab394f1358bfb3dbde7b15b983d715f\
        5c98f369a3f0a288a7010000006a473044022025e8ae682e4e681e6819d704edfc9e0d1e9b47eeaf7306f71437\
        b89fd60b7a3502207396e9861df9d6a9481aa7d7cbb1bf03add8a891bead0d07ff942cf82ac104ce01210361ee\
        947a30572b1e9fd92ca6b0dd2b3cc738e386daf1b19321b15cb1ce6f345bfeffffff02e80300000000000017a9\
        1476ee0b0e9603920c421f1abbda07623eb0c3f2c287370ed70b000000001976a914c89746247160e12dc7b0b3\
        2a5507518a70eabd0a88ac3aae1000")
            .unwrap();
    let (cfg_tx, following_cfg) =
        gen_following_cfg(&sandbox, cfg_change_height, Some(funding_tx.clone()));
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        gen_confirmations_request(anchored_tx.clone(), 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    for _ in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![gen_confirmations_request(anchored_tx.clone(), 10)]);
        sandbox.add_height(&[]);
    }

    let previous_anchored_tx = sandbox.latest_anchored_tx();
    let following_multisig = following_cfg.redeem_script();

    client.expect(vec![
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
    ]);
    sandbox.add_height(&[]);

    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    sandbox.set_latest_anchored_tx(None);
    // Generate new chain
    let (_, signatures) =
        sandbox.gen_anchoring_tx_with_signatures(10,
                                                 block_hash_on_height(&sandbox, 10),
                                                 &[],
                                                 Some(previous_anchored_tx.id()),
                                                 &following_multisig.1);
    let new_chain_tx = sandbox.latest_anchored_tx();

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![
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
    ]);
    sandbox.add_height(&signatures[0..1]);

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                {
                    "txid": &funding_tx.txid(),
                    "vout": 0,
                    "address": &following_multisig.1.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8df\
                                        b78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 200,
                    "spendable": false,
                    "solvable": false
                }
            ]
        },
        gen_confirmations_request(new_chain_tx.clone(), 0),
    ]);
    sandbox.add_height(&signatures[1..]);
    let lects = (0..4)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &new_chain_tx, 2)
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
}

// We send `MsgAnchoringSignature` with current output_address
// problems:
// - none
// result: msg ignored
#[test]
fn test_anchoring_transit_msg_signature_incorrect_output_address() {
    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, 16, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        gen_confirmations_request(anchored_tx.clone(), 0),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![
        gen_confirmations_request(anchored_tx.clone(), 100),
        request! {
            method: "listunspent",
            params: [0, 9999999, [following_addr]],
            response: []
        },
    ]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox
        .gen_anchoring_tx_with_signatures(0,
                                          anchored_tx.payload().block_hash,
                                          &[],
                                          None,
                                          &following_multisig.1);
    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());
    sandbox.add_height(&signatures[0..1]);

    // Gen transaction with different `output_addr`
    let different_signatures = {
        let tx = TransactionBuilder::with_prev_tx(&sandbox.latest_anchored_tx(), 0)
            .fee(1000)
            .payload(5, block_hash_on_height(&sandbox, 5))
            .send_to(sandbox.current_addr())
            .into_transaction()
            .unwrap();
        sandbox.gen_anchoring_signatures(&tx)
    };
    // Try to send different messages
    let txid = different_signatures[0].tx().id();
    let signs_before = dump_signatures(&sandbox, &txid);
    // Try to commit tx
    let different_signatures = different_signatures
        .into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    sandbox.add_height(&different_signatures);
    // Ensure that service ignores tx
    let signs_after = dump_signatures(&sandbox, &txid);
    assert_eq!(signs_before, signs_after);

}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
// - none
// result: unimplemented
#[test]
#[should_panic(expected = "We must not to change genesis configuration!")]
fn test_anchoring_transit_config_after_funding_tx() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    let funding_tx = sandbox.current_funding_tx();
    client.expect(vec![gen_confirmations_request(funding_tx.clone(), 0)]);
    sandbox.add_height(&[]);

    // Commit following configuration
    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        gen_confirmations_request(funding_tx.clone(), 0),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Wait until `funding_tx` get enough confirmations
    for _ in 0..3 {
        client.expect(vec![gen_confirmations_request(funding_tx.clone(), 1)]);
        sandbox.add_height(&[]);
    }

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [following_addr]],
            response: []
        },
        gen_confirmations_request(funding_tx.clone(), 1),
    ]);
    sandbox.add_height(&[]);

    // Has enough confirmations for funding_tx
    client.expect(vec![
        gen_confirmations_request(funding_tx.clone(), 100),
        request! {
            method: "listunspent",
            params: [0, 9999999, [following_addr]],
            response: []
        },
    ]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) =
        sandbox.gen_anchoring_tx_with_signatures(0,
                                                 block_hash_on_height(&sandbox, 0),
                                                 &[],
                                                 None,
                                                 &following_multisig.1);
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 0)]);
    sandbox.add_height(&lects);

    for i in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![gen_confirmations_request(transition_tx.clone(), 15 + i)]);
        sandbox.add_height(&[]);
    }

    client.expect(vec![
        gen_confirmations_request(transition_tx.clone(), 30),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                {
                    "txid": &transition_tx.txid(),
                    "vout": 0,
                    "address": &following_multisig.1.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 30,
                    "spendable": false,
                    "solvable": false
                }
            ]
        },
    ]);
    sandbox.add_height(&[]);
    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    let (_, signatures) =
        sandbox.gen_anchoring_tx_with_signatures(10,
                                                 block_hash_on_height(&sandbox, 10),
                                                 &[],
                                                 None,
                                                 &following_multisig.1);
    let anchored_tx = sandbox.latest_anchored_tx();
    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![gen_confirmations_request(transition_tx.clone(), 40)]);
    sandbox.add_height(&signatures[0..1]);

    let signatures = signatures
        .into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    client.expect(vec![
        gen_confirmations_request(transition_tx.clone(), 100),
        gen_confirmations_request(anchored_tx.clone(), 0),
    ]);
    sandbox.add_height(&signatures[1..]);

    let lects = (0..4)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &anchored_tx, 3)
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![
        request! {
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
        },
    ]);
    sandbox.add_height(&lects);
}

// We exclude sandbox node from consensus and after add it as validator
// problems:
// - none
// result: we continues anchoring as validator
#[test]
fn test_anchoring_transit_after_exclude_from_validator() {
    let cfg_change_height = 16;

    let _ = exonum::helpers::init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    let sandbox_node_pubkey = sandbox.cfg().validators[0].clone();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);
    exclude_node_from_validators(&sandbox);

    let (cfg_tx, cfg, node_cfg, following_addr) = {
        let mut rng: StdRng = SeedableRng::from_seed([3, 12, 3, 117].as_ref());
        let keypair = btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng);

        let mut service_cfg = sandbox.current_cfg().clone();
        let priv_keys = sandbox.current_priv_keys();

        service_cfg.validators.push(keypair.0.clone());
        service_cfg.validators.swap(0, 3);

        let following_addr = service_cfg.redeem_script().1;
        debug!("following_addr={}", following_addr.to_base58check());
        for (id, ref mut node) in sandbox.nodes_mut().iter_mut().enumerate() {
            node.private_keys
                .insert(following_addr.to_base58check(), priv_keys[id].clone());
        }

        // Add NodeConfig for previosly excluded sandbox_node
        let mut node_cfg = sandbox.nodes()[0].clone();
        node_cfg
            .private_keys
            .insert(following_addr.to_base58check(), keypair.1.clone());
        sandbox
            .handler()
            .add_private_key(&following_addr, keypair.1);

        let mut cfg = sandbox.cfg();
        cfg.actual_from = cfg_change_height;
        cfg.validators.push(sandbox_node_pubkey);
        cfg.validators.swap(0, 3);

        *cfg.services
             .get_mut(&ANCHORING_SERVICE_ID.to_string())
             .unwrap() = service_cfg.to_json();
        let tx = TxConfig::new(&sandbox.p(1),
                               &cfg.serialize(),
                               cfg_change_height,
                               sandbox.s(1));
        (tx.raw().clone(), service_cfg, node_cfg, following_addr)
    };
    let prev_tx = sandbox.latest_anchored_tx();

    let signatures = {
        let height = sandbox.latest_anchoring_height();
        sandbox
            .gen_anchoring_tx_with_signatures(height,
                                              block_hash_on_height(&sandbox, height),
                                              &[],
                                              None,
                                              &following_addr)
            .1
    };

    let transition_tx = sandbox.latest_anchored_tx();
    let lects = (0..3)
        .map(|id| {
                 gen_service_tx_lect(&sandbox, id, &transition_tx, lects_count(&sandbox, id))
                     .raw()
                     .clone()
             })
        .collect::<Vec<_>>();

    let txs = [&[cfg_tx], signatures.as_slice(), lects.as_slice()].concat();
    // Push following cfg
    sandbox.add_height_as_auditor(&txs);
    // Apply following cfg
    sandbox.fast_forward_to_height_as_auditor(cfg_change_height - 1);
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
    ]);
    sandbox.add_height_as_auditor(&[]);
    sandbox.set_anchoring_cfg(cfg);
    {
        let mut nodes = sandbox.nodes_mut();
        nodes.push(node_cfg);
        nodes.swap(0, 3);
    }
    // Check transition tx
    sandbox.fast_forward_to_height(sandbox.next_check_lect_height());

    let lect = gen_service_tx_lect(&sandbox, 0, &transition_tx, lects_count(&sandbox, 0))
        .raw()
        .clone();

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                {
                    "txid": &transition_tx.txid(),
                    "vout": 0,
                    "address": &following_addr.to_base58check(),
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
            params: [&transition_tx.txid(), 0],
            response: &transition_tx.to_hex()
        },
        request! {
            method: "getrawtransaction",
            params: [&prev_tx.txid(), 0],
            response: &prev_tx.to_hex()
        },
    ]);
    sandbox.add_height(&[]);

    sandbox.broadcast(lect.clone());
    sandbox.add_height(&[lect]);

    // Create next anchoring tx proposal
    client.expect(vec![
        // Check for the `funding_tx` availability
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                {
                    "txid": &transition_tx.txid(),
                    "vout": 0,
                    "address": &following_addr.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 0,
                    "spendable": false,
                    "solvable": false
                }
            ]
        },
    ]);
    sandbox.add_height(&[]);
    let signatures = {
        let height = sandbox.latest_anchoring_height();
        sandbox
            .gen_anchoring_tx_with_signatures(height,
                                              block_hash_on_height(&sandbox, height),
                                              &[],
                                              None,
                                              &following_addr)
            .1
    };
    let anchored_tx = sandbox.latest_anchored_tx();
    sandbox.broadcast(signatures[0].clone());
    // Commit anchoring transaction to bitcoin blockchain
    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 1],
            error: RpcError::NoInformation("Unable to find tx".to_string()),
        },
        request! {
            method: "sendrawtransaction",
            params: [&anchored_tx.to_hex()]
        },
    ]);
    sandbox.add_height(&signatures);

    let lect = gen_service_tx_lect(&sandbox, 0, &anchored_tx, lects_count(&sandbox, 0));
    sandbox.broadcast(lect);
}
