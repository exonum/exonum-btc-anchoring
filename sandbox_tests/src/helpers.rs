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

use bitcoin::util::base58::{FromBase58, ToBase58};
use serde_json::Value;

use exonum::messages::{Message, RawTransaction};
use exonum::crypto::{Hash, HexValue};
use exonum::blockchain::Schema;
use exonum::storage::StorageValue;
use exonum::helpers;

use sandbox::sandbox::Sandbox;
use sandbox::config_updater::TxConfig;

use btc_anchoring_service::{ANCHORING_SERVICE_NAME, AnchoringConfig};
use btc_anchoring_service::details::btc;
use btc_anchoring_service::details::btc::transactions::{BitcoinTx, RawBitcoinTx, TxFromRaw};
use btc_anchoring_service::details::sandbox::Request;
use btc_anchoring_service::blockchain::dto::{MsgAnchoringSignature, MsgAnchoringUpdateLatest};
use btc_anchoring_service::blockchain::schema::AnchoringSchema;

use AnchoringSandbox;

pub use bitcoinrpc::RpcError as JsonRpcError;
pub use bitcoinrpc::Error as RpcError;

pub fn gen_service_tx_lect(
    sandbox: &Sandbox,
    validator: u16,
    tx: &RawBitcoinTx,
    count: u64,
) -> MsgAnchoringUpdateLatest {
    MsgAnchoringUpdateLatest::new(
        &sandbox.service_public_key(validator as usize),
        validator,
        BitcoinTx::from(tx.clone()),
        count,
        sandbox.service_secret_key(validator as usize),
    )
}

pub fn gen_service_tx_lect_wrong(
    sandbox: &Sandbox,
    real_id: u16,
    fake_id: u16,
    tx: &RawBitcoinTx,
    count: u64,
) -> MsgAnchoringUpdateLatest {
    MsgAnchoringUpdateLatest::new(
        &sandbox.service_public_key(real_id as usize),
        fake_id,
        BitcoinTx::from(tx.clone()),
        count,
        sandbox.service_secret_key(real_id as usize),
    )
}

pub fn dump_lects(sandbox: &Sandbox, id: u16) -> Vec<BitcoinTx> {
    let b = sandbox.blockchain_ref().clone();
    let anchoring_schema = AnchoringSchema::new(b.snapshot());
    let key = &anchoring_schema.actual_anchoring_config().anchoring_keys[id as usize];

    let lects = anchoring_schema.lects(key);
    let lects = lects.into_iter().map(|x| x.tx()).collect::<Vec<_>>();
    lects
}

pub fn lects_count(sandbox: &Sandbox, id: u16) -> u64 {
    dump_lects(sandbox, id).len() as u64
}

pub fn force_commit_lects<I>(sandbox: &Sandbox, lects: I)
where
    I: IntoIterator<Item = MsgAnchoringUpdateLatest>,
{
    let mut blockchain = sandbox.blockchain_mut();
    let mut fork = blockchain.fork();
    {
        let mut anchoring_schema = AnchoringSchema::new(&mut fork);
        let anchoring_cfg = anchoring_schema.actual_anchoring_config();
        for lect_msg in lects {
            let key = &anchoring_cfg.anchoring_keys[lect_msg.validator() as usize];
            anchoring_schema.add_lect(key, lect_msg.tx().clone(), Message::hash(&lect_msg));
        }
    };
    blockchain.merge(fork.into_patch()).unwrap();
}

pub fn dump_signatures(sandbox: &Sandbox, txid: &btc::TxId) -> Vec<MsgAnchoringSignature> {
    let b = sandbox.blockchain_ref().clone();
    let v = b.snapshot();
    let anchoring_schema = AnchoringSchema::new(&v);

    let signatures = anchoring_schema.signatures(txid);
    let signatures = signatures.iter().collect::<Vec<_>>();
    signatures
}

