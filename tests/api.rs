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
    blockchain::SignInput,
    btc,
    test_helpers::{
        create_fake_funding_transaction, get_anchoring_schema, AnchoringTestKit, ValidateProof,
        ANCHORING_INSTANCE_ID,
    },
};
use exonum_supervisor::ConfigPropose;

fn find_transaction(
    anchoring_testkit: &mut AnchoringTestKit,
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
    anchoring_testkit: &mut AnchoringTestKit,
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
    let mut anchoring_testkit = AnchoringTestKit::default();
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
    let following_address = new_cfg.anchoring_address();

    // Commit configuration with without last anchoring node.
    anchoring_testkit.inner.create_block_with_transaction(
        anchoring_testkit.create_config_change_tx(
            ConfigPropose::new(0, anchoring_testkit.inner.height().next())
                .service_config(ANCHORING_INSTANCE_ID, new_cfg),
        ),
    );
    anchoring_testkit.inner.create_block();

    assert_eq!(
        anchoring_testkit.inner.api().following_address().unwrap(),
        Some(following_address)
    );
}

#[test]
fn find_transaction_regular() {
    let mut anchoring_testkit = AnchoringTestKit::default();
    let anchoring_interval = anchoring_testkit
        .actual_anchoring_config()
        .anchoring_interval;

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
    let anchoring_schema = get_anchoring_schema(&snapshot);
    let tx_chain = anchoring_schema.transactions_chain;
    // Find transactions by height.
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(0))).unwrap(),
        tx_chain.get(0).unwrap()
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(3))).unwrap(),
        tx_chain.get(1).unwrap()
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(4))).unwrap(),
        tx_chain.get(1).unwrap()
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(1000))).unwrap(),
        tx_chain.get(4).unwrap()
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, None).unwrap(),
        tx_chain.last().unwrap()
    );
    // Find transactions by index.
    for i in 0..=tx_chain.len() {
        assert_eq!(
            transaction_with_index(&mut anchoring_testkit, i),
            tx_chain.get(i)
        );
    }
}

// Check come edge cases in the find_transaction api method.
#[test]
fn find_transaction_configuration_change() {
    let mut anchoring_testkit = AnchoringTestKit::default();
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

    // Commit configuration with without last anchoring node.
    anchoring_testkit.inner.create_block_with_transaction(
        anchoring_testkit.create_config_change_tx(
            ConfigPropose::new(0, anchoring_testkit.inner.height().next())
                .service_config(ANCHORING_INSTANCE_ID, new_cfg),
        ),
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
    let anchoring_schema = get_anchoring_schema(&snapshot);
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(0))),
        anchoring_schema.transactions_chain.get(1)
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(1))),
        anchoring_schema.transactions_chain.get(1)
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, None),
        anchoring_schema.transactions_chain.get(1)
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
    let anchoring_schema = get_anchoring_schema(&snapshot);
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(0))),
        anchoring_schema.transactions_chain.get(1)
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(1))),
        anchoring_schema.transactions_chain.get(2)
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(10))),
        anchoring_schema.transactions_chain.get(2)
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, None),
        anchoring_schema.transactions_chain.get(2)
    );

    // Anchors block on height 10.
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );

    let snapshot = anchoring_testkit.inner.snapshot();
    let anchoring_schema = get_anchoring_schema(&snapshot);
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(0))),
        anchoring_schema.transactions_chain.get(1)
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(1))),
        anchoring_schema.transactions_chain.get(2)
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(5))),
        anchoring_schema.transactions_chain.get(2)
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, Some(Height(6))),
        anchoring_schema.transactions_chain.get(3)
    );
    assert_eq!(
        find_transaction(&mut anchoring_testkit, None),
        anchoring_schema.transactions_chain.get(3)
    );
}

