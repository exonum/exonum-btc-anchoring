// Copyright 2019 The Exonum Team
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

use exonum::helpers::Height;
use exonum::{explorer::CommittedTransaction, runtime::ErrorKind};
use exonum_btc_anchoring::{
    blockchain::{errors::Error, BtcAnchoringSchema},
    btc::BuilderError,
    config::Config,
    test_helpers::testkit::{
        create_fake_funding_transaction, AnchoringTestKit, ANCHORING_INSTANCE_ID,
        ANCHORING_INSTANCE_NAME,
    },
};
use exonum_testkit::simple_supervisor::ConfigPropose;

fn assert_tx_error(tx: &CommittedTransaction, e: impl Into<ErrorKind>) {
    assert_eq!(tx.status().unwrap_err().kind, e.into(),);
}

fn test_anchoring_config_change<F>(mut config_change_predicate: F) -> AnchoringTestKit
where
    F: FnMut(&mut AnchoringTestKit, &mut Config),
{
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

    let last_anchoring_tx = anchoring_testkit.last_anchoring_tx().unwrap();
    // Remove one of anchoring nodes.
    let mut new_cfg = anchoring_testkit.actual_anchoring_config();
    let old_cfg = new_cfg.clone();
    config_change_predicate(&mut anchoring_testkit, &mut new_cfg);

    // Commit configuration with without last anchoring node.
    anchoring_testkit.inner.create_block_with_transaction(
        ConfigPropose::actual_from(anchoring_testkit.inner.height().next())
            .service_config(ANCHORING_INSTANCE_ID, new_cfg.clone())
            .into_tx(),
    );

    // Extract a previous anchoring transaction from the proposal.
    let (anchoring_tx_proposal, previous_anchoring_tx) = anchoring_testkit
        .anchoring_transaction_proposal()
        .map(|(tx, inputs)| (tx, inputs[0].clone()))
        .unwrap();

    // Verify anchoring transaction proposal.
    {
        assert_eq!(last_anchoring_tx, previous_anchoring_tx);

        let snapshot = anchoring_testkit.inner.snapshot();
        let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
        assert_eq!(schema.following_configuration().unwrap(), new_cfg);
        assert_eq!(schema.actual_configuration(), old_cfg);

        let (out_script, payload) = anchoring_tx_proposal.anchoring_metadata().unwrap();
        // Height for the transition anchoring transaction should be same as in the latest
        // anchoring transaction.
        assert_eq!(payload.block_height, Height(0));
        assert_eq!(&new_cfg.anchoring_out_script(), out_script);
    }

    // Finalize transition transaction
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );

    // Verify that the following configuration becomes an actual.
    let snapshot = anchoring_testkit.inner.snapshot();
    let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
    assert!(schema.following_configuration().is_none());
    assert_eq!(schema.actual_configuration(), new_cfg);

    assert_eq!(
        anchoring_tx_proposal.id(),
        anchoring_testkit.last_anchoring_tx().unwrap().id()
    );

    // Verify that we have an anchoring transaction proposal.
    let anchoring_tx_proposal = anchoring_testkit
        .anchoring_transaction_proposal()
        .unwrap()
        .0;
    // Verify anchoring transaction metadata
    let tx_meta = anchoring_tx_proposal.anchoring_metadata().unwrap();
    assert_eq!(tx_meta.1.block_height, Height(anchoring_interval));
    assert_eq!(
        anchoring_testkit
            .actual_anchoring_config()
            .anchoring_out_script(),
        *tx_meta.0
    );

    anchoring_testkit
}