pub fn gen_update_config_tx(
    sandbox: &Sandbox,
    actual_from: u64,
    service_cfg: &AnchoringConfig,
) -> RawTransaction {
    let mut cfg = sandbox.cfg();
    cfg.actual_from = actual_from;
    cfg.previous_cfg_hash = sandbox.cfg().hash();
    *cfg.services.get_mut(ANCHORING_SERVICE_NAME).unwrap() = json!(service_cfg);
    let tx = TxConfig::new(
        &sandbox.service_public_key(0),
        &cfg.into_bytes(),
        actual_from,
        sandbox.service_secret_key(0),
    );
    tx.raw().clone()
}

pub fn confirmations_request(raw: &RawBitcoinTx, confirmations: u64) -> Request {
    let tx = BitcoinTx::from_raw(raw.clone()).unwrap();
    request! {
            method: "getrawtransaction",
            params: [&tx.txid(), 1],
            response: {
                "hash":&tx.txid(),
                "hex":&tx.to_hex(),
                "confirmations": confirmations,
                "locktime":1088682,
                "size":223,
                "txid":&tx.to_hex(),
                "version":1,
                "vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bac\
                    c2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d\
                    07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f8\
                    76","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd\
                    28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012\
                    102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},
                    "sequence":429496729,
                    "txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645",
                    "vout":0}],
                "vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],
                    "asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL",
                    "hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87",
                    "reqSigs":1,
                    "type":"scripthash"},
                    "value":0.00004},
                    {"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],
                    "asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERI\
                        FY OP_CHECKSIG",
                    "hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac",
                    "reqSigs":1,"type":"pubkeyhash"},
                    "value":1.00768693}],
                "vsize":223
            }
        }
}

pub fn get_transaction_request(raw: &RawBitcoinTx) -> Request {
    let tx = BitcoinTx::from_raw(raw.clone()).unwrap();
    request! {
        method: "getrawtransaction",
        params: [&tx.txid(), 0],
        response: &tx.to_hex()
    }
}

pub fn send_raw_transaction_requests(raw: &RawBitcoinTx) -> Vec<Request> {
    let tx = BitcoinTx::from_raw(raw.clone()).unwrap();
    vec![
        request! {
            method: "getrawtransaction",
            params: [&tx.txid(), 1],
            error: RpcError::NoInformation("Unable to find tx".to_string())
        },
        request! {
            method: "sendrawtransaction",
            params: [tx.to_hex()],
            response: tx.to_hex()
        },
    ]
}

pub fn listunspent_entry(raw: &RawBitcoinTx, addr: &btc::Address, confirmations: u64) -> Value {
    let tx = BitcoinTx::from_raw(raw.clone()).unwrap();
    json!({
        "txid": &tx.txid(),
        "address": &addr.to_base58check(),
        "confirmations": confirmations,
        "vout": 0,
        "account": "multisig",
        "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
        "amount": 0.00010000,
        "spendable": false,
        "solvable": false
    })
}

pub fn block_hash_on_height(sandbox: &Sandbox, height: u64) -> Hash {
    let blockchain = sandbox.blockchain_ref();
    let snapshot = blockchain.snapshot();
    let schema = Schema::new(&snapshot);
    schema.block_hashes_by_height().get(height).unwrap()
}

