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

use bitcoin::network::constants::Network;
use rand::{SeedableRng, StdRng};

use exonum::blockchain::Transaction;
use exonum::crypto::{gen_keypair_from_seed, CryptoHash, Seed};
use exonum::encoding::serialize::FromHex;
use exonum::helpers::{Height, ValidatorId};
use exonum_testkit::{TestNetworkConfiguration, TestNode};

use exonum_btc_anchoring::blockchain::AnchoringSchema;
use exonum_btc_anchoring::details::btc;
use exonum_btc_anchoring::details::btc::transactions::{FundingTx, TransactionBuilder};
use exonum_btc_anchoring::observer::AnchoringChainObserver;
use exonum_btc_anchoring::{AnchoringConfig, AnchoringNodeConfig, ANCHORING_SERVICE_NAME};
use testkit_extras::helpers::*;
use testkit_extras::{AnchoringTestKit, TestClient};

fn gen_following_cfg(
    testkit: &mut AnchoringTestKit,
    from_height: Height,
    funds: Option<FundingTx>,
) -> (TestNetworkConfiguration, AnchoringConfig) {
    let anchoring_addr = testkit.current_addr();

    // Create new keypair for testkit node
    let keypair = {
        let mut rng: StdRng = SeedableRng::from_seed([18, 252, 3, 117].as_ref());
        btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng)
    };

    let mut cfg_proposal = testkit.configuration_change_proposal();
    cfg_proposal.set_actual_from(from_height);
    let mut anchoring_cfg: AnchoringConfig = cfg_proposal.service_config(ANCHORING_SERVICE_NAME);
    let mut priv_keys = testkit.priv_keys(&anchoring_addr);
    anchoring_cfg.anchoring_keys[0] = keypair.0;
    priv_keys[0] = keypair.1;
    if let Some(funds) = funds {
        anchoring_cfg.funding_tx = Some(funds);
    }

    let following_addr = anchoring_cfg.redeem_script().1;
    for (id, ref mut node) in testkit.nodes_mut().iter_mut().enumerate() {
        node.private_keys
            .insert(following_addr.to_string(), priv_keys[id].clone());
    }
    cfg_proposal.set_service_config(ANCHORING_SERVICE_NAME, anchoring_cfg.clone());

    testkit
        .handler()
        .add_private_key(&following_addr, priv_keys[0].clone());
    (cfg_proposal, anchoring_cfg)
}

fn gen_add_funding_tx(
    testkit: &AnchoringTestKit,
    from_height: Height,
    funding_tx: FundingTx,
) -> (TestNetworkConfiguration, AnchoringConfig) {
    let mut cfg_proposal = testkit.configuration_change_proposal();
    cfg_proposal.set_actual_from(from_height);
    let mut anchoring_cfg: AnchoringConfig = cfg_proposal.service_config(ANCHORING_SERVICE_NAME);
    anchoring_cfg.funding_tx = Some(funding_tx);
    cfg_proposal.set_service_config(ANCHORING_SERVICE_NAME, anchoring_cfg.clone());
    (cfg_proposal, anchoring_cfg)
}

fn gen_following_cfg_unchanged_self_key(
    testkit: &mut AnchoringTestKit,
    from_height: Height,
    funds: Option<FundingTx>,
) -> (TestNetworkConfiguration, AnchoringConfig) {
    let anchoring_addr = testkit.current_addr();

    let mut cfg_proposal = testkit.configuration_change_proposal();
    cfg_proposal.set_actual_from(from_height);
    let mut anchoring_cfg: AnchoringConfig = cfg_proposal.service_config(ANCHORING_SERVICE_NAME);
    let mut priv_keys = testkit.priv_keys(&anchoring_addr);
    anchoring_cfg.anchoring_keys.swap(1, 2);
    priv_keys.swap(1, 2);
    if let Some(funds) = funds {
        anchoring_cfg.funding_tx = Some(funds);
    }

    let following_addr = anchoring_cfg.redeem_script().1;
    for (id, ref mut node) in testkit.nodes_mut().iter_mut().enumerate() {
        node.private_keys
            .insert(following_addr.to_string(), priv_keys[id].clone());
    }
    cfg_proposal.set_service_config(ANCHORING_SERVICE_NAME, anchoring_cfg.clone());

    testkit
        .handler()
        .add_private_key(&following_addr, priv_keys[0].clone());
    (cfg_proposal, anchoring_cfg)
}