#[test]
fn simple() {
    let mut anchoring_testkit = AnchoringTestKit::default();
    let anchoring_interval = anchoring_testkit
        .actual_anchoring_config()
        .anchoring_interval;

    assert!(anchoring_testkit.last_anchoring_tx().is_none());

    let signatures = anchoring_testkit
        .create_signature_txs()
        .into_iter()
        .flatten();

    anchoring_testkit
        .inner
        .create_block_with_transactions(signatures);

    let tx0 = anchoring_testkit.last_anchoring_tx().unwrap();
    let tx0_meta = tx0.anchoring_metadata().unwrap();
    assert!(tx0_meta.1.block_height == Height(0));

    anchoring_testkit
        .inner
        .create_blocks_until(Height(anchoring_interval));

    // Ensure that the anchoring proposal has expected height.
    assert_eq!(
        anchoring_testkit
            .anchoring_transaction_proposal()
            .unwrap()
            .0
            .anchoring_payload()
            .unwrap()
            .block_height,
        Height(anchoring_interval)
    );

    let signatures = anchoring_testkit
        .create_signature_txs()
        .into_iter()
        .take(3)
        .flatten();
    anchoring_testkit
        .inner
        .create_block_with_transactions(signatures);

    let tx1 = anchoring_testkit.last_anchoring_tx().unwrap();
    let tx1_meta = tx1.anchoring_metadata().unwrap();

    assert!(tx0.id() == tx1.prev_tx_id());

    // script_pubkey should be the same
    assert!(tx0_meta.0 == tx1_meta.0);
    assert!(tx1_meta.1.block_height == Height(anchoring_interval));
}

#[test]
fn additional_funding() {
    let mut anchoring_testkit = AnchoringTestKit::default();
    let anchoring_interval = anchoring_testkit
        .actual_anchoring_config()
        .anchoring_interval;

    assert!(anchoring_testkit.last_anchoring_tx().is_none());
    // Establish anchoring transactions chain with the initial funding transaction.
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );

    // Add another funding transaction.
    let (txs, new_funding_tx) = anchoring_testkit.create_funding_confirmation_txs(150_000);
    anchoring_testkit.inner.create_block_with_transactions(txs);

    // Reach the next anchoring height.
    anchoring_testkit
        .inner
        .create_blocks_until(Height(anchoring_interval));

    // Ensure that the anchoring proposal has input with the our funding transaction.
    assert_eq!(
        anchoring_testkit
            .anchoring_transaction_proposal()
            .unwrap()
            .1[1],
        new_funding_tx
    );

    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .take(3)
            .flatten(),
    );

    let tx1 = anchoring_testkit.last_anchoring_tx().unwrap();
    let tx1_meta = tx1.anchoring_metadata().unwrap();

    assert!(tx1_meta.1.block_height == Height(anchoring_interval));
    assert_eq!(tx1.0.input[1].previous_output.txid, new_funding_tx.0.txid());
}

#[test]
fn spent_funding() {
    let mut anchoring_testkit = AnchoringTestKit::default();
    let anchoring_interval = anchoring_testkit
        .actual_anchoring_config()
        .anchoring_interval;

    assert!(anchoring_testkit.last_anchoring_tx().is_none());
    // Establish anchoring transactions chain with the initial funding transaction.
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit
            .create_signature_txs()
            .into_iter()
            .flatten(),
    );

    // Add spent funding transaction.
    let spent_funding_transaction = anchoring_testkit
        .actual_anchoring_config()
        .funding_transaction
        .unwrap();
    anchoring_testkit.inner.create_block_with_transactions(
        anchoring_testkit.create_funding_confirmation_txs_with(spent_funding_transaction),
    );

    // Reach the next anchoring height.
    anchoring_testkit
        .inner
        .create_blocks_until(Height(anchoring_interval));

    // Ensure that the anchoring proposal has no input with the spent funding transaction.
    assert_eq!(
        anchoring_testkit
            .anchoring_transaction_proposal()
            .unwrap()
            .1
            .len(),
        1
    );
}

