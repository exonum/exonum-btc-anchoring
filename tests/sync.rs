// Copyright 2018 The Exonum Team
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

extern crate bitcoin;
extern crate exonum;

extern crate exonum_bitcoinrpc as bitcoin_rpc;
#[macro_use]
extern crate exonum_btc_anchoring;
extern crate exonum_testkit;

#[macro_use]
extern crate serde_json;

extern crate btc_transaction_utils;
extern crate rand;

use exonum::helpers::Height;
use exonum_btc_anchoring::blockchain::BtcAnchoringSchema;
use exonum_btc_anchoring::test_helpers::testkit::AnchoringTestKit;

macro_rules! funding_tx_request {
    () => {
    request! {
        method: "getrawtransaction",
        params: [
            "69ef1d6847712089783bf861342568625e1e4a499993f27e10d9bb5f259d0894",
            1
        ],
        response: {
            "hash": "8aa76e05d95c7fec389561a99765543ca27031a262063030035d82a02f2050d3",
            "hex": "02000000000101140b3f5da041f173d938b8fe778d39cb2ef801f75f294\
                    6e490e34d6bb47bb9ce0000000000feffffff0230025400000000001600\
                    14169fa44a9159f281122bb7f3d43d88d56dfa937e70110100000000002\
                    200203abcf8339d06564a151942c35e4a59eee2581e3880bceb84a324e2\
                    237f19ceb502483045022100e91d46b565f26641b353591d0c403a05ada\
                    5735875fb0f055538bf9df4986165022044b5336772de8c5f6cbf83bcc7\
                    099e31d7dce22ba1f3d1badc2fdd7f8013a12201210254053f15b44b825\
                    bc5dabfe88f8b94cd217372f3f297d2696a32835b43497397358d1400",
            "locktime": 1346869,
            "size": 235,
            "txid": "69ef1d6847712089783bf861342568625e1e4a499993f27e10d9bb5f259d0894",
            "version": 2,
            "vin": [
                {
                    "scriptSig": {
                        "asm": "",
                        "hex": ""
                    },
                    "sequence": 4294967294u64,
                    "txid": "ceb97bb46b4de390e446295ff701f82ecb398d77feb838d973f141a05d3f0b14",
                    "txinwitness": [
                        "3045022100e91d46b565f26641b353591d0c403a05ada5735875fb0f055538bf9df4986165022044b5336772de8c5f6cbf83bcc7099e31d7dce22ba1f3d1badc2fdd7f8013a12201",
                        "0254053f15b44b825bc5dabfe88f8b94cd217372f3f297d2696a32835b43497397"
                    ],
                    "vout": 0
                }
            ],
            "vout": [
                {
                    "n": 0,
                    "scriptPubKey": {
                        "addresses": [
                            "tb1qz606gj53t8egzy3tkleag0vg64kl4ym7hws0u8"
                        ],
                        "asm": "0 169fa44a9159f281122bb7f3d43d88d56dfa937e",
                        "hex": "0014169fa44a9159f281122bb7f3d43d88d56dfa937e",
                        "reqSigs": 1,
                        "type": "witness_v0_keyhash"
                    },
                    "value": 0.05505584
                },
                {
                    "n": 1,
                    "scriptPubKey": {
                        "addresses": [
                            "tb1q8270svuaqety59gegtp4ujjeam39s83csz7whp9ryn3zxlcee66setkyq0"
                        ],
                        "asm": "0 3abcf8339d06564a151942c35e4a59eee2581e3880bceb84a324e2237f19ceb5",
                        "hex": "00203abcf8339d06564a151942c35e4a59eee2581e3880bceb84a324e2237f19ceb5",
                        "reqSigs": 1,
                        "type": "witness_v0_scripthash"
                    },
                    "value": 0.0007
                }
            ],
            "vsize": 153
        }
    };
    }
}