/// Anchor genesis block using funding tx
pub fn anchor_first_block(sandbox: &AnchoringSandbox) {
    let client = sandbox.client();

    let anchoring_addr = sandbox.current_addr();


    client.expect(vec![
            confirmations_request(&sandbox.current_funding_tx(), 50),
            request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&sandbox.current_funding_tx(), &anchoring_addr, 50)
            ]
        },
        ]);

    let hash = sandbox.last_hash();
    let (_, signatures) =
        sandbox.gen_anchoring_tx_with_signatures(0, hash, &[], None, &anchoring_addr);
    let anchored_tx = sandbox.latest_anchored_tx();
    sandbox.add_height(&[]);

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![
            confirmations_request(&sandbox.current_funding_tx(), 50),
            request! {
                method: "getrawtransaction",
                params: [&anchored_tx.txid(), 1],
                error: RpcError::NoInformation("Unable to find tx".to_string())
            },
            request! {
                method: "sendrawtransaction",
                params: [anchored_tx.to_hex()],
                response: anchored_tx.to_hex()
            },
        ]);

    let signatures = signatures.into_iter().map(|tx| tx).collect::<Vec<_>>();
    sandbox.add_height(&signatures);

    let txs = (0..4)
        .map(|idx| {
            gen_service_tx_lect(sandbox, idx, &anchored_tx, 1)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(txs[0].clone());
    sandbox.add_height(&txs);
}

pub fn anchor_first_block_lect_normal(sandbox: &AnchoringSandbox) {
    // Just add few heights
    sandbox.fast_forward_to_height(sandbox.next_check_lect_height());

    let anchored_tx = sandbox.latest_anchored_tx();
    let anchoring_addr = sandbox.current_addr();

    sandbox.client().expect(vec![
            request! {
                method: "listunspent",
                params: [0, 9999999, [&anchoring_addr.to_base58check()]],
                response: [
                    listunspent_entry(&anchored_tx, &anchoring_addr, 0),
                ]
            },
            request! {
                method: "getrawtransaction",
                params: [&anchored_tx.txid(), 0],
                response: &anchored_tx.to_hex()
            },
        ]);
    sandbox.add_height(&[]);
}

pub fn anchor_first_block_lect_different(sandbox: &AnchoringSandbox) {
    let client = sandbox.client();

    anchor_first_block(sandbox);
    // Just add few heights
    sandbox.fast_forward_to_height(sandbox.next_check_lect_height());

    let (other_lect, other_signatures) = {
        let anchored_tx = sandbox.latest_anchored_tx();
        let other_signatures = sandbox
            .latest_anchored_tx_signatures()
            .iter()
            .filter(|tx| tx.validator() != 0)
            .cloned()
            .collect::<Vec<_>>();
        let other_lect = sandbox.finalize_tx(anchored_tx.clone(), other_signatures.clone());
        (other_lect, other_signatures)
    };

    let anchoring_addr = sandbox.current_addr();
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&other_lect, &anchoring_addr, 0)
            ]
        },
        get_transaction_request(&other_lect)
    ]);
    sandbox.add_height(&[]);

    let txs = (0..4)
        .map(|idx| {
            gen_service_tx_lect(sandbox, idx, &other_lect, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(txs[0].clone());

    sandbox.add_height(&txs);
    sandbox.set_latest_anchored_tx(Some((other_lect.clone(), other_signatures.clone())));
}

pub fn anchor_first_block_lect_lost(sandbox: &AnchoringSandbox) {
    let client = sandbox.client();

    anchor_first_block(sandbox);
    // Just add few heights
    sandbox.fast_forward_to_height(sandbox.next_check_lect_height());

    let other_lect = sandbox.current_funding_tx();
    let anchoring_addr = sandbox.current_addr();

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&other_lect, &anchoring_addr, 0)
            ]
        },
        get_transaction_request(&other_lect)
    ]);
    sandbox.add_height(&[]);

    let txs = (0..4)
        .map(|idx| {
            gen_service_tx_lect(sandbox, idx, &other_lect, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(txs[0].clone());

    client.expect(vec![
        confirmations_request(&sandbox.current_funding_tx(), 50),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&other_lect, &anchoring_addr, 100)
            ]
        },
    ]);
    sandbox.add_height(&txs);

    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        confirmations_request(&sandbox.current_funding_tx(), 50),
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 1],
            error: RpcError::NoInformation("Unable to find tx".to_string())
        },
        request! {
            method: "sendrawtransaction",
            params: [anchored_tx.to_hex()],
            response: anchored_tx.to_hex()
        },
    ]);
    sandbox.add_height(&[]);
    sandbox.broadcast(gen_service_tx_lect(sandbox, 0, &anchored_tx, 3));
    sandbox.set_latest_anchored_tx(None);
}