fn gen_following_cfg_add_two_validators_changed_self_key(
    testkit: &mut AnchoringTestKit,
    from_height: Height,
    funds: Option<FundingTx>,
) -> (
    TestNetworkConfiguration,
    AnchoringConfig,
    Vec<AnchoringNodeConfig>,
) {
    // Create new keypair for testkit node
    let (self_keypair, anchoring_keypairs, exonum_keypairs) = {
        let mut rng: StdRng = SeedableRng::from_seed([18, 252, 3, 117].as_ref());

        let anchoring_keypairs = [
            btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng),
            btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng),
        ];
        let self_keypair = btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng);

        let exonum_keypairs = vec![
            (
                gen_keypair_from_seed(&Seed::new([212; 32])),
                gen_keypair_from_seed(&Seed::new([213; 32])),
            ),
            (
                gen_keypair_from_seed(&Seed::new([214; 32])),
                gen_keypair_from_seed(&Seed::new([215; 32])),
            ),
        ];
        (self_keypair, anchoring_keypairs, exonum_keypairs)
    };

    let mut cfg_proposal = testkit.configuration_change_proposal();
    cfg_proposal.set_actual_from(from_height);
    let mut anchoring_cfg: AnchoringConfig = cfg_proposal.service_config(ANCHORING_SERVICE_NAME);
    let mut anchoring_priv_keys = testkit.current_priv_keys();
    let mut new_nodes = Vec::new();

    anchoring_priv_keys[0] = self_keypair.1.clone();
    anchoring_cfg.anchoring_keys[0] = self_keypair.0;
    if let Some(funds) = funds {
        anchoring_cfg.funding_tx = Some(funds);
    }

    for keypair in &anchoring_keypairs {
        anchoring_cfg.anchoring_keys.push(keypair.0);
        anchoring_priv_keys.push(keypair.1.clone());
    }

    let following_addr = anchoring_cfg.redeem_script().1;
    for keypair in &anchoring_keypairs {
        let mut new_node = testkit.nodes()[0].clone();
        new_node
            .private_keys
            .insert(following_addr.to_string(), keypair.1.clone());
        new_nodes.push(new_node);
    }
    for (id, ref mut node) in testkit.nodes_mut().iter_mut().enumerate() {
        node.private_keys
            .insert(following_addr.to_string(), anchoring_priv_keys[id].clone());
    }
    testkit
        .handler()
        .add_private_key(&following_addr, anchoring_priv_keys[0].clone());

    // Update consensus config
    let mut validators = cfg_proposal.validators().to_vec();
    for keypair in exonum_keypairs {
        let node = TestNode::from_parts(keypair.0, keypair.1, None);
        validators.push(node);
    }
    cfg_proposal.set_validators(validators);
    cfg_proposal.set_service_config(ANCHORING_SERVICE_NAME, anchoring_cfg.clone());

    (cfg_proposal, anchoring_cfg, new_nodes)
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_transit_changed_self_key_normal() {
    let cfg_change_height = Height(16);

    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let (cfg_proposal, following_cfg) = gen_following_cfg(&mut testkit, cfg_change_height, None);
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    testkit.create_block();

    // Check enough confirmations case
    requests.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = testkit.gen_anchoring_tx_with_signatures(
        Height::zero(),
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = testkit.latest_anchored_tx();

    testkit.create_block();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));

    requests.expect(vec![confirmations_request(&transition_tx, 0)]);
    testkit.create_block_with_transactions(lects);

    for i in testkit.height().next().0..cfg_change_height.previous().0 {
        requests.expect(vec![confirmations_request(&transition_tx, 15 + i)]);
        testkit.create_block();
    }
    // Wait for check lect
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);
    // Gen lect for transition_tx
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_multisig.1.to_string()]],
            response: [
                listunspent_entry(&transition_tx, &following_addr, 30),
            ]
        },
        get_transaction_request(&transition_tx),
        get_transaction_request(&anchored_tx),
    ]);
    testkit.create_block();

    let transition_lect = gen_service_tx_lect(
        &testkit,
        ValidatorId(0),
        &transition_tx,
        lects_count(&testkit, ValidatorId(0)),
    );
    requests.expect(vec![confirmations_request(&transition_tx, 1000)]);

    assert!(testkit.is_tx_in_pool(&transition_lect.hash()));
    testkit.create_block_with_transactions(txvec![transition_lect]);

    let mut signatures = {
        let height = Height(10);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_multisig.1)
            .1
    };
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    requests.expect(vec![confirmations_request(&transition_tx, 100)]);
    testkit.create_block_with_transactions(signatures.drain(0..1));

    // We reached a new anchoring height and we should create a new `anchoring_tx`.
    requests.expect(vec![confirmations_request(&transition_tx, 10_000)]);
    testkit.create_block();

    let signatures = {
        let height = Height(20);
        testkit.set_latest_anchored_tx(Some((transition_tx.clone(), vec![])));

        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_multisig.1)
            .1
    };
    let anchored_tx = testkit.latest_anchored_tx();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    requests.expect(vec![
        confirmations_request(&transition_tx, 20_000),
        get_transaction_request(&anchored_tx),
    ]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &anchored_tx, lects_count(&testkit, id)))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));
    testkit.create_block_with_transactions(lects);
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_transit_unchanged_self_key_normal() {
    let cfg_change_height = Height(16);

    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let (cfg_proposal, following_cfg) =
        gen_following_cfg_unchanged_self_key(&mut testkit, cfg_change_height, None);
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    testkit.create_block();

    // Check enough confirmations case
    requests.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = testkit.gen_anchoring_tx_with_signatures(
        Height::zero(),
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = testkit.latest_anchored_tx();

    testkit.create_block();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));

    requests.expect(vec![confirmations_request(&transition_tx, 0)]);
    testkit.create_block_with_transactions(lects);

    for i in testkit.height().next().0..cfg_change_height.previous().0 {
        requests.expect(vec![confirmations_request(&transition_tx, 15 + i)]);
        testkit.create_block();
    }

    requests.expect(vec![confirmations_request(&transition_tx, 30)]);
    testkit.create_block();
    // Update cfg
    let mut signatures = {
        let height = Height(10);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_multisig.1)
            .1
    };
    let anchored_tx = testkit.latest_anchored_tx();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    requests.expect(vec![confirmations_request(&transition_tx, 40)]);
    testkit.create_block_with_transactions(signatures.drain(0..1));

    requests.expect(vec![
        confirmations_request(&transition_tx, 100),
        get_transaction_request(&anchored_tx),
    ]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &anchored_tx, 3))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_multisig.1]],
            response: [
                listunspent_entry(&anchored_tx, &following_addr, 0)
            ]
        },
        get_transaction_request(&anchored_tx),
    ]);
    testkit.create_block_with_transactions(lects);
}

