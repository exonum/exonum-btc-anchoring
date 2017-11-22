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

use bitcoin::util::base58::ToBase58;
use serde_json::Value;

use exonum::messages::Message;
use exonum::crypto::HexValue;
use exonum::blockchain::Transaction;
use exonum::helpers::{Height, ValidatorId};

use exonum_testkit::{TestKit, TestNetworkConfiguration};

use {AnchoringConfig, ANCHORING_SERVICE_NAME};
use details::btc;
use details::btc::transactions::{BitcoinTx, RawBitcoinTx, TxFromRaw};
use blockchain::dto::{MsgAnchoringSignature, MsgAnchoringUpdateLatest};
use blockchain::schema::AnchoringSchema;

use super::{AnchoringTestKit, TestRequest};

pub use bitcoinrpc::RpcError as JsonRpcError;
pub use bitcoinrpc::Error as RpcError;
pub use super::secp256k1_hack::sign_tx_input_with_nonce;

pub fn to_boxed<T: Transaction>(tx: T) -> Box<Transaction> {
    Box::new(tx) as Box<Transaction>
}

pub fn gen_service_tx_lect(
    testkit: &TestKit,
    validator: ValidatorId,
    tx: &RawBitcoinTx,
    count: u64,
) -> MsgAnchoringUpdateLatest {
    let keypair = testkit.network().validators()[validator.0 as usize].service_keypair();
    MsgAnchoringUpdateLatest::new(
        keypair.0,
        validator,
        BitcoinTx::from(tx.clone()),
        count,
        keypair.1,
    )
}

pub fn gen_service_tx_lect_wrong(
    testkit: &TestKit,
    real_id: ValidatorId,
    fake_id: ValidatorId,
    tx: &RawBitcoinTx,
    count: u64,
) -> MsgAnchoringUpdateLatest {
    let keypair = testkit.network().validators()[real_id.0 as usize].service_keypair();
    MsgAnchoringUpdateLatest::new(
        &keypair.0,
        fake_id,
        BitcoinTx::from(tx.clone()),
        count,
        keypair.1,
    )
}

pub fn dump_lects(testkit: &TestKit, id: ValidatorId) -> Vec<BitcoinTx> {
    let anchoring_schema = AnchoringSchema::new(testkit.snapshot());
    let key = &anchoring_schema.actual_anchoring_config().anchoring_keys[id.0 as usize];

    let lects = anchoring_schema.lects(key);
    let lects = lects.into_iter().map(|x| x.tx()).collect::<Vec<_>>();
    lects
}

pub fn lects_count(testkit: &TestKit, id: ValidatorId) -> u64 {
    dump_lects(testkit, id).len() as u64
}

pub fn force_commit_lects<I>(teskit: &mut TestKit, lects: I)
where
    I: IntoIterator<Item = MsgAnchoringUpdateLatest>,
{
    let blockchain = teskit.blockchain_mut();
    let mut fork = blockchain.fork();
    {
        let mut anchoring_schema = AnchoringSchema::new(&mut fork);
        let anchoring_cfg = anchoring_schema.actual_anchoring_config();
        for lect_msg in lects {
            let validator_id = lect_msg.validator().0 as usize;
            let key = &anchoring_cfg.anchoring_keys[validator_id];
            anchoring_schema.add_lect(key, lect_msg.tx().clone(), Message::hash(&lect_msg));
        }
    };
    blockchain.merge(fork.into_patch()).unwrap();
}

pub fn dump_signatures(testkit: &TestKit, txid: &btc::TxId) -> Vec<MsgAnchoringSignature> {
    let v = testkit.snapshot();
    let anchoring_schema = AnchoringSchema::new(&v);

    let signatures = anchoring_schema.signatures(txid);
    let signatures = signatures.iter().collect::<Vec<_>>();
    signatures
}