pub fn anchor_second_block_normal(sandbox: &AnchoringSandbox) {
    let client = sandbox.client();
    sandbox.fast_forward_to_height(sandbox.next_anchoring_height());

    let anchoring_addr = sandbox.current_addr();
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&sandbox.latest_anchored_tx(), &anchoring_addr, 1)
            ]
        },
    ]);
    sandbox.add_height(&[]);

    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        10,
        sandbox.last_hash(),
        &[],
        None,
        &btc::Address::from_base58check(&anchoring_addr.to_base58check())
            .unwrap(),
    );
    let anchored_tx = sandbox.latest_anchored_tx();

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![confirmations_request(&anchored_tx.clone(), 0)]);
    sandbox.add_height(&signatures);

    let txs = (0..4)
        .map(|idx| {
            gen_service_tx_lect(sandbox, idx, &anchored_tx, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(txs[0].clone());
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&anchored_tx, &anchoring_addr, 100)
            ]
        },
        get_transaction_request(&anchored_tx)
    ]);
    sandbox.add_height(&txs);
}

/// Anchor genesis block using funding tx
pub fn anchor_first_block_without_other_signatures(sandbox: &AnchoringSandbox) {
    let client = sandbox.client();
    let anchoring_addr = sandbox.current_addr();

    client.expect(vec![
        confirmations_request(&sandbox.current_funding_tx(), 50),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&sandbox.current_funding_tx(), &anchoring_addr, 50)
            ]
        },
    ]);

    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        0,
        sandbox.last_hash(),
        &[],
        None,
        &anchoring_addr,
    );
    sandbox.add_height(&[]);

    sandbox.broadcast(signatures[0].clone());
    client.expect(
        vec![confirmations_request(&sandbox.current_funding_tx(), 50)],
    );
    sandbox.add_height(&signatures[0..1]);
}

// Invoke this method after anchor_first_block_lect_normal
pub fn exclude_node_from_validators(sandbox: &AnchoringSandbox) {
    let cfg_change_height = 12;
    let (cfg_tx, following_cfg) = gen_following_cfg_exclude_validator(sandbox, cfg_change_height);
    let (_, following_addr) = following_cfg.redeem_script();

    // Tx has not enough confirmations.
    let anchored_tx = sandbox.latest_anchored_tx();

    let client = sandbox.client();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        0,
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = sandbox.latest_anchored_tx();
    // Tx gets enough confirmations.
    client.expect(vec![confirmations_request(&anchored_tx, 100)]);
    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 100)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(sandbox, id, &transition_tx, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
    client.expect(vec![confirmations_request(&transition_tx, 100)]);
    sandbox.add_height(&lects);
    sandbox.fast_forward_to_height(cfg_change_height);

    sandbox.set_anchoring_cfg(following_cfg);
    sandbox.nodes_mut().swap_remove(0);
    client.expect(vec![
        get_transaction_request(&transition_tx)
    ]);
    sandbox.add_height_as_auditor(&[]);

    assert_eq!(sandbox.handler().errors, Vec::new());
}

pub fn init_logger() {
    let _ = helpers::init_logger();
}


/// Generates a configuration that excludes `sandbox node` from consensus.
/// Then it continues to work as auditor.
fn gen_following_cfg_exclude_validator(
    sandbox: &AnchoringSandbox,
    from_height: u64,
) -> (RawTransaction, AnchoringConfig) {

    let mut service_cfg = sandbox.current_cfg().clone();
    let priv_keys = sandbox.current_priv_keys();
    service_cfg.anchoring_keys.swap_remove(0);

    let following_addr = service_cfg.redeem_script().1;
    for (id, ref mut node) in sandbox.nodes_mut().iter_mut().enumerate() {
        node.private_keys.insert(
            following_addr.to_base58check(),
            priv_keys[id].clone(),
        );
    }

    let mut cfg = sandbox.cfg();
    cfg.actual_from = from_height;
    cfg.previous_cfg_hash = sandbox.cfg().hash();
    cfg.validator_keys.swap_remove(0);
    *cfg.services.get_mut(ANCHORING_SERVICE_NAME).unwrap() = json!(service_cfg);
    let tx = TxConfig::new(
        &sandbox.service_public_key(0),
        &cfg.into_bytes(),
        from_height,
        sandbox.service_secret_key(0),
    );
    (tx.raw().clone(), service_cfg)
}