// We commit a new configuration with confirmed funding tx
// and take actions to transit tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_transit_config_with_funding_tx() {
    let cfg_change_height = Height(16);

    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let funding_tx = FundingTx::from_hex(
        "0200000000010160e35919cf83fd0e2917748b0afa34311e89d3d08e070110151a5778fc83fd9a0000000000f\
         effffff02a0916e0a00000000160014a211578341dd850b6519800a55024bdb2db1ddf8a08601000000000022\
         0020c0276efb42fd5a690fc6c60a23bb2bc6a9e0562a4252c4004dfb662df83f0e9702483045022100d5fa802\
         7e7f70bf551359f62f148720ec6319df529d642744c4002f6d6a7a708022048351e6c3927d604573676176fdd\
         4b2fdb417aebf29c1f9abf14e4f028f984de012103bc41dc4c74188a89b41bbb70cae102b68590f183c7ce5ca\
         124be3a6756b1543412bd1300",
    ).unwrap();
    let (cfg_proposal, following_cfg) =
        gen_following_cfg(&mut testkit, cfg_change_height, Some(funding_tx.clone()));
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    testkit.create_block();

    // Check enough confirmations case
    requests.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = testkit.gen_anchoring_tx_with_signatures(
        Height::zero(),
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = testkit.latest_anchored_tx();

    testkit.create_block();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));

    requests.expect(vec![confirmations_request(&transition_tx, 0)]);
    testkit.create_block_with_transactions(lects);

    for i in testkit.height().next().0..cfg_change_height.previous().0 {
        requests.expect(vec![confirmations_request(&transition_tx, 15 + i)]);
        testkit.create_block();
    }
    // Update cfg
    // Wait for check lect
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);
    // Gen lect for transition_tx
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_multisig.1]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 20),
                listunspent_entry(&transition_tx, &following_addr, 20),
            ]
        },
        get_transaction_request(&funding_tx),
        get_transaction_request(&transition_tx),
        get_transaction_request(&anchored_tx),
    ]);
    testkit.create_block();

    let transition_lect = {
        let validator_0 = ValidatorId(0);
        let lect = gen_service_tx_lect(
            &testkit,
            validator_0,
            &transition_tx,
            lects_count(&testkit, validator_0),
        );
        Box::<Transaction>::from(lect)
    };
    requests.expect(vec![
        confirmations_request(&transition_tx, 1000),
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_multisig.1]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 30),
                listunspent_entry(&transition_tx, &following_addr, 30)
            ]
        },
        get_transaction_request(&funding_tx),
        get_transaction_request(&transition_tx),
    ]);

    assert!(testkit.is_tx_in_pool(&transition_lect.hash()));
    testkit.create_block_with_transactions(txvec![transition_lect]);

    let mut signatures = {
        let height = Height(10);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(
                height,
                hash,
                &[funding_tx.clone()],
                None,
                &following_multisig.1,
            )
            .1
    };
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    assert!(testkit.is_tx_in_pool(&signatures[1].hash()));
    requests.expect(vec![confirmations_request(&transition_tx, 100)]);
    testkit.create_block_with_transactions(signatures.drain(0..2));

    // We reached a new anchoring height and we should create a new `anchoring_tx`.
    requests.expect(vec![
        confirmations_request(&transition_tx, 10_000),
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_multisig.1]],
            response: [
                listunspent_entry(&transition_tx, &following_addr, 30),
                listunspent_entry(&funding_tx, &following_addr, 30),
            ]
        },
        get_transaction_request(&transition_tx),
        get_transaction_request(&funding_tx),
    ]);
    testkit.create_block();

    let signatures = {
        let height = Height(20);
        let hash = testkit.block_hash_on_height(height);
        testkit.set_latest_anchored_tx(Some((transition_tx.clone(), vec![])));

        testkit
            .gen_anchoring_tx_with_signatures(
                height,
                hash,
                &[funding_tx.clone()],
                None,
                &following_multisig.1,
            )
            .1
    };
    let anchored_tx = testkit.latest_anchored_tx();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    assert!(testkit.is_tx_in_pool(&signatures[1].hash()));
    requests.expect(vec![
        confirmations_request(&transition_tx, 20_000),
        get_transaction_request(&anchored_tx),
    ]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &anchored_tx, lects_count(&testkit, id)))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));
    testkit.create_block_with_transactions(lects);

    assert_eq!(anchored_tx.amount(), 10_1000);
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - we losing transition tx before following config height
// result: we resend it
#[test]
fn test_transit_config_lost_lect_resend_before_cfg_change() {
    let cfg_change_height = Height(16);

    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let (cfg_proposal, following_cfg) =
        gen_following_cfg_unchanged_self_key(&mut testkit, cfg_change_height, None);
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    testkit.create_block();

    // Check enough confirmations case
    requests.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = testkit.gen_anchoring_tx_with_signatures(
        Height::zero(),
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = testkit.latest_anchored_tx();

    testkit.create_block();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));

    requests.expect(vec![confirmations_request(&transition_tx, 0)]);
    testkit.create_block_with_transactions(lects);

    requests.expect(resend_raw_transaction_requests(&transition_tx));
    testkit.create_block();
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - we losing transition tx and we have no time to recovering it
// result: we trying to resend tx
#[test]
fn test_transit_config_lost_lect_resend_after_cfg_change() {
    let cfg_change_height = Height(16);

    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let (cfg_proposal, following_cfg) =
        gen_following_cfg_unchanged_self_key(&mut testkit, cfg_change_height, None);
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    testkit.create_block();

    // Check enough confirmations case
    requests.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = testkit.gen_anchoring_tx_with_signatures(
        Height::zero(),
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = testkit.latest_anchored_tx();

    testkit.create_block();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));

    requests.expect(vec![confirmations_request(&transition_tx, 0)]);
    testkit.create_block_with_transactions(lects);

    for _ in testkit.height().next().0..20 {
        requests.expect(vec![confirmations_request(&transition_tx, 20)]);
        testkit.create_block();
    }

    // Update cfg

    requests.expect(resend_raw_transaction_requests(&transition_tx));
    testkit.create_block();

    requests.expect(vec![confirmations_request(&transition_tx, 30)]);
    testkit.create_block();
    let mut signatures = {
        let height = Height(20);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_multisig.1)
            .1
    };
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    requests.expect(vec![confirmations_request(&transition_tx, 40)]);
    testkit.create_block_with_transactions(signatures.drain(0..1));
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - We have no time to create transition transaction
// result: we create a new anchoring tx chain from scratch
#[test]
fn test_transit_unchanged_self_key_recover_with_funding_tx() {
    let cfg_change_height = Height(11);

    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let funding_tx = FundingTx::from_hex(
        "02000000000101bd536be036302f230fac2b172b92a7e151e3d30a39fd2f630730024089c89c850000000000f\
         effffff02670a6d0a00000000160014ef8949227c2624837cdf02620ca54ff7d2de9070a08601000000000022\
         00205f391975ee3f84e481bb1e32bba337b62facc3f5020acb390506ffe9bb89ae6502473044022059bdc5f78\
         0353075a66edb6d265bbb3bc8b6f822eae8df5e0c25df5f3aba4028022053ac6f99abfdf9d398faa24e38f4d5\
         eff486c7e745033fdaaacbfbe463ba2bee0121036c812cf0fa389c7bf9f46e598b22a90d16bb81a6c1f244fcc\
         c4c6a05491a98ef3abd1300",
    ).unwrap();
    let (cfg_proposal, following_cfg) = gen_following_cfg_unchanged_self_key(
        &mut testkit,
        cfg_change_height,
        Some(funding_tx.clone()),
    );
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    testkit.create_block();

    for _ in testkit.height().next().0..cfg_change_height.previous().0 {
        requests.expect(vec![confirmations_request(&anchored_tx, 10)]);
        testkit.create_block();
    }

    let previous_anchored_tx = testkit.latest_anchored_tx();

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
    ]);
    testkit.create_block();

    // Update cfg
    testkit.set_latest_anchored_tx(None);
    // Generate new chain
    let mut signatures = {
        let height = Height(10);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(
                height,
                hash,
                &[],
                Some(previous_anchored_tx.id()),
                &following_addr,
            )
            .1
    };
    let new_chain_tx = testkit.latest_anchored_tx();

    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
    ]);
    testkit.create_block_with_transactions(signatures.drain(0..1));

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        get_transaction_request(&new_chain_tx),
    ]);
    testkit.create_block_with_transactions(signatures);
    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &new_chain_tx, 2))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - We have no time to create transition transaction
