// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSEccccc//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use hex::FromHex;

use exonum::{crypto::Hash, helpers::Height};
use exonum_bitcoinrpc as bitcoin_rpc;
use exonum_btc_anchoring::{
    blockchain::BtcAnchoringSchema,
    btc::Transaction,
    rpc::TransactionInfo as BtcTransactionInfo,
    test_helpers::{
        rpc::{FakeRelayRequest, FakeRelayResponse, TestRequest},
        testkit::{AnchoringTestKit, ANCHORING_INSTANCE_NAME},
    },
};

fn funding_tx_request() -> TestRequest {
    (
        FakeRelayRequest::TransactionInfo {
            id: Hash::from_hex("69ef1d6847712089783bf861342568625e1e4a499993f27e10d9bb5f259d0894")
                .unwrap(),
        },
        FakeRelayResponse::TransactionInfo(Ok(Some(BtcTransactionInfo {
            content: Transaction::from_hex(
                "02000000000101140b3f5da041f173d938b8fe778d39cb2ef801f75f294\
                 6e490e34d6bb47bb9ce0000000000feffffff0230025400000000001600\
                 14169fa44a9159f281122bb7f3d43d88d56dfa937e70110100000000002\
                 200203abcf8339d06564a151942c35e4a59eee2581e3880bceb84a324e2\
                 237f19ceb502483045022100e91d46b565f26641b353591d0c403a05ada\
                 5735875fb0f055538bf9df4986165022044b5336772de8c5f6cbf83bcc7\
                 099e31d7dce22ba1f3d1badc2fdd7f8013a12201210254053f15b44b825\
                 bc5dabfe88f8b94cd217372f3f297d2696a32835b43497397358d1400",
            )
            .unwrap(),
            confirmations: 6,
        }))),
    )
}

// #[test]
// fn normal_operation() {
//     let mut anchoring_testkit = AnchoringTestKit::new_with_fake_rpc(4);
//     let requests = anchoring_testkit.requests();

//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(2)
//         .unwrap();

//     let (proposed, _) =
//         BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &anchoring_testkit.snapshot())
//             .actual_proposed_anchoring_transaction()
//             .unwrap()
//             .unwrap();

//     let anchoring_tx_id = proposed.id();
//     anchoring_testkit.create_block_with_transactions(signatures);

//     // Error while trying fetch info for anchoring tx first time
//     requests.expect(vec![
//         funding_tx_request(),
//         (
//             FakeRelayRequest::TransactionInfo {
//                 id: anchoring_tx_id,
//             },
//             FakeRelayResponse::TransactionInfo(Err(
//                 bitcoin_rpc::Error::Memory(String::new()).into()
//             )),
//         ),
//     ]);

//     anchoring_testkit.create_blocks_until(Height(2));

//     let last_tx = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &anchoring_testkit.snapshot())
//         .anchoring_transactions_chain()
//         .last()
//         .unwrap();

//     // Should retry
//     requests.expect(vec![
//         funding_tx_request(),
//         (
//             FakeRelayRequest::TransactionInfo {
//                 id: anchoring_tx_id,
//             },
//             FakeRelayResponse::TransactionInfo(Ok(None)),
//         ),
//         (
//             FakeRelayRequest::SendTransaction {
//                 transaction: last_tx.clone(),
//             },
//             FakeRelayResponse::SendTransaction(Ok(anchoring_tx_id)),
//         ),
//     ]);

//     anchoring_testkit.create_blocks_until(Height(4));

//     // Should ask btc network about last anchoring tx every anchoring_height / 2
//     requests.expect(vec![
//         funding_tx_request(),
//         (
//             FakeRelayRequest::TransactionInfo {
//                 id: anchoring_tx_id,
//             },
//             FakeRelayResponse::TransactionInfo(Ok(Some(BtcTransactionInfo {
//                 content: last_tx.clone(),
//                 confirmations: 6,
//             }))),
//         ),
//         funding_tx_request(),
//         (
//             FakeRelayRequest::TransactionInfo {
//                 id: anchoring_tx_id,
//             },
//             FakeRelayResponse::TransactionInfo(Ok(Some(BtcTransactionInfo {
//                 content: last_tx.clone(),
//                 confirmations: 6,
//             }))),
//         ),
//     ]);
//     anchoring_testkit.create_blocks_until(Height(8));
// }