#[test]
fn insufficient_funds() {
    let mut anchoring_testkit = AnchoringTestKit::new(4, 10, 5);

    {
        let snapshot = anchoring_testkit.inner.snapshot();
        let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
        let proposal = schema.actual_proposed_anchoring_transaction().unwrap();
        assert_eq!(
            proposal,
            Err(BuilderError::InsufficientFunds {
                total_fee: 1530,
                balance: 10
            })
        );
    }

    // Replenish the anchoring wallet by the given amount of satoshis.
    anchoring_testkit
        .inner
        .create_block_with_transactions(anchoring_testkit.create_funding_confirmation_txs(2000).0);
    anchoring_testkit
        .anchoring_transaction_proposal()
        .expect("Anchoring proposal should be correct.");
}

#[test]
fn no_anchoring_proposal() {
    let mut anchoring_testkit = AnchoringTestKit::default();
    let anchoring_interval = anchoring_testkit
        .actual_anchoring_config()
        .anchoring_interval;

    assert!(anchoring_testkit.last_anchoring_tx().is_none());

    let mut signatures = anchoring_testkit.create_signature_txs();
    let leftover_signatures = signatures.pop().unwrap();

    anchoring_testkit
        .inner
        .create_block_with_transactions(signatures.into_iter().flatten());

    // Anchor a next height.
    anchoring_testkit
        .inner
        .create_blocks_until(Height(anchoring_interval));
    let signatures = anchoring_testkit
        .create_signature_txs()
        .into_iter()
        .flatten();
    anchoring_testkit
        .inner
        .create_block_with_transactions(signatures);

    // Very slow node.
    let block = anchoring_testkit
        .inner
        .create_block_with_transactions(leftover_signatures);
    assert_tx_error(&block[0], Error::AnchoringUnnecessary);
}

#[test]
fn unexpected_anchoring_proposal() {
    let mut anchoring_testkit = AnchoringTestKit::default();
    let anchoring_interval = anchoring_testkit
        .actual_anchoring_config()
        .anchoring_interval;

    assert!(anchoring_testkit.last_anchoring_tx().is_none());

    let mut signatures = anchoring_testkit.create_signature_txs();
    let leftover_signatures = signatures.pop().unwrap();

    anchoring_testkit
        .inner
        .create_block_with_transactions(signatures.into_iter().flatten());

    anchoring_testkit
        .inner
        .create_blocks_until(Height(anchoring_interval));

    // Anchor a next height.
    let signatures = anchoring_testkit
        .create_signature_txs()
        .into_iter()
        .flatten();
    anchoring_testkit
        .inner
        .create_block_with_transactions(signatures);
    // Wait until the next anchoring height becomes an actual.
    anchoring_testkit
        .inner
        .create_blocks_until(Height(anchoring_interval * 2));

    // Very slow node
    let block = anchoring_testkit
        .inner
        .create_block_with_transactions(leftover_signatures);
    assert_tx_error(&block[0], Error::UnexpectedAnchoringProposal);
}

#[test]
fn add_anchoring_node() {
    test_anchoring_config_change(|anchoring_testkit, cfg| {
        cfg.anchoring_keys.push(anchoring_testkit.add_node());
        cfg.funding_transaction = None;
    });
}

#[test]
fn remove_anchoring_node() {
    test_anchoring_config_change(|_, cfg| {
        cfg.anchoring_keys.pop();
        cfg.funding_transaction = None;
    });
}

#[test]
fn change_anchoring_node_without_funds() {
    test_anchoring_config_change(|anchoring_testkit, cfg| {
        cfg.anchoring_keys[0].bitcoin_key = anchoring_testkit.gen_bitcoin_key();
        cfg.funding_transaction = None;
    });
}

#[test]
fn change_anchoring_node_with_funds() {
    test_anchoring_config_change(|anchoring_testkit, cfg| {
        cfg.anchoring_keys[0].bitcoin_key = anchoring_testkit.gen_bitcoin_key();
        cfg.funding_transaction = Some(create_fake_funding_transaction(
            &cfg.anchoring_address(),
            150_000,
        ));
    });
}