// result: we create a new anchoring tx chain from scratch
#[test]
fn test_transit_changed_self_key_recover_with_funding_tx() {
    let cfg_change_height = Height(11);

    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let funding_tx = FundingTx::from_hex(
        "02000000000101c84b13ffd67bbe041ac574aa5a9b3625675a8ec0e567aa131d26d92815399db90000000000f\
         effffff02a086010000000000220020c0276efb42fd5a690fc6c60a23bb2bc6a9e0562a4252c4004dfb662df8\
         3f0e972e836b0a00000000160014cf5c4c21440a983e79f13ebf15413da104c10a5602473044022065ec46d80\
         e46d33ba641aa7d70fe7e9330963e716fc77fb6055e9faa2101c51502203b173df0281839faac8ac5e9379b6f\
         9ced03b24e8294bc18bbe005f49135e53a012103b6c905dc00a9f40537bf888aab969503819460a25f39d4476\
         fb654bd31a719453fbd1300",
    ).unwrap();
    let (cfg_proposal, following_cfg) =
        gen_following_cfg(&mut testkit, cfg_change_height, Some(funding_tx.clone()));
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    testkit.create_block();

    for _ in testkit.height().next().0..cfg_change_height.previous().0 {
        requests.expect(vec![confirmations_request(&anchored_tx, 10)]);
        testkit.create_block();
    }

    let previous_anchored_tx = testkit.latest_anchored_tx();

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
    ]);
    testkit.create_block();

    // Update cfg
    testkit.set_latest_anchored_tx(None);
    // Generate new chain
    let mut signatures = {
        let height = Height(10);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(
                height,
                hash,
                &[],
                Some(previous_anchored_tx.id()),
                &following_addr,
            )
            .1
    };
    let new_chain_tx = testkit.latest_anchored_tx();

    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
    ]);
    testkit.create_block_with_transactions(signatures.drain(0..1));

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        get_transaction_request(&new_chain_tx),
    ]);
    testkit.create_block_with_transactions(signatures);
    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &new_chain_tx, lects_count(&testkit, id)))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - We have no time to create transition transaction
