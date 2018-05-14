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

extern crate bitcoin;
extern crate btc_transaction_utils;
extern crate byteorder;
extern crate exonum;
extern crate exonum_bitcoinrpc as bitcoinrpc;
extern crate exonum_btc_anchoring;
#[macro_use]
extern crate exonum_testkit;
extern crate libc;
#[macro_use]
extern crate log;
#[macro_use]
extern crate pretty_assertions;
extern crate rand;
extern crate secp256k1;
extern crate serde;
#[macro_use]
extern crate serde_json;

#[macro_use]
pub mod testkit_extras;

use exonum::blockchain::Transaction;
use exonum::encoding::serialize::FromHex;
use exonum::helpers::{Height, ValidatorId};
use exonum_testkit::TestNetworkConfiguration;

use exonum_btc_anchoring::blockchain::dto::MsgAnchoringUpdateLatest;
use exonum_btc_anchoring::details::btc::transactions::BitcoinTx;
use exonum_btc_anchoring::handler::error::Error as HandlerError;
use exonum_btc_anchoring::{AnchoringConfig, ANCHORING_SERVICE_NAME};
use testkit_extras::AnchoringTestKit;
use testkit_extras::helpers::*;

/// Generates a configuration that excludes `testkit node` from consensus.
/// Then it continues to work as auditor.
fn gen_following_cfg(
    testkit: &mut AnchoringTestKit,
    from_height: Height,
) -> (TestNetworkConfiguration, AnchoringConfig) {
    let anchoring_addr = testkit.current_addr();

    let mut cfg = testkit.configuration_change_proposal();

    let mut service_cfg: AnchoringConfig = cfg.service_config(ANCHORING_SERVICE_NAME);
    let priv_keys = testkit.priv_keys(&anchoring_addr);
    service_cfg.anchoring_keys.swap_remove(0);

    let following_addr = service_cfg.redeem_script().1;
    for (id, ref mut node) in testkit.nodes_mut().iter_mut().enumerate() {
        node.private_keys
            .insert(following_addr.to_string(), priv_keys[id].clone());
    }

    cfg.set_actual_from(from_height);
    let mut validators = cfg.validators().to_vec();
    validators.swap_remove(0);
    cfg.set_validators(validators);
    cfg.set_service_config(ANCHORING_SERVICE_NAME, service_cfg.clone());
    (cfg, service_cfg)
}

// Invoke this method after anchor_first_block_lect_normal
pub fn exclude_node_from_validators(testkit: &mut AnchoringTestKit) {
    let cfg_change_height = Height(12);
    let (cfg_proposal, following_cfg) = gen_following_cfg(testkit, cfg_change_height);
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
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(|id| gen_service_tx_lect(testkit, ValidatorId(id), &transition_tx, 2))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));
    requests.expect(vec![confirmations_request(&transition_tx, 100)]);
    testkit.create_block_with_transactions(lects);
    testkit.create_blocks_until(cfg_change_height.previous());

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block();

    assert_eq!(testkit.take_handler_errors(), Vec::new());
}

// We exclude testkit node from validators
// problems: None
// result: success
#[test]
fn test_auditing_exclude_node_from_validators() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);
    exclude_node_from_validators(&mut testkit);
}

// There is no consensus in `exonum` about current `lect`.
// result: Error LectNotFound occurred
#[test]
fn test_auditing_no_consensus_in_lect() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);
    let next_anchoring_height = testkit.next_anchoring_height();
    exclude_node_from_validators(&mut testkit);
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);

    let lect_tx = BitcoinTx::from(testkit.current_funding_tx().0);
    let lect = {
        let validator_0 = ValidatorId(0);
        let keypair = testkit.validator(validator_0).service_keypair();
        MsgAnchoringUpdateLatest::new(
            keypair.0,
            validator_0,
            lect_tx,
            lects_count(&testkit, validator_0),
            keypair.1,
        )
    };
    testkit.create_block_with_transactions(txvec![lect.clone()]);

    assert_eq!(
        testkit.take_handler_errors()[0],
        HandlerError::LectNotFound {
            height: next_anchoring_height.next(),
        }
    );
}

// FundingTx from lect not found in `bitcoin` network
// result: Error IncorrectLect occurred
#[test]
#[should_panic(expected = "Initial funding_tx not found in the bitcoin blockchain")]
fn test_auditing_lect_lost_funding_tx() {
    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);
    exclude_node_from_validators(&mut testkit);
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);

    let lect_tx = BitcoinTx::from(testkit.current_funding_tx().0);
    let lects = (0..3)
        .map(ValidatorId)
        .map(|validator_id| {
            let keypair = testkit.validator(validator_id).service_keypair();
            MsgAnchoringUpdateLatest::new(
                keypair.0,
                validator_id,
                lect_tx.clone(),
                lects_count(&testkit, validator_id),
                keypair.1,
            )
        })
        .collect::<Vec<_>>();
    force_commit_lects(&mut testkit, lects);

    requests.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&lect_tx.id(), 0],
            error: RpcError::NoInformation("Unable to find tx".to_string())
        },
    ]);
    testkit.create_block();
}

// FundingTx from lect has no correct outputs
// result: Error IncorrectLect occurred
#[test]
#[should_panic(expected = "Initial funding_tx from cfg is different than in lect")]
fn test_auditing_lect_incorrect_funding_tx() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);
    exclude_node_from_validators(&mut testkit);
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);

    let lect_tx = BitcoinTx::from_hex(
        "02000000000101beaf6f1d54b9de13e0643bda743842b250a636a840a9d66ad5140ba84ba3c98f0000000000f\
         effffff02f460700a00000000160014cd56612aab60cf8b11163ab409c6bbe9a35586b2a00f00000000000022\
         00209d37bce25695790d72e3a2f15a46c5d5c62c3871f95a74db1e4a0e277074792a02483045022100d5dfaa4\
         0ee361ce58025abb3a9dca50ef92777600dd119d722bebb5bf90b79690220049f77648ab668859f50c3d2286f\
         1aacd0b98eebc69ac82649536b89eb6d8265012102ba681aae633b5c58c9bf6a8017a1b7cf8d0cb176017304f\
         f27d49dc8ff309fc236bd1300",
    ).unwrap();

    let lects = (0..3)
        .map(ValidatorId)
        .map(|id| {
            let keypair = testkit.validator(id).service_keypair();
            MsgAnchoringUpdateLatest::new(
                keypair.0,
                id,
                lect_tx.clone(),
                lects_count(&testkit, id),
                keypair.1,
            )
        })
        .collect::<Vec<_>>();
    force_commit_lects(&mut testkit, lects);

    testkit.create_block();
}

// Current lect not found in `bitcoin` network
// result: Error LectNotFound occurred
#[test]
fn test_auditing_lect_lost_current_lect() {
    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);
    exclude_node_from_validators(&mut testkit);
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);

    let lect_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&lect_tx.id(), 0],
            error: RpcError::NoInformation("Unable to find tx".to_string())
        },
    ]);
    testkit.create_block();

    assert_eq!(
        testkit.take_handler_errors()[0],
        HandlerError::LectNotFound { height: Height(0) }
    );
}