// #[test]
// fn several_unsynced() {
//     let mut anchoring_testkit = AnchoringTestKit::new_with_fake_rpc(4);
//     let requests = anchoring_testkit.requests();

//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(3)
//         .unwrap();

//     let (proposed_0, _) =
//         BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &anchoring_testkit.snapshot())
//             .actual_proposed_anchoring_transaction()
//             .unwrap()
//             .unwrap();

//     let tx_id_0 = proposed_0.id();
//     anchoring_testkit.create_block_with_transactions(signatures);

//     // Error while trying fetch info for anchoring tx first time
//     requests.expect(vec![
//         funding_tx_request(),
//         (
//             FakeRelayRequest::TransactionInfo { id: tx_id_0 },
//             FakeRelayResponse::TransactionInfo(Err(
//                 bitcoin_rpc::Error::Memory(String::new()).into()
//             )),
//         ),
//     ]);

//     anchoring_testkit.create_blocks_until(Height(2));

//     let last_tx = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &anchoring_testkit.snapshot())
//         .anchoring_transactions_chain()
//         .last()
//         .unwrap();

//     // Sync failed
//     requests.expect(vec![
//         funding_tx_request(),
//         (
//             FakeRelayRequest::TransactionInfo { id: tx_id_0 },
//             FakeRelayResponse::TransactionInfo(Ok(None)),
//         ),
//         (
//             FakeRelayRequest::SendTransaction {
//                 transaction: last_tx.clone(),
//             },
//             FakeRelayResponse::SendTransaction(Ok(tx_id_0)),
//         ),
//     ]);

//     anchoring_testkit.create_blocks_until(Height(5));

//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(3)
//         .unwrap();

//     let (proposed_1, _) =
//         BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &anchoring_testkit.snapshot())
//             .actual_proposed_anchoring_transaction()
//             .unwrap()
//             .unwrap();

//     let tx_id_1 = proposed_1.id();

//     requests.expect(vec![
//         (
//             FakeRelayRequest::TransactionInfo { id: tx_id_0 },
//             FakeRelayResponse::TransactionInfo(Ok(None)),
//         ),
//         funding_tx_request(),
//         (
//             FakeRelayRequest::TransactionInfo { id: tx_id_0 },
//             FakeRelayResponse::TransactionInfo(Ok(None)),
//         ),
//         (
//             FakeRelayRequest::SendTransaction {
//                 transaction: last_tx.clone(),
//             },
//             FakeRelayResponse::SendTransaction(Err(
//                 bitcoin_rpc::Error::Memory(String::new()).into()
//             )),
//         ),
//         (
//             FakeRelayRequest::TransactionInfo { id: tx_id_0 },
//             FakeRelayResponse::TransactionInfo(Ok(None)),
//         ),
//         funding_tx_request(),
//         (
//             FakeRelayRequest::TransactionInfo { id: tx_id_0 },
//             FakeRelayResponse::TransactionInfo(Ok(None)),
//         ),
//         (
//             FakeRelayRequest::SendTransaction {
//                 transaction: last_tx.clone(),
//             },
//             FakeRelayResponse::SendTransaction(Err(
//                 bitcoin_rpc::Error::Memory(String::new()).into()
//             )),
//         ),
//     ]);

//     anchoring_testkit.create_block_with_transactions(signatures);

//     anchoring_testkit.create_blocks_until(Height(9));
//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(3)
//         .unwrap();

//     // Should walk to first uncommitted
//     requests.expect(vec![
//         (
//             FakeRelayRequest::TransactionInfo { id: tx_id_1 },
//             FakeRelayResponse::TransactionInfo(Ok(None)),
//         ),
//         (
//             FakeRelayRequest::TransactionInfo { id: tx_id_0 },
//             FakeRelayResponse::TransactionInfo(Ok(None)),
//         ),
//         funding_tx_request(),
//         (
//             FakeRelayRequest::TransactionInfo { id: tx_id_0 },
//             FakeRelayResponse::TransactionInfo(Ok(None)),
//         ),
//         (
//             FakeRelayRequest::SendTransaction {
//                 transaction: last_tx.clone(),
//             },
//             FakeRelayResponse::SendTransaction(Err(
//                 bitcoin_rpc::Error::Memory(String::new()).into()
//             )),
//         ),
//     ]);

//     anchoring_testkit.create_block_with_transactions(signatures);
//     anchoring_testkit.create_blocks_until(Height(11));
// }