// and we have no suitable `funding_tx` for a new address.
// result: we create a new anchoring tx chain from scratch
#[test]
fn test_transit_changed_self_key_recover_without_funding_tx() {
    let first_cfg_change_height = Height(11);
    let second_cfg_change_height = Height(13);

    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let funding_tx = FundingTx::from_hex(
        "02000000000101bf38388e54b384527be79b3f073ed96e28dd90d2ec151ee89123652cf1fc35790100000000f\
         effffff02f5fb690a000000001600140d2481bfc824b8d44f010ede3aa310986190c2aca08601000000000022\
         0020c0276efb42fd5a690fc6c60a23bb2bc6a9e0562a4252c4004dfb662df83f0e9702473044022015dd0b7a3\
         6ad6c95c9a0fc2329c40b67a95ae96c62475890887a77395d1ce2c5022034bb49c53ec8f9f985887023b85688\
         2b13aa2966bc64e1be182eb71605c5d2ee01210360b8005275219721562b49cbd0acfc7e60f57123b2e84e9c8\
         42b1e500c2e86e13fbd1300",
    ).unwrap();
    let (cfg_proposal, following_cfg) =
        gen_following_cfg(&mut testkit, first_cfg_change_height, None);
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    testkit.create_block();

    for _ in testkit.height().next().0..(first_cfg_change_height.0 - 1) {
        requests.expect(vec![confirmations_request(&anchored_tx, 10)]);
        testkit.create_block();
    }

    // First config update
    testkit.create_block();
    testkit.set_latest_anchored_tx(None);

    // Add funding tx
    let (cfg_proposal, following_cfg) =
        gen_add_funding_tx(&testkit, second_cfg_change_height, funding_tx.clone());
    let (_, following_addr) = following_cfg.redeem_script();
    testkit.create_block();
    testkit.commit_configuration_change(cfg_proposal);

    testkit.create_blocks_until(second_cfg_change_height.previous().previous());

    // Apply new configuration
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
    ]);
    testkit.create_block();

    // Generate new chain
    let mut signatures = {
        let height = Height(10);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(
                height,
                hash,
                &[],
                Some(anchored_tx.id()),
                &following_addr,
            )
            .1
    };
    let new_chain_tx = testkit.latest_anchored_tx();

    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
    ]);
    testkit.create_block_with_transactions(signatures.drain(0..1));

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        get_transaction_request(&new_chain_tx),
    ]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &new_chain_tx, lects_count(&testkit, id)))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - We have no time to create transition transaction
