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

use rand::{SeedableRng, StdRng};
use bitcoin::network::constants::Network;

use exonum::messages::Message;
use exonum::helpers::{Height, ValidatorId};
use exonum::encoding::serialize::HexValue;
use exonum_testkit::{TestNetworkConfiguration, TestNode};
use exonum::crypto::{gen_keypair_from_seed, Seed};

use {AnchoringConfig, AnchoringNodeConfig, ANCHORING_SERVICE_NAME};
use observer::AnchoringChainObserver;
use blockchain::AnchoringSchema;
use details::btc;
use details::btc::transactions::{FundingTx, TransactionBuilder};
use super::{AnchoringTestKit, TestClient};
use super::helpers::*;

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
        node.private_keys.insert(
            following_addr.to_string(),
            priv_keys[id].clone(),
        );
    }
    cfg_proposal.set_service_config(ANCHORING_SERVICE_NAME, anchoring_cfg.clone());

    testkit.handler().add_private_key(
        &following_addr,
        priv_keys[0].clone(),
    );
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
    anchoring_cfg.funding_tx = Some(funding_tx.clone());
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
        node.private_keys.insert(
            following_addr.to_string(),
            priv_keys[id].clone(),
        );
    }
    cfg_proposal.set_service_config(ANCHORING_SERVICE_NAME, anchoring_cfg.clone());

    testkit.handler().add_private_key(
        &following_addr,
        priv_keys[0].clone(),
    );
    (cfg_proposal, anchoring_cfg)
}

