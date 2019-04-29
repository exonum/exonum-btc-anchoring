// Copyright 2018 The Exonum Team
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

use exonum::helpers::Height;
use exonum_btc_anchoring::{
    api::{FindTransactionQuery, HeightQuery, PublicApi},
    btc,
    config::GlobalConfig,
    test_helpers::testkit::{AnchoringTestKit, ValidateProof},
    BTC_ANCHORING_SERVICE_NAME,
};

const NULL_QUERY: () = ();

fn find_transaction(
    anchoring_testkit: &AnchoringTestKit,
    height: Option<Height>,
) -> Option<btc::Transaction> {
    let api = anchoring_testkit.api();
    api.find_transaction(FindTransactionQuery { height })
        .unwrap()
        .map(|proof| {
            proof
                .validate(&anchoring_testkit.actual_configuration())
                .unwrap()
                .1
        })
}

#[test]
fn actual_address() {
    let validators_num = 4;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, 70000, 4);

    assert!(anchoring_testkit.last_anchoring_tx().is_none());

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(2)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(4));

    let anchoring_api = anchoring_testkit.api();
    assert_eq!(
        anchoring_api.actual_address(NULL_QUERY).unwrap(),
        anchoring_testkit.anchoring_address()
    );
}

#[test]
fn following_address() {
    let validators_num = 5;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, 150000, 4);
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(4));
    // Following address should be none for regular anchoring.
    assert_eq!(
        anchoring_testkit
            .api()
            .following_address(NULL_QUERY)
            .unwrap(),
        None
    );

    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(4)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_blocks_until(Height(6));

    // Removing one of validators
    let mut proposal = anchoring_testkit.drop_validator_proposal();
    let service_config: GlobalConfig = proposal.service_config(BTC_ANCHORING_SERVICE_NAME);
    // Following address
    let following_address = service_config.anchoring_address();
    proposal.set_actual_from(Height(16));
    anchoring_testkit.commit_configuration_change(proposal);
    anchoring_testkit.create_block();

    assert_eq!(
        anchoring_testkit
            .api()
            .following_address(NULL_QUERY)
            .unwrap(),
        Some(following_address)
    );
}

#[test]
fn find_transaction_regular() {
    let validators_num = 4;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, 70000, 4);

    assert!(anchoring_testkit.last_anchoring_tx().is_none());

    // Creates a few anchoring transactions
    for _ in 0..5 {
        let signatures = anchoring_testkit
            .create_signature_tx_for_validators(2)
            .unwrap();
        anchoring_testkit.create_block_with_transactions(signatures);

        let next_anchoring_height = anchoring_testkit
            .actual_anchoring_configuration()
            .following_anchoring_height(anchoring_testkit.height());
        anchoring_testkit.create_blocks_until(next_anchoring_height);
    }

    let anchoring_schema = anchoring_testkit.schema();
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

// Checks come corner cases in the find_transaction api method.
#[test]
fn find_transaction_configuration_change() {
    let validators_num = 5;
    let anchoring_frequency = 10;
    let mut anchoring_testkit =
        AnchoringTestKit::new_without_rpc(validators_num, 150000, anchoring_frequency);
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(4)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);

    // removing one of validators
    let mut proposal = anchoring_testkit.drop_validator_proposal();
    proposal.set_actual_from(Height(20));
    anchoring_testkit.commit_configuration_change(proposal);
    anchoring_testkit.create_block();
    anchoring_testkit.renew_address();
    // Creates transition transaction
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_block();

    let anchoring_schema = anchoring_testkit.schema();
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

    anchoring_testkit.create_blocks_until(Height(20));
    // Resumes regular anchoring (anchors block on height 10).
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_block();

    let anchoring_schema = anchoring_testkit.schema();
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

    // Anchors block on height 20.
    let signatures = anchoring_testkit
        .create_signature_tx_for_validators(3)
        .unwrap();
    anchoring_testkit.create_block_with_transactions(signatures);
    anchoring_testkit.create_block();

    let anchoring_schema = anchoring_testkit.schema();
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
        find_transaction(&anchoring_testkit, Some(Height(11))),
        anchoring_schema.anchoring_transactions_chain().get(3)
    );
    assert_eq!(
        find_transaction(&anchoring_testkit, None),
        anchoring_schema.anchoring_transactions_chain().get(3)
    );
}

// Tries to get a proof of existence for an anchored block.
#[test]
fn block_header_proof() {
    let validators_num = 4;
    let mut anchoring_testkit = AnchoringTestKit::new_without_rpc(validators_num, 70000, 4);
    // Creates a few anchoring transactions
    for _ in 0..5 {
        let signatures = anchoring_testkit
            .create_signature_tx_for_validators(2)
            .unwrap();
        anchoring_testkit.create_block_with_transactions(signatures);

        let next_anchoring_height = anchoring_testkit
            .actual_anchoring_configuration()
            .following_anchoring_height(anchoring_testkit.height());
        anchoring_testkit.create_blocks_until(next_anchoring_height);
    }

    let api = anchoring_testkit.api();
    let cfg = anchoring_testkit.actual_configuration();
    // Checks proof for the genesis block.
    let genesis_block_proof = api.block_header_proof(HeightQuery { height: 0 }).unwrap();
    let value = genesis_block_proof.validate(&cfg).unwrap();
    assert_eq!(value.0, 0);
    assert_eq!(value.1, anchoring_testkit.block_hash_on_height(Height(0)));
    // Checks proof for the second block.
    let second_block_proof = api.block_header_proof(HeightQuery { height: 4 }).unwrap();
    let value = second_block_proof.validate(&cfg).unwrap();
    assert_eq!(value.0, 4);
    assert_eq!(value.1, anchoring_testkit.block_hash_on_height(Height(4)));
}