// and we have no suitable `funding_tx` for a new address.
// result: we create a new anchoring tx chain from scratch
#[test]
fn test_transit_add_validators_recover_without_funding_tx() {
    let first_cfg_change_height = Height(11);
    let second_cfg_change_height = Height(13);

    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let funding_tx = FundingTx::from_hex(
        "02000000000101f9c28254f81acaf9f6d6bfde5611812b488ed6aaf3fa6cd8d0e511a16235b2db0000000000f\
         effffff02a00f0000000000002200202457deee7f0cbd45e4fad2dd180d7163f7758719b292ab154ffe314ba6\
         92ff0fbb50700a00000000160014051433cc35d247df9f0fccf5f09d152af37bb5da0247304402204b4f80846\
         bd190af39acd790515acdde1dc09980b377e51fdfe185acdb7c309d022036bc81b3828401619e0c970e1af1bf\
         2866131e536882e91128fcee62bb633344012103672484f9775e10cba3372d8fe38ab048b24c328503b085246\
         9fbd683ffa499ca3abd1300",
    ).unwrap();
    let initial_funding_tx = testkit.current_funding_tx();

    let (cfg_proposal, following_cfg, new_nodes) =
        gen_following_cfg_add_two_validators_changed_self_key(
            &mut testkit,
            first_cfg_change_height,
            None,
        );
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();

    let requests = testkit.requests();
    // Check insufficient confirmations case
    let anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    testkit.create_block();

    for _ in testkit.height().next().0..(first_cfg_change_height.0 - 1) {
        requests.expect(vec![confirmations_request(&anchored_tx, 10)]);
        testkit.create_block();
    }

    // First config update
    testkit.create_block();
    testkit.nodes_mut().extend_from_slice(&new_nodes);
    testkit.set_latest_anchored_tx(None);

    // Add funding tx
    let (cfg_proposal, following_cfg) =
        gen_add_funding_tx(&testkit, second_cfg_change_height, funding_tx.clone());
    let (_, following_addr) = following_cfg.redeem_script();
    testkit.create_block();
    testkit.commit_configuration_change(cfg_proposal);

    testkit.create_blocks_until(second_cfg_change_height.previous().previous());

    // Apply new configuration
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
    ]);
    testkit.create_block();

    // Generate new chain
    let mut signatures = {
        let height = Height(10);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(
                height,
                hash,
                &[],
                Some(initial_funding_tx.id()),
                &following_addr,
            )
            .1
    };
    let new_chain_tx = testkit.latest_anchored_tx();

    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
    ]);
    testkit.create_block_with_transactions(signatures.drain(0..1));

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        get_transaction_request(&new_chain_tx),
    ]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &new_chain_tx, lects_count(&testkit, id)))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));
}

// We send `MsgAnchoringSignature` with current output_address
// problems:
// - none
// result: msg ignored
#[test]
fn test_transit_msg_signature_incorrect_output_address() {
    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let (cfg_proposal, following_cfg) = gen_following_cfg(&mut testkit, Height(16), None);
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 0),
    ]);
    testkit.create_block();

    // Check enough confirmations case
    requests.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let mut signatures = testkit
        .gen_anchoring_tx_with_signatures(
            Height::zero(),
            anchored_tx.payload().block_hash,
            &[],
            None,
            &following_multisig.1,
        )
        .1;
    testkit.create_block();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    testkit.create_block_with_transactions(signatures.drain(0..1));

    // Gen transaction with different `output_addr`
    let different_signatures = {
        let tx = TransactionBuilder::with_prev_tx(&testkit.latest_anchored_tx(), 0)
            .fee(1000)
            .payload(Height(5), testkit.block_hash_on_height(Height(5)))
            .send_to(testkit.current_addr())
            .into_transaction()
            .unwrap();
        testkit.gen_anchoring_signatures(&tx, &[testkit.latest_anchored_tx().0])
    };
    // Try to send different messages
    let txid = different_signatures[0].tx().id();
    let signs_before = dump_signatures(&testkit, &txid);
    // Try to commit tx
    let different_signatures = different_signatures
        .into_iter()
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    testkit.create_block_with_transactions(different_signatures);
    // Ensure that service ignores tx
    let signs_after = dump_signatures(&testkit, &txid);
    assert_eq!(signs_before, signs_after);
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
// - none
// result: unimplemented
#[test]
#[should_panic(expected = "We must not to change genesis configuration!")]
fn test_transit_config_after_funding_tx() {
    let cfg_change_height = Height(16);

    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    let funding_tx = testkit.current_funding_tx();
    requests.expect(vec![confirmations_request(&funding_tx, 0)]);
    testkit.create_block();

    // Commit following configuration
    let (cfg_proposal, following_cfg) = gen_following_cfg(&mut testkit, cfg_change_height, None);
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&funding_tx, 0),
    ]);
    testkit.create_block();

    // Wait until `funding_tx` get enough confirmations
    for _ in 0..3 {
        requests.expect(vec![confirmations_request(&funding_tx, 1)]);
        testkit.create_block();
    }

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [following_addr]],
            response: []
        },
        confirmations_request(&funding_tx, 1),
    ]);
    testkit.create_block();

    // Has enough confirmations for funding_tx
    requests.expect(vec![
        confirmations_request(&funding_tx, 100),
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [following_addr]],
            response: []
        },
    ]);

    let following_multisig = following_cfg.redeem_script();
    let signatures = {
        let height = Height::zero();
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_multisig.1)
            .1
    };
    let transition_tx = testkit.latest_anchored_tx();

    testkit.create_block();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));

    requests.expect(vec![confirmations_request(&transition_tx, 0)]);
    testkit.create_block_with_transactions(lects);

    for i in testkit.height().next().0..cfg_change_height.previous().0 {
        requests.expect(vec![confirmations_request(&transition_tx, 15 + i)]);
        testkit.create_block();
    }

    requests.expect(vec![
        confirmations_request(&transition_tx, 30),
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_multisig.1]],
            response: [
                listunspent_entry(&transition_tx, &following_addr, 30)
            ]
        },
    ]);
    testkit.create_block();
    // Update cfg
    let mut signatures = {
        let height = Height(10);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_multisig.1)
            .1
    };
    let anchored_tx = testkit.latest_anchored_tx();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    requests.expect(vec![confirmations_request(&transition_tx, 40)]);
    testkit.create_block_with_transactions(signatures.drain(0..1));

    requests.expect(vec![
        confirmations_request(&transition_tx, 100),
        get_transaction_request(&anchored_tx),
    ]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &anchored_tx, 3))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_multisig.1]],
            response: [
                listunspent_entry(&anchored_tx, &following_addr, 0)
            ]
        },
        get_transaction_request(&anchored_tx),
    ]);
    testkit.create_block_with_transactions(lects);
}