fn gen_following_cfg_add_two_validators_changed_self_key(
    testkit: &mut AnchoringTestKit,
    from_height: Height,
    funds: Option<FundingTx>,
) -> (TestNetworkConfiguration, AnchoringConfig, Vec<AnchoringNodeConfig>) {
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
                gen_keypair_from_seed(&Seed::new([213; 32]))
            ),
            (
                gen_keypair_from_seed(&Seed::new([214; 32])),
                gen_keypair_from_seed(&Seed::new([215; 32]))
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
        new_node.private_keys.insert(
            following_addr.to_string(),
            keypair.1.clone(),
        );
        new_nodes.push(new_node);
    }
    for (id, ref mut node) in testkit.nodes_mut().iter_mut().enumerate() {
        node.private_keys.insert(
            following_addr.to_string(),
            anchoring_priv_keys[id].clone(),
        );
    }
    testkit.handler().add_private_key(
        &following_addr,
        anchoring_priv_keys[0].clone(),
    );

    // Update consensus config
    let mut validators = cfg_proposal.validators().to_vec();
    for keypair in exonum_keypairs.into_iter() {
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
    testkit.mempool().contains_key(&signatures[0].hash());

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());

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

    testkit.mempool().contains_key(&transition_lect.hash());
    testkit.create_block_with_transactions(txvec![transition_lect]);

    let mut signatures = {
        let height = Height(10);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_multisig.1)
            .1
    };
    testkit.mempool().contains_key(&signatures[0].hash());
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
    testkit.mempool().contains_key(&signatures[0].hash());
    requests.expect(vec![
        confirmations_request(&transition_tx, 20_000),
        get_transaction_request(&anchored_tx),
    ]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| {
            gen_service_tx_lect(&testkit, id, &anchored_tx, lects_count(&testkit, id))
        })
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());
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
    testkit.mempool().contains_key(&signatures[0].hash());

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());

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
    testkit.mempool().contains_key(&signatures[0].hash());
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
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());

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
        "0200000001a4f68040d03b137746fd10351c163ed4e826fd70d3db9c6\
         457c63a5e8571a47c010000006a47304402202d09a52acc5b9a40c1d8\
         9dc39c877c394b7b6804cda2bd6549bb7c66b9a1b73b02206b8a9d2ff\
         830c639050b96f97461d0f833c9e3632aaba5d704d1656de95248ca01\
         2103e82393d87254777a79476a92f5a4debeba4b5dea4d7f0df8f8319\
         be605327bebfeffffff02a08601000000000017a914ee6737f9c8f5a7\
         3bece543883a670ff3056d353387418ea107000000001976a91454cf1\
         d2fe5f7aa552c419c07914af8dea318888988ac222e1100",
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
    testkit.mempool().contains_key(&signatures[0].hash());

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());

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
        to_boxed(lect)
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

    testkit.mempool().contains_key(&transition_lect.hash());
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
    testkit.mempool().contains_key(&signatures[0].hash());
    testkit.mempool().contains_key(&signatures[1].hash());
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
    testkit.mempool().contains_key(&signatures[0].hash());
    testkit.mempool().contains_key(&signatures[1].hash());
    requests.expect(vec![
        confirmations_request(&transition_tx, 20_000),
        get_transaction_request(&anchored_tx),
    ]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| {
            gen_service_tx_lect(&testkit, id, &anchored_tx, lects_count(&testkit, id))
        })
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());
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
    testkit.mempool().contains_key(&signatures[0].hash());

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());

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
    testkit.mempool().contains_key(&signatures[0].hash());

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());

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
    testkit.mempool().contains_key(&signatures[0].hash());
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
        "0200000001cc68f92d3a37bfcb956e5d2dd0d1a38e5755892e26dfba4\
         f6c5607590fe9ba9b010000006a473044022073ef329fbe124b158980\
         ba33970550bc915f8fa9af464aa4e60fa33ecc8b76ac022036aa7ded6\
         d720c2ba086f091c648e3a633b313189b3a873653d5e95c29b0476c01\
         2103c799495eac26b9fcf31da64e70ebf3a3a073edb4e26136655c426\
         823ca49f8ebfeffffff02c106a007000000001976a914f950ca6e1756\
         d97f075b3a4f24ba890ee075083788aca08601000000000017a9142bf\
         681d557af5259acdb53b40a99ab426f40330f87252e1100",
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

    testkit.mempool().contains_key(&signatures[0].hash());
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
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());
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
        "0200000001b658a16511311568670756f3912f890441d5ea069eadf50\
         f73bcaeaf6fa91ac4000000006b483045022100da8016735aa4a31e34\
         e9a52876491952d5bcbc53dba6ee86501ad6665806d5fe02204b0df7d\
         5678c53ba0507a588ffd239d3ec1150ea218323534bd65feab3067886\
         012102da41e6c40a472b97a09dea858d8bc69c805ecc180d0955132c9\
         8a2ad04111401feffffff02213c8f07000000001976a914dfd62142b0\
         5559d396b2e036b4916e9873cfb79188aca08601000000000017a914e\
         e6737f9c8f5a73bece543883a670ff3056d3533877b2e1100",
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

    testkit.mempool().contains_key(&signatures[0].hash());
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
        .map(|id| {
            gen_service_tx_lect(&testkit, id, &new_chain_tx, lects_count(&testkit, id))
        })
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());
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
        "0200000001b658a16511311568670756f3912f890441d5ea069eadf50\
         f73bcaeaf6fa91ac4000000006b483045022100da8016735aa4a31e34\
         e9a52876491952d5bcbc53dba6ee86501ad6665806d5fe02204b0df7d\
         5678c53ba0507a588ffd239d3ec1150ea218323534bd65feab3067886\
         012102da41e6c40a472b97a09dea858d8bc69c805ecc180d0955132c9\
         8a2ad04111401feffffff02213c8f07000000001976a914dfd62142b0\
         5559d396b2e036b4916e9873cfb79188aca08601000000000017a914e\
         e6737f9c8f5a73bece543883a670ff3056d3533877b2e1100",
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

    testkit.mempool().contains_key(&signatures[0].hash());
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
        .map(|id| {
            gen_service_tx_lect(&testkit, id, &new_chain_tx, lects_count(&testkit, id))
        })
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());
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
        "0200000001e4333634a7b42fb770802a219f175bca28e63bab7457a50\
         77785cff95c411c0c010000006b483045022100b2a37136c2fd7f86da\
         af62e824470d7e95a2083df9cb78a1afb04ad5e98f035202201886fdc\
         78413f02baf99fce4bc00238911e25d959da95798349e16b1fb330e4c\
         0121027f096c405b55de7746866dec411582c322c9875824d0545765e\
         4635cb3581d82feffffff0231d58807000000001976a914ff2f437f7f\
         71ca7af810013b05a52bbd17a9774088aca08601000000000017a914f\
         975aeb4dffaf76ec07ef3dd5b8b778863feea3487542f1100",
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

    testkit.mempool().contains_key(&signatures[0].hash());
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
        .map(|id| {
            gen_service_tx_lect(&testkit, id, &new_chain_tx, lects_count(&testkit, id))
        })
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());
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
    testkit.mempool().contains_key(&signatures[0].hash());
    testkit.create_block_with_transactions(signatures.drain(0..1));

    // Gen transaction with different `output_addr`
    let different_signatures = {
        let tx = TransactionBuilder::with_prev_tx(&testkit.latest_anchored_tx(), 0)
            .fee(1000)
            .payload(Height(5), testkit.block_hash_on_height(Height(5)))
            .send_to(testkit.current_addr())
            .into_transaction()
            .unwrap();
        testkit.gen_anchoring_signatures(&tx)
    };
    // Try to send different messages
    let txid = different_signatures[0].tx().id();
    let signs_before = dump_signatures(&testkit, &txid);
    // Try to commit tx
    let different_signatures = different_signatures
        .into_iter()
        .map(to_boxed)
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
    testkit.mempool().contains_key(&signatures[0].hash());

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());

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
    testkit.mempool().contains_key(&signatures[0].hash());
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
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());

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
            node.private_keys.insert(
                following_addr.to_string(),
                priv_keys[id].clone(),
            );
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
        testkit.handler().add_private_key(
            &following_addr,
            anchoring_keypairs[0].1.clone(),
        );
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
        .map(|id| {
            gen_service_tx_lect(&testkit, id, &transition_tx, lects_count(&testkit, id))
        })
        .map(to_boxed)
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

    testkit.mempool().contains_key(&lect.hash());
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
    testkit.mempool().contains_key(&signatures[0].hash());
    // Commit anchoring transaction to bitcoin blockchain
    requests.expect(vec![
        confirmations_request(&transition_tx, 1000),
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 0],
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
    testkit.mempool().contains_key(&lect.hash());
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
    testkit.mempool().contains_key(&signatures[0].hash());

    requests.expect(vec![get_transaction_request(&transition_tx)]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| gen_service_tx_lect(&testkit, id, &transition_tx, 2))
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());

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
        to_boxed(lect)
    };
    requests.expect(vec![confirmations_request(&transition_tx, 1000)]);

    testkit.mempool().contains_key(&transition_lect.hash());
    testkit.create_block_with_transactions(txvec![transition_lect]);

    let mut signatures = {
        let height = Height(10);
        let hash = testkit.block_hash_on_height(height);
        testkit
            .gen_anchoring_tx_with_signatures(height, hash, &[], None, &following_multisig.1)
            .1
    };
    testkit.mempool().contains_key(&signatures[0].hash());
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
    testkit.mempool().contains_key(&signatures[0].hash());
    requests.expect(vec![
        confirmations_request(&transition_tx, 20_000),
        get_transaction_request(&third_anchored_tx),
    ]);
    testkit.create_block_with_transactions(signatures);

    let lects = (0..4)
        .map(ValidatorId)
        .map(|id| {
            gen_service_tx_lect(&testkit, id, &third_anchored_tx, lects_count(&testkit, id))
        })
        .map(to_boxed)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&lects[0].hash());
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

    // Checks that all anchoring transaction successfuly commited to `anchoring_tx_chain` table.
    let blockchain = observer.blockchain().clone();
    let snapshot = blockchain.snapshot();
    let anchoring_schema = AnchoringSchema::new(&snapshot);
    let tx_chain_index = anchoring_schema.anchoring_tx_chain();

    assert_eq!(tx_chain_index.get(&0), Some(first_anchored_tx));
    assert_eq!(tx_chain_index.get(&20), Some(third_anchored_tx));
}