#[test]
fn normal_operation() {
    let mut anchoring_testkit = AnchoringTestKit::new_with_fake_rpc(4, 7000, 4);
    let requests = anchoring_testkit.requests();

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(2)
        .unwrap();

    let schema = BtcAnchoringSchema::new(anchoring_testkit.snapshot());
    let (proposed, _) = schema
        .actual_proposed_anchoring_transaction()
        .unwrap()
        .unwrap();

    let anchoring_tx_id = proposed.id().to_hex();
    anchoring_testkit.create_block_with_transactions(signatures);

    // error white trying fetch info for anchoring  tx first time
    requests.expect(vec![
        funding_tx_request!{},
        request! {
            method: "getrawtransaction",
            params: [ anchoring_tx_id, 1],
            error: bitcoin_rpc::Error::Memory(String::new())
        },
    ]);

    anchoring_testkit.create_blocks_until(Height(2));

    let schema = BtcAnchoringSchema::new(anchoring_testkit.snapshot());
    let last_tx = schema.anchoring_transactions_chain().last().unwrap();

    // should retry
    requests.expect(vec![
        funding_tx_request!{},
        request! {
            method: "getrawtransaction",
            params: [ anchoring_tx_id, 1],
            error: bitcoin_rpc::Error::NoInformation(String::new())
        },
        request! {
            method: "sendrawtransaction",
            params: [ last_tx.to_string() ],
            response: anchoring_tx_id
        },
    ]);

    anchoring_testkit.create_blocks_until(Height(4));

    // should ask btc network about last anchoring tx every anchoring_height / 2
    requests.expect(vec![
        funding_tx_request!{},
        request! {
            method: "getrawtransaction",
            params: [ anchoring_tx_id, 1],
            response:  {
                "txid": "1c87d930767143fd6f1b4616a5e84d4cf50c0705aca49f74fefb6b89c820513c",
                "hash": "5bf1134da19f25a5d7b7df7f211eb54839c4f9407e777b407e485898ea13f13a",
                "hex": proposed.to_string(),
                "version": 2,
                "size": 515,
                "vsize": 244,
                "locktime": 0,
                "vin": [],
                "vout": []
            }
        },
        funding_tx_request!{},
        request! {
            method: "getrawtransaction",
            params: [ anchoring_tx_id, 1],
            response:  {
                "txid": "1c87d930767143fd6f1b4616a5e84d4cf50c0705aca49f74fefb6b89c820513c",
                "hash": "5bf1134da19f25a5d7b7df7f211eb54839c4f9407e777b407e485898ea13f13a",
                "hex": proposed.to_string(),
                "version": 2,
                "size": 515,
                "vsize": 244,
                "locktime": 0,
                "vin": [],
                "vout": []
            }

        },
    ]);
    anchoring_testkit.create_blocks_until(Height(8));
}

#[test]
fn several_unsynced() {
    let mut anchoring_testkit = AnchoringTestKit::new_with_fake_rpc(4, 7000, 4);
    let requests = anchoring_testkit.requests();

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();

    let schema = BtcAnchoringSchema::new(anchoring_testkit.snapshot());
    let (proposed_0, _) = schema
        .actual_proposed_anchoring_transaction()
        .unwrap()
        .unwrap();

    let tx_id_0 = proposed_0.id().to_hex();
    anchoring_testkit.create_block_with_transactions(signatures);

    // error white trying fetch info for anchoring  tx first time
    requests.expect(vec![
        funding_tx_request!{},
        request! {
            method: "getrawtransaction",
            params: [ tx_id_0, 1],
            error: bitcoin_rpc::Error::Memory(String::new())
        },
    ]);

    anchoring_testkit.create_blocks_until(Height(2));

    let schema = BtcAnchoringSchema::new(anchoring_testkit.snapshot());
    let last_tx = schema.anchoring_transactions_chain().last().unwrap();

    // sync failed
    requests.expect(vec![
        funding_tx_request!{},
        request! {
            method: "getrawtransaction",
            params: [ tx_id_0, 1],
            error: bitcoin_rpc::Error::NoInformation(String::new())
        },
        request! {
            method: "sendrawtransaction",
            params: [ last_tx.to_string() ],
            error: bitcoin_rpc::Error::Memory(String::new())
        },
    ]);

    anchoring_testkit.create_blocks_until(Height(5));

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();

    let schema = BtcAnchoringSchema::new(anchoring_testkit.snapshot());
    let (proposed_1, _) = schema
        .actual_proposed_anchoring_transaction()
        .unwrap()
        .unwrap();

    let tx_id_1 = proposed_1.id().to_hex();

    requests.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [ tx_id_0, 1],
            error: bitcoin_rpc::Error::NoInformation(String::new())
        },
        funding_tx_request!{},
        request! {
            method: "getrawtransaction",
            params: [ tx_id_0, 1],
            error: bitcoin_rpc::Error::NoInformation(String::new())
        },
        request! {
            method: "sendrawtransaction",
            params: [ last_tx.to_string() ],
            error: bitcoin_rpc::Error::Memory(String::new())
        },
        request! {
            method: "getrawtransaction",
            params: [ tx_id_0, 1],
            error: bitcoin_rpc::Error::NoInformation(String::new())
        },
        funding_tx_request!{},
        request! {
            method: "getrawtransaction",
            params: [ tx_id_0, 1],
            error: bitcoin_rpc::Error::NoInformation(String::new())
        },
        request! {
            method: "sendrawtransaction",
            params: [ last_tx.to_string() ],
            error: bitcoin_rpc::Error::Memory(String::new())
        },
    ]);

    anchoring_testkit.create_block_with_transactions(signatures);

    anchoring_testkit.create_blocks_until(Height(9));
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();

    // should walk to first uncommitted
    requests.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [ tx_id_1, 1],
            error: bitcoin_rpc::Error::NoInformation(String::new())
        },
        request! {
            method: "getrawtransaction",
            params: [ tx_id_0, 1],
            error: bitcoin_rpc::Error::NoInformation(String::new())
        },
        funding_tx_request!{},
        request! {
            method: "getrawtransaction",
            params: [ tx_id_0, 1],
            error: bitcoin_rpc::Error::NoInformation(String::new())
        },
        request! {
            method: "sendrawtransaction",
            params: [ last_tx.to_string() ],
            error: bitcoin_rpc::Error::Memory(String::new())
        },
    ]);

    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(11));
}