pub fn confirmations_request(raw: &RawBitcoinTx, confirmations: u64) -> TestRequest {
    let tx = BitcoinTx::from_raw(raw.clone()).unwrap();
    request! {
        method: "getrawtransaction",
        params: [&tx.txid(), 1],
        response: {
            "hash":&tx.txid(),
            "hex":&tx.to_hex(),
            "confirmations": confirmations,
            "locktime":1_088_682,
            "size":223,
            "txid":&tx.to_hex(),
            "version":1,
            "vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bac\
                c2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d\
                07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f8\
                76","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd\
                28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012\
                102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},
                "sequence":429_496_729,
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

pub fn get_transaction_request(raw: &RawBitcoinTx) -> TestRequest {
    let tx = BitcoinTx::from_raw(raw.clone()).unwrap();
    request! {
        method: "getrawtransaction",
        params: [&tx.txid(), 0],
        response: &tx.to_hex()
    }
}

pub fn send_raw_transaction_requests(raw: &RawBitcoinTx) -> Vec<TestRequest> {
    let tx = BitcoinTx::from_raw(raw.clone()).unwrap();
    vec![
        request! {
            method: "getrawtransaction",
            params: [&tx.txid(), 0],
            error: RpcError::NoInformation("Unable to find tx".to_string())
        },
        request! {
            method: "sendrawtransaction",
            params: [tx.to_hex()],
            response: tx.to_hex()
        },
    ]
}

pub fn resend_raw_transaction_requests(raw: &RawBitcoinTx) -> Vec<TestRequest> {
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

/// Anchor genesis block using funding tx
pub fn anchor_first_block(testkit: &mut AnchoringTestKit) {
    let requests = testkit.requests();

    let anchoring_addr = testkit.current_addr();
    requests.expect(vec![
        confirmations_request(&testkit.current_funding_tx(), 50),
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&testkit.current_funding_tx(), &anchoring_addr, 50)
            ]
        },
        get_transaction_request(&testkit.current_funding_tx()),
    ]);

    let hash = testkit.last_block_hash();
    let (_, signatures) =
        testkit.gen_anchoring_tx_with_signatures(Height::zero(), hash, &[], None, &anchoring_addr);
    let anchored_tx = testkit.latest_anchored_tx();
    testkit.create_block();

    testkit.mempool().contains_key(&signatures[0].hash());
    requests.expect(vec![
        confirmations_request(&testkit.current_funding_tx(), 50),
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 0],
            error: RpcError::NoInformation("Unable to find tx".to_string())
        },
        request! {
            method: "sendrawtransaction",
            params: [anchored_tx.to_hex()],
            response: anchored_tx.to_hex()
        },
    ]);
    testkit.create_block_with_transactions(signatures);

    let txs = (0..4)
        .map(|idx| {
            gen_service_tx_lect(testkit, ValidatorId(idx), &anchored_tx, 1)
        })
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&txs[0].hash());
    testkit.create_block_with_transactions(txs);
}

pub fn anchor_first_block_lect_normal(testkit: &mut AnchoringTestKit) {
    // Just add few heights
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);

    let anchored_tx = testkit.latest_anchored_tx();
    let anchoring_addr = testkit.current_addr();

    testkit.requests().expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_string()]],
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
    testkit.create_block();
}

pub fn anchor_first_block_lect_different(testkit: &mut AnchoringTestKit) {
    let requests = testkit.requests();

    anchor_first_block(testkit);
    // Just add few heights
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);

    let (other_lect, other_signatures) = {
        let anchored_tx = testkit.latest_anchored_tx();
        let other_signatures = testkit
            .latest_anchored_tx_signatures()
            .iter()
            .filter(|tx| tx.validator() != ValidatorId(0))
            .cloned()
            .collect::<Vec<_>>();
        let other_lect = testkit.finalize_tx(anchored_tx.clone(), other_signatures.clone());
        (other_lect, other_signatures)
    };

    let anchoring_addr = testkit.current_addr();
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&other_lect, &anchoring_addr, 0)
            ]
        },
        get_transaction_request(&other_lect),
    ]);
    testkit.create_block();

    let txs = (0..4)
        .map(|idx| {
            gen_service_tx_lect(testkit, ValidatorId(idx), &other_lect, 2)
        })
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&txs[0].hash());

    testkit.create_block_with_transactions(txs);
    testkit.set_latest_anchored_tx(Some((other_lect.clone(), other_signatures.clone())));
}

pub fn anchor_first_block_lect_lost(testkit: &mut AnchoringTestKit) {
    let requests = testkit.requests();

    anchor_first_block(testkit);
    // Just add few heights
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);

    let other_lect = testkit.current_funding_tx();
    let anchoring_addr = testkit.current_addr();

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&other_lect, &anchoring_addr, 0)
            ]
        },
        get_transaction_request(&other_lect),
    ]);
    testkit.create_block();

    let txs = (0..4)
        .map(|idx| {
            gen_service_tx_lect(testkit, ValidatorId(idx), &other_lect, 2)
        })
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&txs[0].hash());

    requests.expect(vec![
        confirmations_request(&testkit.current_funding_tx(), 50),
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&other_lect, &anchoring_addr, 100)
            ]
        },
        get_transaction_request(&other_lect),
    ]);
    testkit.create_block_with_transactions(txs);

    let anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        confirmations_request(&testkit.current_funding_tx(), 50),
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 0],
            error: RpcError::NoInformation("Unable to find tx".to_string())
        },
        request! {
            method: "sendrawtransaction",
            params: [anchored_tx.to_hex()],
            response: anchored_tx.to_hex()
        },
    ]);
    testkit.create_block();
    let lect = gen_service_tx_lect(testkit, ValidatorId(0), &anchored_tx, 3);
    testkit.mempool().contains_key(&to_boxed(lect).hash());
    testkit.set_latest_anchored_tx(None);
}

