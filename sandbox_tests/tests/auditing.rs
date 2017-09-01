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

extern crate exonum;
extern crate sandbox;
extern crate exonum_btc_anchoring;
#[macro_use]
extern crate exonum_btc_anchoring_sandbox;
#[macro_use]
extern crate serde_json;
extern crate bitcoin;

use bitcoin::util::base58::ToBase58;

use exonum::crypto::HexValue;
use exonum::messages::{Message, RawTransaction};
use exonum::storage::StorageValue;
use exonum::helpers::{Height, ValidatorId};
use sandbox::config_updater::TxConfig;

use exonum_btc_anchoring::{ANCHORING_SERVICE_NAME, AnchoringConfig};
use exonum_btc_anchoring::details::sandbox::Request;
use exonum_btc_anchoring::blockchain::dto::MsgAnchoringUpdateLatest;
use exonum_btc_anchoring::error::HandlerError;
use exonum_btc_anchoring::details::btc::transactions::BitcoinTx;
use exonum_btc_anchoring_sandbox::{AnchoringSandbox, ANCHORING_VALIDATOR};
use exonum_btc_anchoring_sandbox::helpers::*;

/// Generates a configuration that excludes `sandbox node` from consensus.
/// Then it continues to work as auditor.
fn gen_following_cfg(
    sandbox: &AnchoringSandbox,
    from_height: Height,
) -> (RawTransaction, AnchoringConfig) {
    let anchoring_addr = sandbox.current_addr();

    let mut service_cfg = sandbox.current_cfg().clone();
    let priv_keys = sandbox.priv_keys(&anchoring_addr);
    service_cfg.anchoring_keys.swap_remove(0);

    let following_addr = service_cfg.redeem_script().1;
    for (id, ref mut node) in sandbox.nodes_mut().iter_mut().enumerate() {
        node.private_keys.insert(
            following_addr.to_base58check(),
            priv_keys[id].clone(),
        );
    }

    let mut cfg = sandbox.cfg();
    cfg.actual_from = from_height;
    cfg.previous_cfg_hash = sandbox.cfg().hash();
    cfg.validator_keys.swap_remove(0);
    *cfg.services.get_mut(ANCHORING_SERVICE_NAME).unwrap() = json!(service_cfg);
    let tx = TxConfig::new(
        &sandbox.service_public_key(ANCHORING_VALIDATOR),
        &cfg.into_bytes(),
        from_height,
        sandbox.service_secret_key(ANCHORING_VALIDATOR),
    );
    (tx.raw().clone(), service_cfg)
}

// Invoke this method after anchor_first_block_lect_normal
pub fn exclude_node_from_validators(sandbox: &AnchoringSandbox) {
    let cfg_change_height = Height(12);
    let (cfg_tx, following_cfg) = gen_following_cfg(sandbox, cfg_change_height);
    let (_, following_addr) = following_cfg.redeem_script();

    // Tx has not enough confirmations.
    let anchored_tx = sandbox.latest_anchored_tx();

    let client = sandbox.client();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        Height::zero(),
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = sandbox.latest_anchored_tx();
    // Tx gets enough confirmations.
    client.expect(vec![confirmations_request(&anchored_tx, 100)]);
    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 100)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(sandbox, ValidatorId(id), &transition_tx, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
    client.expect(vec![confirmations_request(&transition_tx, 100)]);
    sandbox.add_height(&lects);
    sandbox.fast_forward_to_height(cfg_change_height);

    sandbox.set_anchoring_cfg(following_cfg);
    client.expect(vec![get_transaction_request(&transition_tx)]);
    sandbox.add_height_as_auditor(&[]);

    assert_eq!(sandbox.handler().errors, Vec::new());
}

// We exclude sandbox node from validators
// problems: None
// result: success
#[test]
fn test_auditing_exclude_node_from_validators() {
    let _ = exonum::helpers::init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);
    exclude_node_from_validators(&sandbox);
}

