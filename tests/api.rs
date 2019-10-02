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
    api::{AnchoringProposalState, PrivateApi, PublicApi},
    blockchain::{BtcAnchoringSchema, SignInput},
    btc,
    test_helpers::testkit::{
        AnchoringTestKit, ValidateProof, ANCHORING_INSTANCE_ID, ANCHORING_INSTANCE_NAME,
    },
};
use exonum_testkit::simple_supervisor::ConfigPropose;
use futures::Future;

fn find_transaction(
    anchoring_testkit: &AnchoringTestKit,
    height: Option<Height>,
) -> Option<btc::Transaction> {
    let api = anchoring_testkit.inner.api();
    let proof = api.find_transaction(height).unwrap();
    proof
        .validate(&anchoring_testkit.inner.consensus_config())
        .unwrap()
        .map(|x| x.1)
}

fn transaction_with_index(
    anchoring_testkit: &AnchoringTestKit,
    index: u64,
) -> Option<btc::Transaction> {
    anchoring_testkit
        .inner
        .api()
        .transaction_with_index(index)
        .unwrap()
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

#[test]
fn following_address() {
    let mut anchoring_testkit = AnchoringTestKit::new(4, 2_000, 5);
    let anchoring_interval = anchoring_testkit
        .actual_anchoring_config()
        .anchoring_interval;

    assert!(anchoring_testkit.last_anchoring_tx().is_none());
    // Establish anchoring transactions chain.
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );

    // Skip the next anchoring height.
    anchoring_testkit
        .inner
        .create_blocks_until(Height(anchoring_interval * 2));

    // Add an anchoring node.
    let mut new_cfg = anchoring_testkit.actual_anchoring_config();
    new_cfg.anchoring_keys.push(anchoring_testkit.add_node());
    new_cfg.funding_transaction = None;
    let following_address = new_cfg.anchoring_address();

    // Commit configuration with without last anchoring node.
    anchoring_testkit.inner.create_block_with_transaction(
        ConfigPropose::actual_from(anchoring_testkit.inner.height().next())
            .service_config(ANCHORING_INSTANCE_ID, new_cfg.clone())
            .into_tx(),
    );
    anchoring_testkit.inner.create_block();

    assert_eq!(
        anchoring_testkit.inner.api().following_address().unwrap(),
        Some(following_address)
    );
}

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
    // Find transactions by height.
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
    // Find transactions by index.
    for i in 0..=tx_chain.len() {
        assert_eq!(
            transaction_with_index(&anchoring_testkit, i),
            tx_chain.get(i)
        );
    }
}

// Check come edge cases in the find_transaction api method.
#[test]
fn find_transaction_configuration_change() {
    let anchoring_interval = 5;
    let mut anchoring_testkit = AnchoringTestKit::new(4, 150_000, anchoring_interval);
    let anchoring_interval = anchoring_testkit
        .actual_anchoring_config()
        .anchoring_interval;

    assert!(anchoring_testkit.last_anchoring_tx().is_none());
    // Establish anchoring transactions chain.
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );

    // Skip the next anchoring height.
    anchoring_testkit
        .inner
        .create_blocks_until(Height(anchoring_interval * 2));

    // Add an anchoring node.
    let mut new_cfg = anchoring_testkit.actual_anchoring_config();
    new_cfg.anchoring_keys.push(anchoring_testkit.add_node());
    new_cfg.funding_transaction = None;

    // Commit configuration with without last anchoring node.
    anchoring_testkit.inner.create_block_with_transaction(
        ConfigPropose::actual_from(anchoring_testkit.inner.height().next())
            .service_config(ANCHORING_INSTANCE_ID, new_cfg.clone())
            .into_tx(),
    );
    anchoring_testkit.inner.create_block();

    // Transit to the new address.
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );

    let snapshot = anchoring_testkit.inner.snapshot();
    let anchoring_schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(0))),
        anchoring_schema.anchoring_transactions_chain().get(1)
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(1))),
        anchoring_schema.anchoring_transactions_chain().get(1)
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, None),
        anchoring_schema.anchoring_transactions_chain().get(1)
    );

    // Resume regular anchoring (anchors block on height 5).
    anchoring_testkit
        .inner
        .create_blocks_until(Height(anchoring_interval * 2));
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );

    let snapshot = anchoring_testkit.inner.snapshot();
    let anchoring_schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(0))),
        anchoring_schema.anchoring_transactions_chain().get(1)
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(1))),
        anchoring_schema.anchoring_transactions_chain().get(2)
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(10))),
        anchoring_schema.anchoring_transactions_chain().get(2)
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, None),
        anchoring_schema.anchoring_transactions_chain().get(2)
    );

    // Anchors block on height 10.
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );

    let snapshot = anchoring_testkit.inner.snapshot();
    let anchoring_schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(0))),
        anchoring_schema.anchoring_transactions_chain().get(1)
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(1))),
        anchoring_schema.anchoring_transactions_chain().get(2)
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(5))),
        anchoring_schema.anchoring_transactions_chain().get(2)
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, Some(Height(6))),
        anchoring_schema.anchoring_transactions_chain().get(3)
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, None),
        anchoring_schema.anchoring_transactions_chain().get(3)
    );
}

#[test]
fn actual_config() {
    let anchoring_testkit = AnchoringTestKit::default();
    let cfg = anchoring_testkit.actual_anchoring_config();

    let api = anchoring_testkit.inner.api();
    assert_eq!(PublicApi::config(&api).unwrap(), cfg);
    assert_eq!(PrivateApi::config(&api).unwrap(), cfg);
}

#[test]
fn anchoring_proposal_ok() {
    let anchoring_testkit = AnchoringTestKit::default();
    let proposal = anchoring_testkit.anchoring_transaction_proposal().unwrap();

    let api = anchoring_testkit.inner.api();
    assert_eq!(
        api.anchoring_proposal().unwrap(),
        AnchoringProposalState::Available {
            transaction: proposal.0,
            inputs: proposal.1,
        }
    );
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
    assert_eq!(
        api.anchoring_proposal().unwrap(),
        AnchoringProposalState::None
    );
}

#[test]
fn anchoring_proposal_err_insufficient_funds() {
    let anchoring_testkit = AnchoringTestKit::new(4, 100, 5);

    let api = anchoring_testkit.inner.api();
    let state = api.anchoring_proposal().unwrap();
    assert_eq!(
        state,
        AnchoringProposalState::InsufficientFunds {
            total_fee: 1530,
            balance: 100
        }
    );
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