pub fn anchor_second_block_normal(testkit: &mut AnchoringTestKit) {
    let requests = testkit.requests();
    let height = testkit.next_anchoring_height();
    testkit.create_blocks_until(height);

    let anchoring_addr = testkit.current_addr();
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&testkit.latest_anchored_tx(), &anchoring_addr, 1)
            ]
        },
        get_transaction_request(&testkit.latest_anchored_tx()),
    ]);
    testkit.create_block();

    let last_block_hash = testkit.last_block_hash();
    let (_, signatures) = testkit.gen_anchoring_tx_with_signatures(
        Height(10),
        last_block_hash,
        &[],
        None,
        &anchoring_addr,
    );
    let anchored_tx = testkit.latest_anchored_tx();

    testkit.mempool().contains_key(&signatures[0].hash());
    requests.expect(vec![get_transaction_request(&anchored_tx.clone())]);
    testkit.create_block_with_transactions(signatures);

    let txs = (0..4)
        .map(|idx| {
            gen_service_tx_lect(testkit, ValidatorId(idx), &anchored_tx, 2)
        })
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&txs[0].hash());
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&anchored_tx, &anchoring_addr, 100)
            ]
        },
        get_transaction_request(&anchored_tx),
    ]);
    testkit.create_block_with_transactions(txs);
}

/// Anchor genesis block using funding tx
pub fn anchor_first_block_without_other_signatures(testkit: &mut AnchoringTestKit) {
    let requests = testkit.requests();
    let anchoring_addr = testkit.current_addr();

    requests.expect(vec![
        confirmations_request(&testkit.current_funding_tx(), 50),
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&testkit.current_funding_tx(), &anchoring_addr, 50)
            ]
        },
        get_transaction_request(&testkit.current_funding_tx()),
    ]);

    let last_block_hash = testkit.last_block_hash();
    let (_, mut signatures) = testkit.gen_anchoring_tx_with_signatures(
        Height::zero(),
        last_block_hash,
        &[],
        None,
        &anchoring_addr,
    );
    testkit.create_block();

    testkit.mempool().contains_key(&signatures[0].hash());
    requests.expect(vec![
        confirmations_request(&testkit.current_funding_tx(), 50),
    ]);
    testkit.create_block_with_transactions(signatures.drain(0..1));
}

/// Invoke this method after anchor_first_block_lect_normal
pub fn exclude_node_from_validators(testkit: &mut AnchoringTestKit) {
    let cfg_change_height = Height(12);
    let (cfg_proposal, following_cfg) =
        gen_following_cfg_exclude_validator(testkit, cfg_change_height);
    let (_, following_addr) = following_cfg.redeem_script();

    // Tx has not enough confirmations.
    let anchored_tx = testkit.latest_anchored_tx();

    let requests = testkit.requests();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    testkit.commit_configuration_change(cfg_proposal);
    testkit.create_block();

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = testkit.gen_anchoring_tx_with_signatures(
        Height::zero(),
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = testkit.latest_anchored_tx();
    // Tx gets enough confirmations.
    requests.expect(vec![confirmations_request(&anchored_tx, 100)]);
    testkit.create_block();
    testkit.mempool().contains_key(&signatures[0].hash());

    requests.expect(send_raw_transaction_requests(&transition_tx));
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(testkit, ValidatorId(id), &transition_tx, 2)
        })
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());
    requests.expect(vec![confirmations_request(&transition_tx, 100)]);
    testkit.create_block_with_transactions(lects);
    testkit.create_blocks_until(cfg_change_height.previous());

    testkit.nodes_mut().swap_remove(0);
    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block();

    assert_eq!(testkit.take_handler_errors(), Vec::new());
}

/// Generates a configuration that excludes `testkit node` from consensus.
/// Then it continues to work as auditor.
fn gen_following_cfg_exclude_validator(
    testkit: &mut AnchoringTestKit,
    from_height: Height,
) -> (TestNetworkConfiguration, AnchoringConfig) {
    let mut cfg = testkit.configuration_change_proposal();
    let mut service_cfg: AnchoringConfig = cfg.service_config(ANCHORING_SERVICE_NAME);
    let priv_keys = testkit.current_priv_keys();
    service_cfg.anchoring_keys.swap_remove(0);

    let following_addr = service_cfg.redeem_script().1;
    for (id, ref mut node) in testkit.nodes_mut().iter_mut().enumerate() {
        node.private_keys.insert(
            following_addr.to_string(),
            priv_keys[id].clone(),
        );
    }

    cfg.set_actual_from(from_height);
    let mut validators = cfg.validators().to_vec();
    validators.swap_remove(0);
    cfg.set_validators(validators);
    cfg.set_service_config(ANCHORING_SERVICE_NAME, service_cfg.clone());
    (cfg, service_cfg)
}