// There is no consensus in `exonum` about current `lect`.
// result: Error LectNotFound occured
#[test]
fn test_auditing_no_consensus_in_lect() {
    let _ = exonum::helpers::init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);
    let next_anchoring_height = sandbox.next_anchoring_height();
    exclude_node_from_validators(&sandbox);
    sandbox.fast_forward_to_height_as_auditor(sandbox.next_check_lect_height());

    let lect_tx = BitcoinTx::from(sandbox.current_funding_tx().0);
    let lect = MsgAnchoringUpdateLatest::new(
        &sandbox.service_public_key(ANCHORING_VALIDATOR),
        ANCHORING_VALIDATOR,
        lect_tx,
        lects_count(&sandbox, ANCHORING_VALIDATOR),
        sandbox.service_secret_key(ANCHORING_VALIDATOR),
    );
    sandbox.add_height_as_auditor(&[lect.raw().clone()]);

    assert_eq!(
        sandbox.take_errors()[0],
        HandlerError::LectNotFound { height: next_anchoring_height }
    );
}

// FundingTx from lect not found in `bitcoin` network
// result: Error IncorrectLect occured
#[test]
fn test_auditing_lect_lost_funding_tx() {
    let _ = exonum::helpers::init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);
    exclude_node_from_validators(&sandbox);
    sandbox.fast_forward_to_height_as_auditor(sandbox.next_check_lect_height());

    let lect_tx = BitcoinTx::from(sandbox.current_funding_tx().0);
    let lects = (0..3)
        .map(ValidatorId)
        .map(|validator_id| {
            MsgAnchoringUpdateLatest::new(
                &sandbox.service_public_key(validator_id),
                validator_id,
                lect_tx.clone(),
                lects_count(&sandbox, validator_id),
                sandbox.service_secret_key(validator_id),
            )
        })
        .collect::<Vec<_>>();
    force_commit_lects(&sandbox, lects);

    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&lect_tx.txid(), 0],
            error: RpcError::NoInformation("Unable to find tx".to_string())
        },
    ]);
    sandbox.add_height_as_auditor(&[]);

    assert_eq!(
        sandbox.take_errors()[0],
        HandlerError::IncorrectLect {
            reason: String::from("Initial funding_tx not found in the bitcoin blockchain"),
            tx: lect_tx.into(),
        }
    );
}

// FundingTx from lect has no correct outputs
// result: Error IncorrectLect occured
#[test]
fn test_auditing_lect_incorrect_funding_tx() {
    let _ = exonum::helpers::init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);
    exclude_node_from_validators(&sandbox);
    sandbox.fast_forward_to_height_as_auditor(sandbox.next_check_lect_height());

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
            MsgAnchoringUpdateLatest::new(
                &sandbox.service_public_key(id),
                id,
                lect_tx.clone(),
                lects_count(&sandbox, id),
                sandbox.service_secret_key(id),
            )
        })
        .collect::<Vec<_>>();
    force_commit_lects(&sandbox, lects);

    sandbox.add_height_as_auditor(&[]);

    assert_eq!(
        sandbox.take_errors()[0],
        HandlerError::IncorrectLect {
            reason: String::from("Initial funding_tx from cfg is different than in lect"),
            tx: lect_tx.into(),
        }
    );
}

// Current lect not found in `bitcoin` network
// result: Error IncorrectLect occured
#[test]
fn test_auditing_lect_lost_current_lect() {
    let _ = exonum::helpers::init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);
    exclude_node_from_validators(&sandbox);
    sandbox.fast_forward_to_height_as_auditor(sandbox.next_check_lect_height());

    let lect_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: [&lect_tx.txid(), 0],
            error: RpcError::NoInformation("Unable to find tx".to_string())
        },
    ]);
    sandbox.add_height_as_auditor(&[]);

    assert_eq!(
        sandbox.take_errors()[0],
        HandlerError::IncorrectLect {
            reason: String::from("Lect not found in the bitcoin blockchain"),
            tx: lect_tx.into(),
        }
    );
}