// We exclude testkit node from consensus and after add it as validator
// with another validator
// problems:
// - none
// result: we continues anchoring as validator
#[test]
fn test_transit_after_exclude_from_validator() {
    let cfg_change_height = Height(16);

    let mut testkit = AnchoringTestKit::default();

    let us = testkit.network().us().clone();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);
    exclude_node_from_validators(&mut testkit);

    // Add two validators
    let (cfg_proposal, node_cfgs, following_addr) = {
        let mut rng: StdRng = SeedableRng::from_seed([3, 12, 3, 117].as_ref());
        let anchoring_keypairs = [
            btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng),
            btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng),
        ];
        let validator_keypair = (
            gen_keypair_from_seed(&Seed::new([115; 32])),
            gen_keypair_from_seed(&Seed::new([116; 32])),
        );

        let mut service_cfg = testkit.current_cfg().clone();
        let priv_keys = testkit.current_priv_keys();

        service_cfg.anchoring_keys.push(anchoring_keypairs[0].0);
        service_cfg.anchoring_keys.push(anchoring_keypairs[1].0);
        service_cfg.anchoring_keys.swap(0, 3);

        let following_addr = service_cfg.redeem_script().1;
        for (id, ref mut node) in testkit.nodes_mut().iter_mut().enumerate() {
            node.private_keys
                .insert(following_addr.to_string(), priv_keys[id].clone());
        }

        // Add a new nodes configs with private keys
        let mut node_cfgs = [testkit.nodes()[0].clone(), testkit.nodes()[0].clone()];
        for (idx, cfg) in node_cfgs.iter_mut().enumerate() {
            cfg.private_keys.clear();
            cfg.private_keys.insert(
                following_addr.to_string(),
                anchoring_keypairs[idx].1.clone(),
            );
        }
        // Add private key for service handler
        testkit
            .handler()
            .add_private_key(&following_addr, anchoring_keypairs[0].1.clone());
        // Update consensus config
        let cfg_proposal = {
            let mut cfg_proposal = testkit.configuration_change_proposal();
            cfg_proposal.set_actual_from(cfg_change_height);
            let mut validators = cfg_proposal.validators().to_vec();
            validators.push(us);
            validators.push(TestNode::from_parts(
                validator_keypair.0,
                validator_keypair.1,
                None,
            ));
            validators.swap(0, 3);
            cfg_proposal.set_validators(validators);
            cfg_proposal.set_service_config(ANCHORING_SERVICE_NAME, service_cfg);
            cfg_proposal
        };
        (cfg_proposal, node_cfgs, following_addr)
    };
    testkit.commit_configuration_change(cfg_proposal);

    let requests = testkit.requests();

    let prev_tx = testkit.latest_anchored_tx();
    let signatures = {
        let height = testkit.latest_anchoring_height();
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_addr)
            .1
    };

    let transition_tx = testkit.latest_anchored_tx();
    let lects = (0..3)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, lects_count(&testkit, id)))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();

    let txs = {
        let mut txs = signatures;
        txs.extend(lects);
        txs
    };
    // Push following cfg
    testkit.create_block_with_transactions(txs);
    // Apply following cfg
    testkit.create_blocks_until(cfg_change_height.previous().previous());
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
    ]);
    testkit.create_block();
    {
        let nodes = testkit.nodes_mut();
        nodes.extend_from_slice(&node_cfgs);
        nodes.swap(0, 3);
    }
    // Check transition tx
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);

    let lect = {
        let validator_0 = ValidatorId(0);
        gen_service_tx_lect(
            &testkit,
            validator_0,
            &transition_tx,
            lects_count(&testkit, validator_0),
        )
    };

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_addr.to_string()]],
            response: [
                listunspent_entry(&transition_tx, &following_addr, 0)
            ]
        },
        get_transaction_request(&transition_tx),
        get_transaction_request(&prev_tx),
    ]);
    testkit.create_block();

    assert!(testkit.is_tx_in_pool(&lect.hash()));
    requests.expect(vec![confirmations_request(&transition_tx, 100)]);
    testkit.create_block_with_transactions(txvec![lect]);

    // Create next anchoring tx proposal
    requests.expect(vec![confirmations_request(&transition_tx, 100)]);
    testkit.create_block();
    let signatures = {
        let height = testkit.latest_anchoring_height();
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_addr)
            .1
    };
    let anchored_tx = testkit.latest_anchored_tx();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    // Commit anchoring transaction to bitcoin blockchain
    requests.expect(vec![
        confirmations_request(&transition_tx, 1000),
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.id(), 0],
            error: RpcError::NoInformation("Unable to find tx".to_string()),
        },
        request! {
            method: "sendrawtransaction",
            params: [&anchored_tx.to_hex()],
            response: json!(&anchored_tx.to_hex())
        },
    ]);
    testkit.create_block_with_transactions(signatures);

    let validator_0 = ValidatorId(0);
    let lect = gen_service_tx_lect(
        &testkit,
        validator_0,
        &anchored_tx,
        lects_count(&testkit, validator_0),
    );
    assert!(testkit.is_tx_in_pool(&lect.hash()));
    testkit.create_block_with_transactions(txvec![lect.clone()]);

    let lects = dump_lects(&testkit, validator_0);
    assert_eq!(lects.last().unwrap(), &lect.tx());
}