#[test]
fn actual_config() {
    let mut anchoring_testkit = AnchoringTestKit::default();
    let cfg = anchoring_testkit.actual_anchoring_config();

    let api = anchoring_testkit.inner.api();
    assert_eq!(PublicApi::config(&api).unwrap(), cfg);
    assert_eq!(PrivateApi::config(&api).unwrap(), cfg);
}

#[test]
fn anchoring_proposal_ok() {
    let mut anchoring_testkit = AnchoringTestKit::default();
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
fn anchoring_proposal_err_without_initial_funds() {
    let mut anchoring_testkit = AnchoringTestKit::new(4, 5);

    let api = anchoring_testkit.inner.api();
    let state = api.anchoring_proposal().unwrap();
    assert_eq!(state, AnchoringProposalState::NoInitialFunds);
}

#[test]
fn anchoring_proposal_err_insufficient_funds() {
    let mut anchoring_testkit = AnchoringTestKit::new(4, 5);

    // Add an initial funding transaction to enable anchoring.
    anchoring_testkit
        .inner
        .create_block_with_transactions(anchoring_testkit.create_funding_confirmation_txs(20).0);

    let api = anchoring_testkit.inner.api();
    let state = api.anchoring_proposal().unwrap();
    assert_eq!(
        state,
        AnchoringProposalState::InsufficientFunds {
            total_fee: 1530,
            balance: 20
        }
    );
}

#[test]
fn sign_input() {
    let mut anchoring_testkit = AnchoringTestKit::default();

    let config = anchoring_testkit.actual_anchoring_config();
    let bitcoin_public_key = config
        .find_bitcoin_key(&anchoring_testkit.inner.us().service_keypair().public_key())
        .unwrap()
        .1;
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

    p2wsh::InputSigner::new(config.redeem_script())
        .verify_input(
            TxInRef::new(proposal.as_ref(), 0),
            proposal_input.as_ref(),
            &bitcoin_public_key.0,
            &signature,
        )
        .unwrap();

    let tx_hash = anchoring_testkit
        .inner
        .api()
        .sign_input(SignInput {
            input: 0,
            input_signature: signature.into(),
            txid: proposal.id(),
        })
        .unwrap();

    anchoring_testkit
        .inner
        .create_block_with_tx_hashes(&[tx_hash])[0]
        .status()
        .expect("Transaction should be successful");
}

#[test]
fn add_funds_ok() {
    let anchoring_interval = 5;
    let mut anchoring_testkit = AnchoringTestKit::new(1, anchoring_interval);

    let config = anchoring_testkit.actual_anchoring_config();
    let funding_transaction = create_fake_funding_transaction(&config.anchoring_address(), 10_000);

    let tx_hash = anchoring_testkit
        .inner
        .api()
        .add_funds(funding_transaction)
        .unwrap();

    anchoring_testkit
        .inner
        .create_block_with_tx_hashes(&[tx_hash])[0]
        .status()
        .expect("Transaction should be successful");
}

#[test]
fn add_funds_err_already_used() {
    let anchoring_interval = 5;
    let mut anchoring_testkit = AnchoringTestKit::new(1, anchoring_interval);

    // Add an initial funding transaction to enable anchoring.
    let (txs, funding_transaction) = anchoring_testkit.create_funding_confirmation_txs(2000);
    anchoring_testkit.inner.create_block_with_transactions(txs);

    // Establish anchoring transactions chain.
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );

    anchoring_testkit
        .inner
        .api()
        .add_funds(funding_transaction)
        .expect_err("Add funds must fail");
}

#[test]
fn add_funds_err_unsuitable() {
    let anchoring_interval = 5;
    let mut anchoring_testkit = AnchoringTestKit::new(4, anchoring_interval);

    let mut config = anchoring_testkit.actual_anchoring_config();
    config.anchoring_keys.swap(1, 3);
    let funding_transaction = create_fake_funding_transaction(&config.anchoring_address(), 10_000);

    anchoring_testkit
        .inner
        .api()
        .add_funds(funding_transaction)
        .expect_err("Add funds must fail");
}
