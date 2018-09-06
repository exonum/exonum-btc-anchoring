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

extern crate bitcoin;
extern crate btc_transaction_utils;
extern crate exonum;
extern crate exonum_bitcoinrpc as bitcoin_rpc;
extern crate exonum_btc_anchoring;
extern crate exonum_testkit;
extern crate serde_json;

use exonum::helpers::Height;
use exonum_btc_anchoring::{
    api::{FindTransactionQuery, HeightQuery, PublicApi},
    blockchain::BtcAnchoringSchema,
    btc,
    config::GlobalConfig,
    test_helpers::testkit::{AnchoringTestKit, ValidateProof},
    BTC_ANCHORING_SERVICE_NAME,
};
use exonum_testkit::TestKitApi;

const NULL_QUERY: () = ();

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

    let tx0 = anchoring_testkit.last_anchoring_tx().unwrap();

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
fn find_transaction() {
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

    let api = anchoring_testkit.api();
    let find_transaction = |height: Option<Height>| -> btc::Transaction {
        api.find_transaction(FindTransactionQuery { height })
            .unwrap()
            .unwrap()
            .validate(&anchoring_testkit.actual_configuration())
            .unwrap()
            .1
    };

    let snapshot = anchoring_testkit.snapshot();
    let anchoring_schema = BtcAnchoringSchema::new(snapshot);
    let tx_chain = anchoring_schema.anchoring_transactions_chain();

    assert_eq!(find_transaction(Some(Height(0))), tx_chain.get(0).unwrap());
    // assert_eq!(find_transaction(None), tx_chain.last().unwrap());
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
    let second_block_proof = api.block_header_proof(HeightQuery { height: 10 }).unwrap();
    let value = second_block_proof.validate(&cfg).unwrap();
    assert_eq!(value.0, 10);
    assert_eq!(value.1, anchoring_testkit.block_hash_on_height(Height(10)));
}
