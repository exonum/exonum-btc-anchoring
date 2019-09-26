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

use btc_transaction_utils::{p2wsh, TxInRef};
use exonum::helpers::Height;
use exonum_btc_anchoring::{
    api::{PrivateApi, PublicApi},
    blockchain::{BtcAnchoringSchema, SignInput},
    btc,
    test_helpers::testkit::{AnchoringTestKit, ValidateProof, ANCHORING_INSTANCE_NAME},
};
use futures::Future;

fn find_transaction(
    anchoring_testkit: &AnchoringTestKit,
    height: Option<Height>,
) -> Option<btc::Transaction> {
    let api = anchoring_testkit.inner.api();
    api.find_transaction(height).unwrap().map(|proof| {
        proof
            .validate(&anchoring_testkit.inner.consensus_config())
            .unwrap()
            .1
    })
}

#[test]
fn actual_address() {
    let mut anchoring_testkit = AnchoringTestKit::default();
    let anchoring_interval = anchoring_testkit
        .actual_anchoring_config()
        .anchoring_interval;

    assert!(anchoring_testkit.last_anchoring_tx().is_none());

    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );
    anchoring_testkit
        .inner
        .create_blocks_until(Height(anchoring_interval));

    let anchoring_api = anchoring_testkit.inner.api();
    assert_eq!(
        anchoring_api.actual_address().unwrap(),
        anchoring_testkit
            .actual_anchoring_config()
            .anchoring_address()
    );
}

// #[test]
// fn following_address() {
//     let validators_num = 5;
//     let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, 150_000, 4);
//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(3)
//         .unwrap();
//     anchoring_testkit.create_block_with_transactions(signatures);
//     anchoring_testkit.create_blocks_until(Height(4));
//     // Following address should be none for regular anchoring.
//     assert_eq!(anchoring_testkit.api().following_address().unwrap(), None);

//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(4)
//         .unwrap();
//     anchoring_testkit.create_block_with_transactions(signatures);
//     anchoring_testkit.create_blocks_until(Height(6));

//     // Removing one of validators
//     let mut proposal = anchoring_testkit.drop_validator_proposal();
//     let service_config: GlobalConfig = proposal.service_config(ANCHORING_INSTANCE_NAME);
//     // Following address
//     let following_address = service_config.anchoring_address();
//     proposal.set_actual_from(Height(16));
//     anchoring_testkit.commit_configuration_change(proposal);
//     anchoring_testkit.create_block();

//     assert_eq!(
//         anchoring_testkit.api().following_address().unwrap(),
//         Some(following_address)
//     );
// }

#[test]
fn find_transaction_regular() {
    let anchoring_interval = 4;
    let mut anchoring_testkit = AnchoringTestKit::new(4, 70_000, anchoring_interval);
    // Create a several anchoring transactions
    for i in 1..=5 {
        anchoring_testkit.inner.create_block_with_transactions(
            anchoring_testkit
                .create_signature_txs()
                .into_iter()
                .flatten(),
        );
        anchoring_testkit
            .inner
            .create_blocks_until(Height(anchoring_interval * i));
    }

    let snapshot = anchoring_testkit.inner.snapshot();
    let anchoring_schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
    let tx_chain = anchoring_schema.anchoring_transactions_chain();

    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(0))).unwrap(),
        tx_chain.get(0).unwrap()
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(3))).unwrap(),
        tx_chain.get(1).unwrap()
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(4))).unwrap(),
        tx_chain.get(1).unwrap()
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(1000))).unwrap(),
        tx_chain.get(4).unwrap()
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, None).unwrap(),
        tx_chain.last().unwrap()
    );
}

// // Checks come corner cases in the find_transaction api method.
// #[test]
// fn find_transaction_configuration_change() {
//     let validators_num = 5;
//     let anchoring_frequency = 10;
//     let mut anchoring_testkit =
//         AnchoringTestKit::new_without_rpc(validators_num, 150_000, anchoring_frequency);
//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(4)
//         .unwrap();
//     anchoring_testkit.create_block_with_transactions(signatures);

//     // removing one of validators
//     let mut proposal = anchoring_testkit.drop_validator_proposal();
//     proposal.set_actual_from(Height(20));
//     anchoring_testkit.commit_configuration_change(proposal);
//     anchoring_testkit.create_block();
//     anchoring_testkit.renew_address();
//     // Creates transition transaction
//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(3)
//         .unwrap();
//     anchoring_testkit.create_block_with_transactions(signatures);
//     anchoring_testkit.create_block();

//     let anchoring_schema = anchoring_testkit.schema();
//     assert_eq!(
//         find_transaction(&anchoring_testkit, Some(Height(0))),
//         anchoring_schema.anchoring_transactions_chain().get(1)
//     );
//     assert_eq!(
//         find_transaction(&anchoring_testkit, Some(Height(1))),
//         anchoring_schema.anchoring_transactions_chain().get(1)
//     );
//     assert_eq!(
//         find_transaction(&anchoring_testkit, None),
//         anchoring_schema.anchoring_transactions_chain().get(1)
//     );

//     anchoring_testkit.create_blocks_until(Height(20));
//     // Resumes regular anchoring (anchors block on height 10).
//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(3)
//         .unwrap();
//     anchoring_testkit.create_block_with_transactions(signatures);
//     anchoring_testkit.create_block();

