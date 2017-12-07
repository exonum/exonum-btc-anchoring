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
extern crate exonum_bitcoinrpc as bitcoinrpc;
extern crate secp256k1;
extern crate rand;
extern crate serde;
extern crate libc;
extern crate byteorder;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate pretty_assertions;
extern crate exonum;
extern crate exonum_btc_anchoring;
#[macro_use]
extern crate exonum_testkit;

#[macro_use]
pub mod testkit_extras;

use exonum::helpers::{Height, ValidatorId};
use exonum::blockchain::Transaction;
use exonum::encoding::serialize::FromHex;
use exonum_testkit::TestNetworkConfiguration;

use exonum_btc_anchoring::{AnchoringConfig, ANCHORING_SERVICE_NAME};
use exonum_btc_anchoring::handler::error::Error as HandlerError;
use exonum_btc_anchoring::blockchain::dto::MsgAnchoringUpdateLatest;
use exonum_btc_anchoring::details::btc::transactions::BitcoinTx;
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
        node.private_keys.insert(
            following_addr.to_string(),
            priv_keys[id].clone(),
        );
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
    assert!(testkit.mempool().contains_key(&signatures[0].hash()));

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(testkit, ValidatorId(id), &transition_tx, 2)
        })
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.mempool().contains_key(&lects[0].hash()));
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
// result: Error LectNotFound occured
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
        HandlerError::LectNotFound { height: next_anchoring_height.next() }
    );
}

// FundingTx from lect not found in `bitcoin` network
// result: Error IncorrectLect occured
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
            params: [&lect_tx.txid(), 0],
            error: RpcError::NoInformation("Unable to find tx".to_string())
        },
    ]);
    testkit.create_block();
}

// FundingTx from lect has no correct outputs
// result: Error IncorrectLect occured
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
        "020000000152f2e44424d6cc16ce29566b54468084d1d15329b28e\
         8fc7cb9d9d783b8a76d3010000006b4830450221009e5ae44ba558\
         6e4aadb9e1bc5369cc9fe9f16c12ff94454ac90414f1c5a3df9002\
         20794b24afab7501ba12ea504853a31359d718c2a7ff6dd2688e95\
         c5bc6634ce39012102f81d4470a303a508bf03de893223c89360a5\
         d093e3095560b71de245aaf45d57feffffff028096980000000000\
         17a914dcfbafb4c432a24dd4b268570d26d7841a20fbbd87e7cc39\
         0a000000001976a914b3203ee5a42f8f524d14397ef10b84277f78\
         4b4a88acd81d1100",
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
// result: Error LectNotFound occured
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
            params: [&lect_tx.txid(), 0],
            error: RpcError::NoInformation("Unable to find tx".to_string())
        },
    ]);
    testkit.create_block();

    assert_eq!(
        testkit.take_handler_errors()[0],
        HandlerError::LectNotFound { height: Height(0) }
    );
}
