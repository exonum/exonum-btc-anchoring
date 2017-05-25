extern crate exonum;
extern crate sandbox;
extern crate anchoring_btc_service;
#[macro_use]
extern crate anchoring_btc_sandbox;
extern crate serde;
extern crate serde_json;
extern crate bitcoin;
extern crate bitcoinrpc;
extern crate secp256k1;

use exonum::crypto::HexValue;
use exonum::messages::Message;

use anchoring_btc_service::details::sandbox::Request;
use anchoring_btc_service::blockchain::dto::MsgAnchoringUpdateLatest;
use anchoring_btc_service::AnchoringConfig;
use anchoring_btc_service::error::HandlerError;
use anchoring_btc_service::details::btc::transactions::BitcoinTx;
use anchoring_btc_sandbox::AnchoringSandbox;
use anchoring_btc_sandbox::helpers::*;

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
    let lect = MsgAnchoringUpdateLatest::new(&sandbox.p(0),
                                             0,
                                             lect_tx,
                                             lects_count(&sandbox, 0),
                                             sandbox.s(0));
    sandbox.add_height_as_auditor(&[lect.raw().clone()]);

    assert_eq!(sandbox.take_errors()[0],
               HandlerError::LectNotFound { height: next_anchoring_height });
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
        .map(|id| {
                 MsgAnchoringUpdateLatest::new(&sandbox.p(id as usize),
                                               id,
                                               lect_tx.clone(),
                                               lects_count(&sandbox, id),
                                               sandbox.s(id as usize))
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

    assert_eq!(sandbox.take_errors()[0],
               HandlerError::IncorrectLect {
                   reason: String::from("Initial funding_tx not found in the bitcoin blockchain"),
                   tx: lect_tx.into(),
               });
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

    let lect_tx = BitcoinTx::from_hex("020000000152f2e44424d6cc16ce29566b54468084d1d15329b28e\
                                       8fc7cb9d9d783b8a76d3010000006b4830450221009e5ae44ba558\
                                       6e4aadb9e1bc5369cc9fe9f16c12ff94454ac90414f1c5a3df9002\
                                       20794b24afab7501ba12ea504853a31359d718c2a7ff6dd2688e95\
                                       c5bc6634ce39012102f81d4470a303a508bf03de893223c89360a5\
                                       d093e3095560b71de245aaf45d57feffffff028096980000000000\
                                       17a914dcfbafb4c432a24dd4b268570d26d7841a20fbbd87e7cc39\
                                       0a000000001976a914b3203ee5a42f8f524d14397ef10b84277f78\
                                       4b4a88acd81d1100")
            .unwrap();
    let lects = (0..3)
        .map(|id| {
                 MsgAnchoringUpdateLatest::new(&sandbox.p(id as usize),
                                               id,
                                               lect_tx.clone(),
                                               lects_count(&sandbox, id),
                                               sandbox.s(id as usize))
             })
        .collect::<Vec<_>>();
    force_commit_lects(&sandbox, lects);

    sandbox.add_height_as_auditor(&[]);

    assert_eq!(sandbox.take_errors()[0],
               HandlerError::IncorrectLect {
                   reason: String::from("Initial funding_tx from cfg is different than in lect"),
                   tx: lect_tx.into(),
               });
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

    assert_eq!(sandbox.take_errors()[0],
               HandlerError::IncorrectLect {
                   reason: String::from("Lect not found in the bitcoin blockchain"),
                   tx: lect_tx.into(),
               });
}
