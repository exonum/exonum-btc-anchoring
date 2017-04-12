pub use bitcoinrpc::RpcError as JsonRpcError;
pub use bitcoinrpc::Error as RpcError;

use serde_json::value::ToJson;
use bitcoin::util::base58::{ToBase58, FromBase58};

use exonum::messages::{Message, RawTransaction};
use exonum::crypto::{Hash, HexValue};
use exonum::blockchain::Schema;
use exonum::storage::{List, StorageValue};

use sandbox::sandbox::Sandbox;
use sandbox::sandbox_tests_helper::{SandboxState, add_one_height_with_transactions};
use sandbox::config_updater::TxConfig;

use anchoring_btc_service::{ANCHORING_SERVICE_ID, AnchoringConfig};
use anchoring_btc_service::details::btc;
use anchoring_btc_service::details::btc::transactions::{RawBitcoinTx, BitcoinTx};
use anchoring_btc_service::details::sandbox::{SandboxClient, Request};
use anchoring_btc_service::blockchain::dto::{MsgAnchoringSignature, MsgAnchoringUpdateLatest};
use anchoring_btc_service::blockchain::schema::AnchoringSchema;

use AnchoringSandboxState;

pub fn gen_service_tx_lect(sandbox: &Sandbox,
                           validator: u32,
                           tx: &RawBitcoinTx,
                           count: u64)
                           -> RawTransaction {
    let tx = MsgAnchoringUpdateLatest::new(&sandbox.p(validator as usize),
                                           validator,
                                           BitcoinTx::from(tx.clone()),
                                           count,
                                           sandbox.s(validator as usize));
    tx.raw().clone()
}

pub fn gen_service_tx_lect_wrong(sandbox: &Sandbox,
                                 real_id: u32,
                                 fake_id: u32,
                                 tx: &RawBitcoinTx,
                                 count: u64)
                                 -> RawTransaction {
    let tx = MsgAnchoringUpdateLatest::new(&sandbox.p(real_id as usize),
                                           fake_id,
                                           BitcoinTx::from(tx.clone()),
                                           count,
                                           sandbox.s(real_id as usize));
    tx.raw().clone()
}

pub fn dump_lects(sandbox: &Sandbox, id: u32) -> Vec<BitcoinTx> {
    let b = sandbox.blockchain_ref().clone();
    let v = b.view();
    let s = AnchoringSchema::new(&v);
    s.lects(id).values().unwrap()
}

pub fn dump_signatures(sandbox: &Sandbox, txid: &btc::TxId) -> Vec<MsgAnchoringSignature> {
    let b = sandbox.blockchain_ref().clone();
    let v = b.view();
    let s = AnchoringSchema::new(&v);
    s.signatures(txid).values().unwrap()
}

pub fn gen_update_config_tx(sandbox: &Sandbox,
                            actual_from: u64,
                            service_cfg: AnchoringConfig)
                            -> RawTransaction {
    let mut cfg = sandbox.cfg();
    cfg.actual_from = actual_from;
    *cfg.services
         .get_mut(&ANCHORING_SERVICE_ID.to_string())
         .unwrap() = service_cfg.to_json();
    let tx = TxConfig::new(&sandbox.p(0), &cfg.serialize(), actual_from, sandbox.s(0));
    tx.raw().clone()
}

pub fn block_hash_on_height(sandbox: &Sandbox, height: u64) -> Hash {
    let blockchain = sandbox.blockchain_ref();
    let view = blockchain.view();
    let schema = Schema::new(&view);
    schema.heights().get(height).unwrap().unwrap()
}

/// Anchor genesis block using funding tx 
pub fn anchor_first_block_without_other_signatures(sandbox: &Sandbox,
                          client: &SandboxClient,
                          sandbox_state: &SandboxState,
                          anchoring_state: &mut AnchoringSandboxState) {
    let (_, anchoring_addr) = anchoring_state.common.redeem_script();

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                {
                    "txid": &anchoring_state.common.funding_tx.txid(),
                    "vout": 0,
                    "address": &anchoring_addr.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 50,
                    "spendable": false,
                    "solvable": false
                }
            ]
        }]);

    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(sandbox,
                                                         0,
                                                         sandbox.last_hash(),
                                                         &[],
                                                         None,
                                                         &anchoring_addr);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    sandbox.broadcast(signatures[0].clone());
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures[0..1]);
}

/// Anchor genesis block using funding tx
pub fn anchor_first_block(sandbox: &Sandbox,
                          client: &SandboxClient,
                          sandbox_state: &SandboxState,
                          anchoring_state: &mut AnchoringSandboxState) {
    let (_, anchoring_addr) = anchoring_state.common.redeem_script();

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                {
                    "txid": &anchoring_state.common.funding_tx.txid(),
                    "vout": 0,
                    "address": &anchoring_addr.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 50,
                    "spendable": false,
                    "solvable": false
                }
            ]
        }]);

    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(sandbox,
                                                         0,
                                                         sandbox.last_hash(),
                                                         &[],
                                                         None,
                                                         &anchoring_addr);
    let anchored_tx = anchoring_state.latest_anchored_tx();
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![request! {
                           method: "getrawtransaction",
                           params: [&anchored_tx.txid(), 1],
                           error: RpcError::NoInformation("Unable to find tx".to_string())
                       },
                       request! {
                           method: "sendrawtransaction",
                           params: [anchored_tx.to_hex()]
                       }]);

    let signatures = signatures
        .into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures);

    let txs = (0..4)
        .map(|idx| gen_service_tx_lect(sandbox, idx, &anchored_tx, 1))
        .collect::<Vec<_>>();
    sandbox.broadcast(txs[0].clone());
    add_one_height_with_transactions(sandbox, sandbox_state, &txs);
}