//     let anchoring_schema = anchoring_testkit.schema();
//     assert_eq!(
//         find_transaction(&anchoring_testkit, Some(Height(0))),
//         anchoring_schema.anchoring_transactions_chain().get(1)
//     );
//     assert_eq!(
//         find_transaction(&anchoring_testkit, Some(Height(1))),
//         anchoring_schema.anchoring_transactions_chain().get(2)
//     );
//     assert_eq!(
//         find_transaction(&anchoring_testkit, Some(Height(10))),
//         anchoring_schema.anchoring_transactions_chain().get(2)
//     );
//     assert_eq!(
//         find_transaction(&anchoring_testkit, None),
//         anchoring_schema.anchoring_transactions_chain().get(2)
//     );

//     // Anchors block on height 20.
//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(3)
//         .unwrap();
//     anchoring_testkit.create_block_with_transactions(signatures);
//     anchoring_testkit.create_block();

//     let anchoring_schema = anchoring_testkit.schema();
//     assert_eq!(
//         find_transaction(&anchoring_testkit, Some(Height(0))),
//         anchoring_schema.anchoring_transactions_chain().get(1)
//     );
//     assert_eq!(
//         find_transaction(&anchoring_testkit, Some(Height(1))),
//         anchoring_schema.anchoring_transactions_chain().get(2)
//     );
//     assert_eq!(
//         find_transaction(&anchoring_testkit, Some(Height(10))),
//         anchoring_schema.anchoring_transactions_chain().get(2)
//     );
//     assert_eq!(
//         find_transaction(&anchoring_testkit, Some(Height(11))),
//         anchoring_schema.anchoring_transactions_chain().get(3)
//     );
//     assert_eq!(
//         find_transaction(&anchoring_testkit, None),
//         anchoring_schema.anchoring_transactions_chain().get(3)
//     );
// }

// Try to get a proof of existence for an anchored block.
#[test]
fn block_header_proof() {
    let anchoring_interval = 4;
    let mut anchoring_testkit = AnchoringTestKit::new(4, 70_000, anchoring_interval);
    // Create a several anchoring transactions
    for i in 1..=5 {
        anchoring_testkit.inner.create_block_with_transactions(
            anchoring_testkit
                .create_signature_txs()
                .into_iter()
                .flatten(),
        );
        anchoring_testkit
            .inner
            .create_blocks_until(Height(anchoring_interval * i));
    }

    let api = anchoring_testkit.inner.api();
    let cfg = anchoring_testkit.inner.consensus_config();
    // Check proof for the genesis block.
    let genesis_block_proof = api.block_header_proof(Height(0)).unwrap();
    let value = genesis_block_proof.validate(&cfg).unwrap();
    assert_eq!(value.0, 0);
    assert_eq!(value.1, anchoring_testkit.block_hash_on_height(Height(0)));
    // Check proof for the second block.
    let second_block_proof = api.block_header_proof(Height(4)).unwrap();
    let value = second_block_proof.validate(&cfg).unwrap();
    assert_eq!(value.0, 4);
    assert_eq!(value.1, anchoring_testkit.block_hash_on_height(Height(4)));
}

#[test]
fn actual_config() {
    let anchoring_testkit = AnchoringTestKit::default();
    let cfg = anchoring_testkit.actual_anchoring_config();

    let api = anchoring_testkit.inner.api();
    assert_eq!(PublicApi::config(&api).wait().unwrap(), cfg);
    assert_eq!(PrivateApi::config(&api).wait().unwrap(), cfg);
}

#[test]
fn anchoring_proposal_ok() {
    let anchoring_testkit = AnchoringTestKit::default();
    let proposal = anchoring_testkit
        .anchoring_transaction_proposal()
        .unwrap()
        .0;

    let api = anchoring_testkit.inner.api();
    assert_eq!(api.anchoring_proposal().wait().unwrap(), Some(proposal));
}

#[test]
fn anchoring_proposal_none() {
    let mut anchoring_testkit = AnchoringTestKit::default();

    // Establish anchoring transactions chain.
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );

    let api = anchoring_testkit.inner.api();
    assert!(api.anchoring_proposal().wait().unwrap().is_none());
}

#[test]
fn anchoring_proposal_err_insufficient_funds() {
    let anchoring_testkit = AnchoringTestKit::new(4, 100, 5);

    let api = anchoring_testkit.inner.api();
    let e = api.anchoring_proposal().wait().unwrap_err();
    assert!(e.to_string().contains("Insufficient funds"));
}

#[test]
fn anchoring_sign_input() {
    let mut anchoring_testkit = AnchoringTestKit::new(1, 10_000, 5);

    let config = anchoring_testkit.actual_anchoring_config();
    let bitcoin_public_key = config.anchoring_keys[0].bitcoin_key;
    let bitcoin_private_key = anchoring_testkit.node_private_key(&bitcoin_public_key);
    // Create sign input transaction
    let redeem_script = config.redeem_script();

    let (proposal, proposal_inputs) = anchoring_testkit.anchoring_transaction_proposal().unwrap();
    let proposal_input = &proposal_inputs[0];

    let signature = p2wsh::InputSigner::new(redeem_script)
        .sign_input(
            TxInRef::new(proposal.as_ref(), 0),
            proposal_input.as_ref(),
            &bitcoin_private_key.0.key,
        )
        .unwrap();

    let tx_hash = anchoring_testkit
        .inner
        .api()
        .sign_input(SignInput {
            transaction: proposal,
            input: 0,
            input_signature: signature.into(),
        })
        .wait()
        .unwrap();

    anchoring_testkit
        .inner
        .create_block_with_tx_hashes(&[tx_hash])[0]
        .status()
        .expect("Transaction should be successful");
}