// We commit a new configuration and take actions to transit tx chain to the new address.
// Also we check chain with the anchoring observer.
// problems:
// - none
// result: success
#[test]
fn test_transit_changed_self_key_observer() {
    let cfg_change_height = Height(16);

    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let (cfg_proposal, following_cfg) = gen_following_cfg(&mut testkit, cfg_change_height, None);
    testkit.commit_configuration_change(cfg_proposal);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let first_anchored_tx = testkit.latest_anchored_tx();
    requests.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&first_anchored_tx, 10),
    ]);
    testkit.create_block();

    // Check enough confirmations case
    requests.expect(vec![confirmations_request(&first_anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = testkit.gen_anchoring_tx_with_signatures(
        Height::zero(),
        first_anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = testkit.latest_anchored_tx();

    testkit.create_block();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));

    requests.expect(vec![confirmations_request(&transition_tx, 0)]);
    testkit.create_block_with_transactions(lects);

    for i in testkit.height().next().0..cfg_change_height.previous().0 {
        requests.expect(vec![confirmations_request(&transition_tx, 15 + i)]);
        testkit.create_block();
    }
    // Update cfg
    // Wait for check lect
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);
    // Gen lect for transition_tx
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&following_multisig.1]],
            response: [
                listunspent_entry(&transition_tx, &following_addr, 30),
            ]
        },
        get_transaction_request(&transition_tx),
        get_transaction_request(&first_anchored_tx),
    ]);
    testkit.create_block();

    let transition_lect = {
        let validator_0 = ValidatorId(0);
        let lect = gen_service_tx_lect(
            &testkit,
            validator_0,
            &transition_tx,
            lects_count(&testkit, validator_0),
        );
        Box::<Transaction>::from(lect)
    };
    requests.expect(vec![confirmations_request(&transition_tx, 1000)]);

    assert!(testkit.is_tx_in_pool(&transition_lect.hash()));
    testkit.create_block_with_transactions(txvec![transition_lect]);

    let mut signatures = {
        let height = Height(10);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_multisig.1)
            .1
    };
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    requests.expect(vec![confirmations_request(&transition_tx, 100)]);
    testkit.create_block_with_transactions(signatures.drain(0..1));

    // We reached a new anchoring height and we should create a new `anchoring_tx`.
    requests.expect(vec![confirmations_request(&transition_tx, 10_000)]);
    testkit.create_block();

    let signatures = {
        let height = Height(20);
        let hash = testkit.block_hash_on_height(height);
        testkit.set_latest_anchored_tx(Some((transition_tx.clone(), vec![])));

        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_multisig.1)
            .1
    };
    let third_anchored_tx = testkit.latest_anchored_tx();
    assert!(testkit.is_tx_in_pool(&signatures[0].hash()));
    requests.expect(vec![
        confirmations_request(&transition_tx, 20_000),
        get_transaction_request(&third_anchored_tx),
    ]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &third_anchored_tx, lects_count(&testkit, id)))
        .map(Box::<Transaction>::from)
        .collect::<Vec<_>>();
    assert!(testkit.is_tx_in_pool(&lects[0].hash()));
    testkit.create_block_with_transactions(lects);

    let anchoring_addr = testkit.current_addr();
    let client = TestClient::default();
    let requests = client.requests();
    let mut observer = AnchoringChainObserver::new_with_client(
        testkit.blockchain_mut().clone(),
        Box::new(client),
        0,
    );

    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr]],
            response: [
                listunspent_entry(&third_anchored_tx, &anchoring_addr, 10)
            ]
        },
        get_transaction_request(&third_anchored_tx),
        confirmations_request(&third_anchored_tx, 100),
        get_transaction_request(&transition_tx),
        confirmations_request(&transition_tx, 150),
        get_transaction_request(&first_anchored_tx),
        confirmations_request(&first_anchored_tx, 200),
        get_transaction_request(&testkit.current_funding_tx()),
    ]);

    observer.check_anchoring_chain().unwrap();

    // Checks that all anchoring transaction unsuccessfully committed to
    // `anchoring_tx_chain` table.
    let blockchain = observer.blockchain().clone();
    let snapshot = blockchain.snapshot();
    let anchoring_schema = AnchoringSchema::new(&snapshot);
    let tx_chain_index = anchoring_schema.anchoring_tx_chain();

    assert_eq!(tx_chain_index.get(&0), Some(first_anchored_tx));
    assert_eq!(tx_chain_index.get(&20), Some(third_anchored_tx));
}