pub fn anchor_first_block_lect_normal(sandbox: &Sandbox,
                                      client: &SandboxClient,
                                      sandbox_state: &SandboxState,
                                      anchoring_state: &mut AnchoringSandboxState) {
    // Just add few heights
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    let anchored_tx = anchoring_state.latest_anchored_tx();
    let (_, anchoring_addr) = anchoring_state.common.redeem_script();

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                {
                    "txid": &anchored_tx.txid(),
                    "vout": 0,
                    "address": &anchoring_addr.to_base58check(),
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
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);
}

pub fn anchor_first_block_lect_lost(sandbox: &Sandbox,
                                    client: &SandboxClient,
                                    sandbox_state: &SandboxState,
                                    anchoring_state: &mut AnchoringSandboxState) {
    anchor_first_block(sandbox, client, sandbox_state, anchoring_state);
    // Just add few heights
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);

    let other_lect = anchoring_state.common.funding_tx.clone();
    let (_, anchoring_addr) = anchoring_state.common.redeem_script();

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                {
                    "txid": &other_lect.txid(),
                    "vout": 0,
                    "address": &anchoring_addr.to_base58check(),
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
            params: [&other_lect.txid(), 0],
            response: &other_lect.to_hex()
        }]);
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);

    let txs = (0..4)
        .map(|idx| gen_service_tx_lect(sandbox, idx, &other_lect, 2))
        .collect::<Vec<_>>();
    sandbox.broadcast(txs[0].clone());

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                {
                    "txid": &other_lect.txid(),
                    "vout": 0,
                    "address": &anchoring_addr.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 100,
                    "spendable": false,
                    "solvable": false
                }
            ]
        }]);
    add_one_height_with_transactions(sandbox, sandbox_state, &txs);

    {
        let anchored_tx = anchoring_state.latest_anchored_tx();

        client.expect(vec![request! {
                               method: "getrawtransaction",
                               params: [&anchored_tx.txid(), 1],
                               error: RpcError::NoInformation("Unable to find tx".to_string())
                           },
                           request! {
                                method: "sendrawtransaction",
                                params: [anchored_tx.to_hex()]
                            }]);
        add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
        sandbox.broadcast(gen_service_tx_lect(sandbox, 0, &anchored_tx, 3))
    }
    anchoring_state.latest_anchored_tx = None;
}

pub fn anchor_first_block_lect_different(sandbox: &Sandbox,
                                         client: &SandboxClient,
                                         sandbox_state: &SandboxState,
                                         anchoring_state: &mut AnchoringSandboxState) {
    anchor_first_block(sandbox, client, sandbox_state, anchoring_state);
    // Just add few heights
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);

    let (other_lect, other_signatures) = {
        let anchored_tx = anchoring_state.latest_anchored_tx();
        let other_signatures = anchoring_state
            .latest_anchored_tx_signatures()
            .iter()
            .filter(|tx| tx.validator() != 0)
            .cloned()
            .collect::<Vec<_>>();
        let other_lect = anchoring_state.finalize_tx(anchored_tx.clone(),
                                                     other_signatures.as_ref());
        (other_lect, other_signatures)
    };

    let (_, anchoring_addr) = anchoring_state.common.redeem_script();
    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                {
                    "txid": &other_lect.txid(),
                    "vout": 0,
                    "address": &anchoring_addr.to_base58check(),
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
            params: [&other_lect.txid(), 0],
            response: &other_lect.to_hex()
        }]);
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);

    let txs = (0..4)
        .map(|idx| gen_service_tx_lect(sandbox, idx, &other_lect, 2))
        .collect::<Vec<_>>();
    sandbox.broadcast(txs[0].clone());

    add_one_height_with_transactions(sandbox, sandbox_state, &txs);
    anchoring_state.latest_anchored_tx = Some((other_lect.clone(), other_signatures.clone()));
}

pub fn anchor_second_block_normal(sandbox: &Sandbox,
                                  client: &SandboxClient,
                                  sandbox_state: &SandboxState,
                                  anchoring_state: &mut AnchoringSandboxState) {
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);

    let (_, anchoring_addr) = anchoring_state.common.redeem_script();
    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                {
                    "txid": "fea0a60f7146e7facf5bb382b80dafb762175bf0d4b6ac4e59c09cd4214d1491",
                    "vout": 0,
                    "address": &anchoring_addr.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 1,
                    "spendable": false,
                    "solvable": false
                }
            ]
        }]);
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);

    let (_, signatures) = anchoring_state.gen_anchoring_tx_with_signatures(sandbox,
        10,
        sandbox.last_hash(),
        &[],
        None,
        &btc::Address::from_base58check(&anchoring_addr.to_base58check()).unwrap()
    );
    let anchored_tx = anchoring_state.latest_anchored_tx();

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 1],
            response: {
                "hash":&anchored_tx.txid(),"hex":&anchored_tx.to_hex(),
                "locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        }
    ]);
    add_one_height_with_transactions(sandbox, sandbox_state, &signatures);

    let txs = (0..4)
        .map(|idx| gen_service_tx_lect(sandbox, idx, &anchored_tx, 2))
        .collect::<Vec<_>>();
    sandbox.broadcast(txs[0].clone());
    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                {
                    "txid": &anchored_tx.txid(),
                    "vout": 0,
                    "address": &anchoring_addr.to_base58check(),
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 100,
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
    add_one_height_with_transactions(sandbox, sandbox_state, &txs);
}
