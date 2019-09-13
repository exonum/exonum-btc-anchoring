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

use exonum::blockchain::TransactionErrorType;
use exonum::explorer::BlockWithTransactions;
use exonum::helpers::Height;
use exonum_btc_anchoring::{
    blockchain::errors::ErrorCode,
    btc::BuilderError,
    config::GlobalConfig,
    test_helpers::testkit::{create_fake_funding_transaction, AnchoringTestKit},
    BTC_ANCHORING_SERVICE_NAME,
};

use matches::assert_matches;

fn assert_tx_error(block: BlockWithTransactions, e: ErrorCode) {
    assert_eq!(
        block[0].status().unwrap_err().error_type(),
        TransactionErrorType::Code(e as u8),
    );
}

#[test]
fn simple() {
    let validators_num = 4;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, 70000, 4);

    assert!(anchoring_testkit.last_anchoring_tx().is_none());

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(2)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(4));

    let tx0 = anchoring_testkit.last_anchoring_tx().unwrap();
    let tx0_meta = tx0.anchoring_metadata().unwrap();
    assert!(tx0_meta.1.block_height == Height(0));

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(2)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(8));

    let tx1 = anchoring_testkit.last_anchoring_tx().unwrap();
    let tx1_meta = tx1.anchoring_metadata().unwrap();

    assert!(tx0.id() == tx1.prev_tx_id());

    // script_pubkey should be the same
    assert!(tx0_meta.0 == tx1_meta.0);
    assert!(tx1_meta.1.block_height == Height(4));
}

#[test]
fn additional_funding() {
    let validators_num = 4;
    let initial_sum = 50000;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, initial_sum, 4);

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(2)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(4));

    let tx0 = anchoring_testkit.last_anchoring_tx().unwrap();
    let output_val0 = tx0.unspent_value().unwrap();

    assert!(tx0.0.input.len() == 1);
    assert!(output_val0 < initial_sum);

    //creating new funding tx
    let address = anchoring_testkit.anchoring_address();
    let new_funding_tx = create_fake_funding_transaction(&address, initial_sum);

    let mut proposal = anchoring_testkit.configuration_change_proposal();
    let service_configuration = GlobalConfig {
        funding_transaction: Some(new_funding_tx),
        ..proposal.service_config(BTC_ANCHORING_SERVICE_NAME)
    };

    proposal.set_service_config(BTC_ANCHORING_SERVICE_NAME, service_configuration);
    proposal.set_actual_from(Height(6));
    anchoring_testkit.commit_configuration_change(proposal);
    anchoring_testkit.create_blocks_until(Height(6));

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(2)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);

    let tx1 = anchoring_testkit.last_anchoring_tx().unwrap();
    let output_val1 = tx1.unspent_value().unwrap();
    let tx1_meta = tx1.anchoring_metadata().unwrap();
    assert!(tx1_meta.1.block_height == Height(4));

    assert!(tx1.0.input.len() == 2);

    assert!(output_val1 > output_val0);
    assert!(output_val1 > initial_sum);
}

#[test]
fn address_changed() {
    let validators_num = 5;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, 150_000, 4);
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(4));

    let tx0 = anchoring_testkit.last_anchoring_tx().unwrap();
    let tx0_meta = tx0.anchoring_metadata().unwrap();

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(4)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(6));

    // removing one of validators
    let mut proposal = anchoring_testkit.drop_validator_proposal();
    proposal.set_actual_from(Height(16));
    anchoring_testkit.commit_configuration_change(proposal);
    anchoring_testkit.create_blocks_until(Height(7));
    anchoring_testkit.renew_address();

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(10));

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(12));

    let tx_transition = anchoring_testkit.last_anchoring_tx().unwrap();

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(16));

    let tx_same = anchoring_testkit.last_anchoring_tx().unwrap();
    // anchoring is paused till new config
    assert!(tx_transition == tx_same);

    anchoring_testkit.create_blocks_until(Height(17));
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();

    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(20));

    let tx_changed = anchoring_testkit.last_anchoring_tx().unwrap();
    let tx_changed_meta = tx_changed.anchoring_metadata().unwrap();

    assert!(tx_transition != tx_changed);
    // script_pubkey should *not* be the same
    assert!(tx0_meta.0 != tx_changed_meta.0);
}

#[test]
fn address_changed_and_new_funding_tx() {
    let validators_num = 5;
    let initial_sum = 150_000;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, initial_sum, 4);
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(4));

    let tx0 = anchoring_testkit.last_anchoring_tx().unwrap();
    let tx0_meta = tx0.anchoring_metadata().unwrap();
    let output_val0 = tx0.unspent_value().unwrap();

    // removing one of validators
    let mut proposal = anchoring_testkit.drop_validator_proposal();
    let mut service_config: GlobalConfig = proposal.service_config(BTC_ANCHORING_SERVICE_NAME);

    // additional funding
    let new_address = service_config.anchoring_address();
    let new_funding_tx = create_fake_funding_transaction(&new_address, initial_sum);

    service_config.funding_transaction = Some(new_funding_tx);
    proposal.set_service_config(BTC_ANCHORING_SERVICE_NAME, service_config);
    proposal.set_actual_from(Height(16));
    anchoring_testkit.commit_configuration_change(proposal);

    anchoring_testkit.create_blocks_until(Height(7));

    anchoring_testkit.renew_address();

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(10));

    let tx_transition = anchoring_testkit.last_anchoring_tx().unwrap();

    //new funding transaction should not be consumed during creation of transition tx
    assert!(tx_transition.0.input.len() == 1);

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(16));

    anchoring_testkit.create_blocks_until(Height(17));
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();

    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(20));

    let tx_changed = anchoring_testkit.last_anchoring_tx().unwrap();
    let tx_changed_meta = tx_changed.anchoring_metadata().unwrap();
    let output_changed = tx_changed.unspent_value().unwrap();

    assert!(tx_transition != tx_changed);
    assert!(tx_changed.0.input.len() == 2);

    // script_pubkey should *not* be the same
    assert!(tx0_meta.0 != tx_changed_meta.0);

    assert!(output_changed > output_val0);
    assert!(output_changed > initial_sum);
}