#[test]
fn add_anchoring_node_insufficient_funds() {
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

    // Remove one of anchoring nodes.
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

    // Ensure that the anchoring transaction proposal is unsuitable.
    {
        let snapshot = anchoring_testkit.inner.snapshot();
        let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
        let proposal = schema.actual_proposed_anchoring_transaction().unwrap();
        assert_eq!(
            proposal,
            Err(BuilderError::InsufficientFunds {
                total_fee: 1530,
                balance: 470
            })
        );
    }

    // Add funds.
    let (txs, funding_tx) = anchoring_testkit.create_funding_confirmation_txs(2000);
    anchoring_testkit.inner.create_block_with_transactions(txs);
    // Ensure that we have a suitable transition anchoring transaction proposal.
    assert_eq!(
        anchoring_testkit
            .anchoring_transaction_proposal()
            .unwrap()
            .1[1],
        funding_tx
    );
}

// #[test]
// fn broken_anchoring_recovery() {
//     let validators_num = 5;

//     // single tx fee is ~ 15000
//     let initial_sum = 20000;
//     let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, initial_sum, 4);
//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(3)
//         .unwrap();
//     anchoring_testkit.create_block_with_transactions(signatures);
//     anchoring_testkit.create_blocks_until(Height(4));

//     let latest_successful_tx = anchoring_testkit.last_anchoring_tx().unwrap();

//     // removing one of validators
//     let mut proposal = anchoring_testkit.drop_validator_proposal();

//     proposal.set_actual_from(Height(16));
//     anchoring_testkit.commit_configuration_change(proposal);

//     anchoring_testkit.create_blocks_until(Height(7));
//     anchoring_testkit.renew_address();

//     anchoring_testkit.create_blocks_until(Height(20));

//     let same_tx = anchoring_testkit.last_anchoring_tx().unwrap();
//     // No new transactions

//     assert!(latest_successful_tx == same_tx);

//     // Creating new funding tx
//     let address = anchoring_testkit.anchoring_address();
//     let new_funding_tx = create_fake_funding_transaction(&address, initial_sum * 3);

//     let mut proposal = anchoring_testkit.configuration_change_proposal();
//     let service_configuration = GlobalConfig {
//         funding_transaction: Some(new_funding_tx),
//         ..proposal.service_config(BTC_ANCHORING_SERVICE_NAME)
//     };

//     proposal.set_service_config(BTC_ANCHORING_SERVICE_NAME, service_configuration);
//     proposal.set_actual_from(Height(24));

//     anchoring_testkit.commit_configuration_change(proposal);
//     anchoring_testkit.create_blocks_until(Height(26));

//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(3)
//         .unwrap();
//     anchoring_testkit.create_block_with_transactions(signatures);
//     anchoring_testkit.create_blocks_until(Height(28));

//     let recovery_tx = anchoring_testkit.last_anchoring_tx().unwrap();

//     // check if it's recovery tx
//     assert_eq!(
//         recovery_tx
//             .anchoring_payload()
//             .unwrap()
//             .prev_tx_chain
//             .unwrap(),
//         latest_successful_tx.id()
//     );

//     assert!(
//         recovery_tx.anchoring_payload().unwrap().block_height
//             > latest_successful_tx
//                 .anchoring_payload()
//                 .unwrap()
//                 .block_height
//     );

//     let signatures = anchoring_testkit
//         .create_signature_tx_for_validators(3)
//         .unwrap();
//     anchoring_testkit.create_block_with_transactions(signatures);
//     anchoring_testkit.create_blocks_until(Height(32));

//     let after_recovery_tx = anchoring_testkit.last_anchoring_tx().unwrap();
//     assert!(after_recovery_tx
//         .anchoring_payload()
//         .unwrap()
//         .prev_tx_chain
//         .is_none());
//     assert!(
//         after_recovery_tx.anchoring_payload().unwrap().block_height
//             > recovery_tx.anchoring_payload().unwrap().block_height
//     );
// }