#[test]
fn insufficient_funds_during_address_change() {
    let validators_num = 5;
    // single tx fee is ~ 15000
    let initial_sum = 20000;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, initial_sum, 4);
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(4));

    let tx0 = anchoring_testkit.last_anchoring_tx();

    // removing one of validators
    let mut proposal = anchoring_testkit.drop_validator_proposal();
    proposal.set_actual_from(Height(16));
    anchoring_testkit.commit_configuration_change(proposal);
    anchoring_testkit.create_blocks_until(Height(7));

    anchoring_testkit.renew_address();
    anchoring_testkit.create_blocks_until(Height(20));

    let tx1 = anchoring_testkit.last_anchoring_tx();

    // no new transactions
    assert!(tx0 == tx1);

    assert_matches!(
        anchoring_testkit
            .create_signature_tx_for_validators(1)
            .unwrap_err(),
        BuilderError::UnsuitableOutput
    );
}

#[test]
fn signature_while_paused_in_transition() {
    let validators_num = 5;
    let initial_sum = 80000;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, initial_sum, 4);

    let mut signatures = anchoring_testkit
        .create_signature_tx_for_validators(4)
        .unwrap();
    let leftover_signature = signatures.remove(0);

    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(4));

    // to be sure if anchoring is started
    assert!(anchoring_testkit.last_anchoring_tx().is_some());

    // removing one of validators
    let mut proposal = anchoring_testkit.drop_validator_proposal();
    proposal.set_actual_from(Height(16));

    anchoring_testkit.commit_configuration_change(proposal);
    anchoring_testkit.create_blocks_until(Height(7));
    anchoring_testkit.renew_address();

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(12));

    let block = anchoring_testkit.create_block_with_transactions(vec![leftover_signature]);
    assert_tx_error(block, ErrorCode::InTransition);
}

#[test]
fn wrong_signature_tx() {
    let validators_num = 4;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, 70000, 4);

    assert!(anchoring_testkit.last_anchoring_tx().is_none());

    let mut signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    let leftover_signature = signatures.pop().unwrap();

    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(4));

    let tx0 = anchoring_testkit.last_anchoring_tx().unwrap();
    let tx0_meta = tx0.anchoring_metadata().unwrap();
    assert!(tx0_meta.1.block_height == Height(0));

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(2)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(8));

    // very slow node
    let block = anchoring_testkit.create_block_with_transactions(vec![leftover_signature]);
    assert_tx_error(block, ErrorCode::Unexpected);
}

#[test]
fn broken_anchoring_recovery() {
    let validators_num = 5;

    // single tx fee is ~ 15000
    let initial_sum = 20000;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, initial_sum, 4);
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(4));

    let latest_successful_tx = anchoring_testkit.last_anchoring_tx().unwrap();

    // removing one of validators
    let mut proposal = anchoring_testkit.drop_validator_proposal();

    proposal.set_actual_from(Height(16));
    anchoring_testkit.commit_configuration_change(proposal);

    anchoring_testkit.create_blocks_until(Height(7));
    anchoring_testkit.renew_address();

    anchoring_testkit.create_blocks_until(Height(20));

    let same_tx = anchoring_testkit.last_anchoring_tx().unwrap();
    // No new transactions

    assert!(latest_successful_tx == same_tx);

    // Creating new funding tx
    let address = anchoring_testkit.anchoring_address();
    let new_funding_tx = create_fake_funding_transaction(&address, initial_sum * 3);

    let mut proposal = anchoring_testkit.configuration_change_proposal();
    let service_configuration = GlobalConfig {
        funding_transaction: Some(new_funding_tx),
        ..proposal.service_config(BTC_ANCHORING_SERVICE_NAME)
    };

    proposal.set_service_config(BTC_ANCHORING_SERVICE_NAME, service_configuration);
    proposal.set_actual_from(Height(24));

    anchoring_testkit.commit_configuration_change(proposal);
    anchoring_testkit.create_blocks_until(Height(26));

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(28));

    let recovery_tx = anchoring_testkit.last_anchoring_tx().unwrap();

    // check if it's recovery tx
    assert_eq!(
        recovery_tx
            .anchoring_payload()
            .unwrap()
            .prev_tx_chain
            .unwrap(),
        latest_successful_tx.id()
    );

    assert!(
        recovery_tx.anchoring_payload().unwrap().block_height
            > latest_successful_tx
                .anchoring_payload()
                .unwrap()
                .block_height
    );

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(32));

    let after_recovery_tx = anchoring_testkit.last_anchoring_tx().unwrap();
    assert!(after_recovery_tx
        .anchoring_payload()
        .unwrap()
        .prev_tx_chain
        .is_none());
    assert!(
        after_recovery_tx.anchoring_payload().unwrap().block_height
            > recovery_tx.anchoring_payload().unwrap().block_height
    );
}
