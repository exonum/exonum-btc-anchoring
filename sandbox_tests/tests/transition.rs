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
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate bitcoin;
extern crate exonum_bitcoinrpc as bitcoinrpc;
extern crate secp256k1;
extern crate rand;

use bitcoin::util::base58::ToBase58;
use bitcoin::network::constants::Network;
use rand::{SeedableRng, StdRng};

use exonum::messages::{Message, RawTransaction};
use exonum::crypto::{HexValue, Seed, gen_keypair_from_seed};
use exonum::storage::StorageValue;
use exonum::blockchain::ValidatorKeys;
use sandbox::config_updater::TxConfig;

use exonum_btc_anchoring::{ANCHORING_SERVICE_NAME, AnchoringConfig, AnchoringNodeConfig};
use exonum_btc_anchoring::details::sandbox::Request;
use exonum_btc_anchoring::details::btc;
use exonum_btc_anchoring::details::btc::transactions::{FundingTx, TransactionBuilder};
use exonum_btc_anchoring::details::rpc::AnchoringRpc;
use exonum_btc_anchoring::details::sandbox::SandboxClient;
use exonum_btc_anchoring::observer::AnchoringChainObserver;
use exonum_btc_anchoring::blockchain::AnchoringSchema;

use exonum_btc_anchoring_sandbox::AnchoringSandbox;
use exonum_btc_anchoring_sandbox::helpers::*;

fn gen_following_cfg(
    sandbox: &AnchoringSandbox,
    from_height: u64,
    funds: Option<FundingTx>,
) -> (RawTransaction, AnchoringConfig) {
    let anchoring_addr = sandbox.current_addr();

    // Create new keypair for sandbox node
    let keypair = {
        let mut rng: StdRng = SeedableRng::from_seed([18, 252, 3, 117].as_ref());
        btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng)
    };

    let mut cfg = sandbox.current_cfg().clone();
    let mut priv_keys = sandbox.priv_keys(&anchoring_addr);
    cfg.anchoring_keys[0] = keypair.0;
    priv_keys[0] = keypair.1;
    if let Some(funds) = funds {
        cfg.funding_tx = Some(funds);
    }

    let following_addr = cfg.redeem_script().1;
    for (id, ref mut node) in sandbox.nodes_mut().iter_mut().enumerate() {
        node.private_keys.insert(
            following_addr.to_base58check(),
            priv_keys[id].clone(),
        );
    }

    sandbox.handler().add_private_key(
        &following_addr,
        priv_keys[0].clone(),
    );
    (gen_update_config_tx(sandbox, from_height, &cfg), cfg)
}

fn gen_following_cfg_unchanged_self_key(
    sandbox: &AnchoringSandbox,
    from_height: u64,
    funds: Option<FundingTx>,
) -> (RawTransaction, AnchoringConfig) {
    let anchoring_addr = sandbox.current_addr();

    let mut cfg = sandbox.current_cfg().clone();
    let mut priv_keys = sandbox.priv_keys(&anchoring_addr);
    cfg.anchoring_keys.swap(1, 2);
    priv_keys.swap(1, 2);
    if let Some(funds) = funds {
        cfg.funding_tx = Some(funds);
    }

    let following_addr = cfg.redeem_script().1;
    for (id, ref mut node) in sandbox.nodes_mut().iter_mut().enumerate() {
        node.private_keys.insert(
            following_addr.to_base58check(),
            priv_keys[id].clone(),
        );
    }
    sandbox.handler().add_private_key(
        &following_addr,
        priv_keys[0].clone(),
    );
    (gen_update_config_tx(sandbox, from_height, &cfg), cfg)
}

fn gen_following_cfg_add_two_validators_changed_self_key(
    sandbox: &mut AnchoringSandbox,
    from_height: u64,
    funds: Option<FundingTx>,
) -> (RawTransaction, AnchoringConfig, Vec<AnchoringNodeConfig>) {
    // Create new keypair for sandbox node
    let (self_keypair, anchoring_keypairs, exonum_keypairs) = {
        let mut rng: StdRng = SeedableRng::from_seed([18, 252, 3, 117].as_ref());

        let anchoring_keypairs = [
            btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng),
            btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng),
        ];
        let self_keypair = btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng);

        let exonum_keypairs = [
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

    let mut anchoring_cfg = sandbox.current_cfg().clone();
    let mut anchoring_priv_keys = sandbox.current_priv_keys();
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
        let mut new_node = sandbox.nodes()[0].clone();
        new_node.private_keys.insert(
            following_addr.to_base58check(),
            keypair.1.clone(),
        );
        new_nodes.push(new_node);
    }
    for (id, ref mut node) in sandbox.nodes_mut().iter_mut().enumerate() {
        node.private_keys.insert(
            following_addr.to_base58check(),
            anchoring_priv_keys[id].clone(),
        );
    }
    sandbox.handler().add_private_key(
        &following_addr,
        anchoring_priv_keys[0].clone(),
    );

    // Update consensus config
    let consensus_cfg = {
        let mut cfg = sandbox.cfg();
        cfg.actual_from = from_height;
        cfg.previous_cfg_hash = sandbox.cfg().hash();

        for keypair in &exonum_keypairs {
            cfg.validator_keys.push(ValidatorKeys {
                consensus_key: (keypair.0).0,
                service_key: (keypair.1).0,
            });
            // Add validator to exonum sandbox validators map
            sandbox.validators_map.insert(
                (keypair.0).0,
                (keypair.0).1.clone(),
            );
            sandbox.services_map.insert(
                (keypair.1).0,
                (keypair.1).1.clone(),
            );
        }
        // Generate cfg change tx
        *cfg.services.get_mut(ANCHORING_SERVICE_NAME).unwrap() = json!(anchoring_cfg);
        cfg
    };

    let tx = TxConfig::new(
        &sandbox.service_public_key(0),
        &consensus_cfg.into_bytes(),
        from_height,
        sandbox.service_secret_key(0),
    );

    (tx.raw().clone(), anchoring_cfg, new_nodes)
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_anchoring_transit_changed_self_key_normal() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        0,
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&lects);

    for i in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![confirmations_request(&transition_tx, 15 + i)]);
        sandbox.add_height(&[]);
    }
    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    // Wait for check lect
    sandbox.fast_forward_to_height(sandbox.next_check_lect_height());
    // Gen lect for transition_tx
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                listunspent_entry(&transition_tx, &following_addr, 30),
            ]
        },
        get_transaction_request(&transition_tx),
        get_transaction_request(&anchored_tx),
    ]);
    sandbox.add_height(&[]);

    let transition_lect =
        gen_service_tx_lect(&sandbox, 0, &transition_tx, lects_count(&sandbox, 0))
            .raw()
            .clone();
    client.expect(vec![confirmations_request(&transition_tx, 1000)]);

    sandbox.broadcast(transition_lect.clone());
    sandbox.add_height(&[transition_lect]);

    let signatures = {
        let height = 10;
        sandbox
            .gen_anchoring_tx_with_signatures(
                height,
                block_hash_on_height(&sandbox, height),
                &[],
                None,
                &following_multisig.1,
            )
            .1
    };
    sandbox.broadcast(signatures[0].raw().clone());
    client.expect(vec![confirmations_request(&transition_tx, 100)]);
    sandbox.add_height(&signatures[0..1]);

    // We reached a new anchoring height and we should create a new `anchoring_tx`.
    client.expect(vec![confirmations_request(&transition_tx, 10000)]);
    sandbox.add_height(&[]);

    let signatures = {
        let height = 20;
        sandbox.set_latest_anchored_tx(Some((transition_tx.clone(), vec![])));

        sandbox
            .gen_anchoring_tx_with_signatures(
                height,
                block_hash_on_height(&sandbox, height),
                &[],
                None,
                &following_multisig.1,
            )
            .1
    };
    let anchored_tx = sandbox.latest_anchored_tx();
    sandbox.broadcast(signatures[0].raw().clone());
    client.expect(vec![
        confirmations_request(&transition_tx, 20000),
        confirmations_request(&anchored_tx, 0),
    ]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &anchored_tx, lects_count(&sandbox, id))
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
    sandbox.add_height(&lects);
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_anchoring_transit_unchanged_self_key_normal() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let (cfg_tx, following_cfg) =
        gen_following_cfg_unchanged_self_key(&sandbox, cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        0,
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&lects);

    for i in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![confirmations_request(&transition_tx, 15 + i)]);
        sandbox.add_height(&[]);
    }

    client.expect(vec![confirmations_request(&transition_tx, 30)]);
    sandbox.add_height(&[]);
    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        10,
        block_hash_on_height(&sandbox, 10),
        &[],
        None,
        &following_multisig.1,
    );
    let anchored_tx = sandbox.latest_anchored_tx();
    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![confirmations_request(&transition_tx, 40)]);
    sandbox.add_height(&signatures[0..1]);

    let signatures = signatures
        .into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    client.expect(vec![
        confirmations_request(&transition_tx, 100),
        confirmations_request(&anchored_tx, 0),
    ]);
    sandbox.add_height(&signatures[1..]);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &anchored_tx, 3)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                listunspent_entry(&anchored_tx, &following_addr, 0)
            ]
        },
        get_transaction_request(&anchored_tx),
    ]);
    sandbox.add_height(&lects);
}

// We commit a new configuration with confirmed funding tx
// and take actions to transit tx chain to the new address
// problems:
// - none
// result: success
#[test]
fn test_anchoring_transit_config_with_funding_tx() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

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
    let (cfg_tx, following_cfg) =
        gen_following_cfg(&sandbox, cfg_change_height, Some(funding_tx.clone()));
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        0,
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&lects);

    for i in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![confirmations_request(&transition_tx, 15 + i)]);
        sandbox.add_height(&[]);
    }
    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    // Wait for check lect
    sandbox.fast_forward_to_height(sandbox.next_check_lect_height());
    // Gen lect for transition_tx
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 20),
                listunspent_entry(&transition_tx, &following_addr, 20),
            ]
        },
        get_transaction_request(&funding_tx),
        get_transaction_request(&transition_tx),
        get_transaction_request(&anchored_tx),
    ]);
    sandbox.add_height(&[]);

    let transition_lect =
        gen_service_tx_lect(&sandbox, 0, &transition_tx, lects_count(&sandbox, 0))
            .raw()
            .clone();
    client.expect(vec![
        confirmations_request(&transition_tx, 1000),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 30),
                listunspent_entry(&transition_tx, &following_addr, 30)
            ]
        },
    ]);

    sandbox.broadcast(transition_lect.clone());
    sandbox.add_height(&[transition_lect]);

    let signatures = {
        let height = 10;
        sandbox
            .gen_anchoring_tx_with_signatures(
                height,
                block_hash_on_height(&sandbox, height),
                &[funding_tx.clone()],
                None,
                &following_multisig.1,
            )
            .1
    };
    sandbox.broadcast(signatures[0].raw().clone());
    sandbox.broadcast(signatures[1].raw().clone());
    client.expect(vec![confirmations_request(&transition_tx, 100)]);
    sandbox.add_height(&signatures[0..2]);

    // We reached a new anchoring height and we should create a new `anchoring_tx`.
    client.expect(vec![
        confirmations_request(&transition_tx, 10000),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                listunspent_entry(&transition_tx, &following_addr, 30),
                listunspent_entry(&funding_tx, &following_addr, 30),
            ]
        },
    ]);
    sandbox.add_height(&[]);

    let signatures = {
        let height = 20;
        sandbox.set_latest_anchored_tx(Some((transition_tx.clone(), vec![])));

        sandbox
            .gen_anchoring_tx_with_signatures(
                height,
                block_hash_on_height(&sandbox, height),
                &[funding_tx.clone()],
                None,
                &following_multisig.1,
            )
            .1
    };
    let anchored_tx = sandbox.latest_anchored_tx();
    sandbox.broadcast(signatures[0].raw().clone());
    sandbox.broadcast(signatures[1].raw().clone());
    client.expect(vec![
        confirmations_request(&transition_tx, 20000),
        confirmations_request(&anchored_tx, 0),
    ]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &anchored_tx, lects_count(&sandbox, id))
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
    sandbox.add_height(&lects);

    assert_eq!(anchored_tx.amount(), 101000);
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - we losing transition tx before following config height
// result: we resend it
#[test]
fn test_anchoring_transit_config_lost_lect_resend_before_cfg_change() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let (cfg_tx, following_cfg) =
        gen_following_cfg_unchanged_self_key(&sandbox, cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        0,
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&lects);

    client.expect(send_raw_transaction_requests(&transition_tx));
    sandbox.add_height(&[]);
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - we losing transition tx and we have no time to recovering it
// result: we trying to resend tx
#[test]
fn test_anchoring_transit_config_lost_lect_resend_after_cfg_change() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let (cfg_tx, following_cfg) =
        gen_following_cfg_unchanged_self_key(&sandbox, cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        0,
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&lects);

    for _ in sandbox.current_height()..20 {
        client.expect(vec![confirmations_request(&transition_tx, 20)]);
        sandbox.add_height(&[]);
    }

    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);

    client.expect(send_raw_transaction_requests(&transition_tx));
    sandbox.add_height(&[]);

    client.expect(vec![confirmations_request(&transition_tx, 30)]);
    sandbox.add_height(&[]);
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        20,
        block_hash_on_height(&sandbox, 20),
        &[],
        None,
        &following_multisig.1,
    );
    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![confirmations_request(&transition_tx, 40)]);
    sandbox.add_height(&signatures[0..1]);
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - We have no time to create transition transaction
// result: we create a new anchoring tx chain from scratch
#[test]
fn test_anchoring_transit_unchanged_self_key_recover_with_funding_tx() {
    let cfg_change_height = 11;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

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
    let (cfg_tx, following_cfg) =
        gen_following_cfg_unchanged_self_key(&sandbox, cfg_change_height, Some(funding_tx.clone()));
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    for _ in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![confirmations_request(&anchored_tx, 10)]);
        sandbox.add_height(&[]);
    }

    let previous_anchored_tx = sandbox.latest_anchored_tx();

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
    ]);
    sandbox.add_height(&[]);

    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    sandbox.set_latest_anchored_tx(None);
    // Generate new chain
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        10,
        block_hash_on_height(&sandbox, 10),
        &[],
        Some(previous_anchored_tx.id()),
        &following_addr,
    );
    let new_chain_tx = sandbox.latest_anchored_tx();

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
    ]);
    sandbox.add_height(&signatures[0..1]);

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        confirmations_request(&new_chain_tx, 0),
    ]);
    sandbox.add_height(&signatures[1..]);
    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &new_chain_tx, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - We have no time to create transition transaction
// result: we create a new anchoring tx chain from scratch
#[test]
fn test_anchoring_transit_changed_self_key_recover_with_funding_tx() {
    let cfg_change_height = 11;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

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
    let (cfg_tx, following_cfg) =
        gen_following_cfg(&sandbox, cfg_change_height, Some(funding_tx.clone()));
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    for _ in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![confirmations_request(&anchored_tx, 10)]);
        sandbox.add_height(&[]);
    }

    let previous_anchored_tx = sandbox.latest_anchored_tx();

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
    ]);
    sandbox.add_height(&[]);

    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    sandbox.set_latest_anchored_tx(None);
    // Generate new chain
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        10,
        block_hash_on_height(&sandbox, 10),
        &[],
        Some(previous_anchored_tx.id()),
        &following_addr,
    );
    let new_chain_tx = sandbox.latest_anchored_tx();

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
    ]);
    sandbox.add_height(&signatures[0..1]);

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        confirmations_request(&new_chain_tx, 0),
    ]);
    sandbox.add_height(&signatures[1..]);
    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &new_chain_tx, lects_count(&sandbox, id))
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - We have no time to create transition transaction
// and we have no suitable `funding_tx` for a new address.
// result: we create a new anchoring tx chain from scratch
#[test]
fn test_anchoring_transit_changed_self_key_recover_without_funding_tx() {
    let first_cfg_change_height = 11;
    let second_cfg_change_height = 13;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

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
    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, first_cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    for _ in sandbox.current_height()..(first_cfg_change_height - 1) {
        client.expect(vec![confirmations_request(&anchored_tx, 10)]);
        sandbox.add_height(&[]);
    }

    // First config update
    sandbox.add_height(&[]);
    sandbox.set_anchoring_cfg(following_cfg.clone());
    sandbox.set_latest_anchored_tx(None);

    // Add funding tx
    let (cfg_tx, following_cfg) = {
        let mut cfg = following_cfg;
        cfg.funding_tx = Some(funding_tx.clone());
        let cfg_tx = gen_update_config_tx(&sandbox, second_cfg_change_height, &cfg);
        (cfg_tx, cfg)
    };
    let (_, following_addr) = following_cfg.redeem_script();
    sandbox.add_height(&[cfg_tx]);

    sandbox.fast_forward_to_height(second_cfg_change_height - 1);

    // Apply new configuration
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
    ]);
    sandbox.add_height(&[]);
    sandbox.set_anchoring_cfg(following_cfg.clone());

    // Generate new chain
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        10,
        block_hash_on_height(&sandbox, 10),
        &[],
        Some(anchored_tx.id()),
        &following_addr,
    );
    let new_chain_tx = sandbox.latest_anchored_tx();

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
    ]);
    sandbox.add_height(&signatures[0..1]);

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        confirmations_request(&new_chain_tx, 0),
    ]);
    sandbox.add_height(&signatures[1..]);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &new_chain_tx, lects_count(&sandbox, id))
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
//  - We have no time to create transition transaction
// and we have no suitable `funding_tx` for a new address.
// result: we create a new anchoring tx chain from scratch
#[test]
fn test_anchoring_transit_add_validators_recover_without_funding_tx() {
    let first_cfg_change_height = 11;
    let second_cfg_change_height = 13;

    init_logger();
    let mut sandbox = AnchoringSandbox::initialize(&[]);

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

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
    let initial_funding_tx = sandbox.current_funding_tx();

    let (cfg_tx, following_cfg, new_nodes) =
        gen_following_cfg_add_two_validators_changed_self_key(
            &mut sandbox,
            first_cfg_change_height,
            None,
        );
    let (_, following_addr) = following_cfg.redeem_script();

    let client = sandbox.client();
    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    for _ in sandbox.current_height()..(first_cfg_change_height - 1) {
        client.expect(vec![confirmations_request(&anchored_tx, 10)]);
        sandbox.add_height(&[]);
    }

    // First config update
    sandbox.add_height(&[]);
    sandbox.set_anchoring_cfg(following_cfg.clone());
    sandbox.nodes_mut().extend_from_slice(&new_nodes);
    sandbox.set_latest_anchored_tx(None);

    // Add funding tx
    let (cfg_tx, following_cfg) = {
        let mut cfg = following_cfg;
        cfg.funding_tx = Some(funding_tx.clone());
        let cfg_tx = gen_update_config_tx(&sandbox, second_cfg_change_height, &cfg);
        (cfg_tx, cfg)
    };
    let (_, following_addr) = following_cfg.redeem_script();
    sandbox.add_height(&[cfg_tx]);

    sandbox.fast_forward_to_height(second_cfg_change_height - 1);

    // Apply new configuration
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        get_transaction_request(&funding_tx),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
    ]);
    sandbox.add_height(&[]);
    sandbox.set_anchoring_cfg(following_cfg.clone());

    // Generate new chain
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        10,
        block_hash_on_height(&sandbox, 10),
        &[],
        Some(initial_funding_tx.id()),
        &following_addr,
    );
    let new_chain_tx = sandbox.latest_anchored_tx();

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
    ]);
    sandbox.add_height(&signatures[0..1]);

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&funding_tx, &following_addr, 200)
            ]
        },
        confirmations_request(&new_chain_tx, 0),
    ]);
    sandbox.add_height(&signatures[1..]);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &new_chain_tx, lects_count(&sandbox, id))
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
}

// We send `MsgAnchoringSignature` with current output_address
// problems:
// - none
// result: msg ignored
#[test]
fn test_anchoring_transit_msg_signature_incorrect_output_address() {
    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, 16, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&anchored_tx, 0),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![confirmations_request(&anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        0,
        anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());
    sandbox.add_height(&signatures[0..1]);

    // Gen transaction with different `output_addr`
    let different_signatures = {
        let tx = TransactionBuilder::with_prev_tx(&sandbox.latest_anchored_tx(), 0)
            .fee(1000)
            .payload(5, block_hash_on_height(&sandbox, 5))
            .send_to(sandbox.current_addr())
            .into_transaction()
            .unwrap();
        sandbox.gen_anchoring_signatures(&tx)
    };
    // Try to send different messages
    let txid = different_signatures[0].tx().id();
    let signs_before = dump_signatures(&sandbox, &txid);
    // Try to commit tx
    let different_signatures = different_signatures
        .into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    sandbox.add_height(&different_signatures);
    // Ensure that service ignores tx
    let signs_after = dump_signatures(&sandbox, &txid);
    assert_eq!(signs_before, signs_after);

}

// We commit a new configuration and take actions to transit tx chain to the new address
// problems:
// - none
// result: unimplemented
#[test]
#[should_panic(expected = "We must not to change genesis configuration!")]
fn test_anchoring_transit_config_after_funding_tx() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    let funding_tx = sandbox.current_funding_tx();
    client.expect(vec![confirmations_request(&funding_tx, 0)]);
    sandbox.add_height(&[]);

    // Commit following configuration
    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&funding_tx, 0),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Wait until `funding_tx` get enough confirmations
    for _ in 0..3 {
        client.expect(vec![confirmations_request(&funding_tx, 1)]);
        sandbox.add_height(&[]);
    }

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [following_addr]],
            response: []
        },
        confirmations_request(&funding_tx, 1),
    ]);
    sandbox.add_height(&[]);

    // Has enough confirmations for funding_tx
    client.expect(vec![
        confirmations_request(&funding_tx, 100),
        request! {
            method: "listunspent",
            params: [0, 9999999, [following_addr]],
            response: []
        },
    ]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        0,
        block_hash_on_height(&sandbox, 0),
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&lects);

    for i in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![confirmations_request(&transition_tx, 15 + i)]);
        sandbox.add_height(&[]);
    }

    client.expect(vec![
        confirmations_request(&transition_tx, 30),
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                listunspent_entry(&transition_tx, &following_addr, 30)
            ]
        },
    ]);
    sandbox.add_height(&[]);
    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        10,
        block_hash_on_height(&sandbox, 10),
        &[],
        None,
        &following_multisig.1,
    );
    let anchored_tx = sandbox.latest_anchored_tx();
    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![confirmations_request(&transition_tx, 40)]);
    sandbox.add_height(&signatures[0..1]);

    let signatures = signatures
        .into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    client.expect(vec![
        confirmations_request(&transition_tx, 100),
        confirmations_request(&anchored_tx, 0),
    ]);
    sandbox.add_height(&signatures[1..]);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &anchored_tx, 3)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                listunspent_entry(&anchored_tx, &following_addr, 0)
            ]
        },
        get_transaction_request(&anchored_tx),
    ]);
    sandbox.add_height(&lects);
}

// We exclude sandbox node from consensus and after add it as validator
// with another validator
// problems:
// - none
// result: we continues anchoring as validator
#[test]
fn test_anchoring_transit_after_exclude_from_validator() {
    let cfg_change_height = 16;

    let _ = exonum::helpers::init_logger();

    let mut sandbox = AnchoringSandbox::initialize(&[]);

    let sandbox_consensus_pubkey = sandbox.cfg().validator_keys[0].consensus_key;
    let sandbox_service_pubkey = sandbox.cfg().validator_keys[0].service_key;

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);
    exclude_node_from_validators(&sandbox);

    // Add two validators
    let (cfg_tx, cfg, node_cfgs, following_addr) = {
        let mut rng: StdRng = SeedableRng::from_seed([3, 12, 3, 117].as_ref());
        let anchoring_keypairs = [
            btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng),
            btc::gen_btc_keypair_with_rng(Network::Testnet, &mut rng),
        ];
        let validator_keypair = (
            gen_keypair_from_seed(&Seed::new([115; 32])),
            gen_keypair_from_seed(&Seed::new([116; 32])),
        );

        let mut service_cfg = sandbox.current_cfg().clone();
        let priv_keys = sandbox.current_priv_keys();

        service_cfg.anchoring_keys.push(anchoring_keypairs[0].0);
        service_cfg.anchoring_keys.push(anchoring_keypairs[1].0);
        service_cfg.anchoring_keys.swap(0, 3);

        let following_addr = service_cfg.redeem_script().1;
        for (id, ref mut node) in sandbox.nodes_mut().iter_mut().enumerate() {
            node.private_keys.insert(
                following_addr.to_base58check(),
                priv_keys[id].clone(),
            );
        }

        // Add a new nodes configs with private keys
        let mut node_cfgs = [sandbox.nodes()[0].clone(), sandbox.nodes()[0].clone()];
        for (idx, cfg) in node_cfgs.iter_mut().enumerate() {
            cfg.private_keys.clear();
            cfg.private_keys.insert(
                following_addr.to_base58check(),
                anchoring_keypairs[idx].1.clone(),
            );
        }
        // Add private key for service handler
        sandbox.handler().add_private_key(
            &following_addr,
            anchoring_keypairs[0].1.clone(),
        );
        // Update consensus config
        let consensus_cfg = {
            let mut cfg = sandbox.cfg();
            cfg.actual_from = cfg_change_height;
            cfg.previous_cfg_hash = sandbox.cfg().hash();
            cfg.validator_keys.push(ValidatorKeys {
                consensus_key: sandbox_consensus_pubkey,
                service_key: sandbox_service_pubkey,
            });
            cfg.validator_keys.push(ValidatorKeys {
                consensus_key: (validator_keypair.0).0,
                service_key: (validator_keypair.1).0,
            });
            cfg.validator_keys.swap(0, 3);
            // Generate cfg change tx
            *cfg.services.get_mut(ANCHORING_SERVICE_NAME).unwrap() = json!(service_cfg);
            cfg
        };

        let tx = TxConfig::new(
            &sandbox.service_public_key(1),
            &consensus_cfg.into_bytes(),
            cfg_change_height,
            sandbox.service_secret_key(1),
        );
        // Add validator to exonum sandbox validators map
        sandbox.validators_map.insert(
            (validator_keypair.0).0,
            (validator_keypair.0).1,
        );
        sandbox.services_map.insert(
            (validator_keypair.1).0,
            (validator_keypair.1).1,
        );
        (tx.raw().clone(), service_cfg, node_cfgs, following_addr)
    };

    let client = sandbox.client();

    let prev_tx = sandbox.latest_anchored_tx();
    let signatures = {
        let height = sandbox.latest_anchoring_height();
        sandbox
            .gen_anchoring_tx_with_signatures(
                height,
                block_hash_on_height(&sandbox, height),
                &[],
                None,
                &following_addr,
            )
            .1
    };

    let transition_tx = sandbox.latest_anchored_tx();
    let lects = (0..3)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &transition_tx, lects_count(&sandbox, id))
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();

    let txs = [&[cfg_tx], signatures.as_slice(), lects.as_slice()].concat();
    // Push following cfg
    sandbox.add_height_as_auditor(&txs);
    // Apply following cfg
    sandbox.fast_forward_to_height_as_auditor(cfg_change_height - 1);
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
    ]);
    sandbox.add_height_as_auditor(&[]);
    sandbox.set_anchoring_cfg(cfg);
    {
        let mut nodes = sandbox.nodes_mut();
        nodes.extend_from_slice(&node_cfgs);
        nodes.swap(0, 3);
    }
    // Check transition tx
    sandbox.fast_forward_to_height(sandbox.next_check_lect_height());

    let lect = gen_service_tx_lect(&sandbox, 0, &transition_tx, lects_count(&sandbox, 0))
        .raw()
        .clone();

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_addr.to_base58check()]],
            response: [
                listunspent_entry(&transition_tx, &following_addr, 0)
            ]
        },
        get_transaction_request(&transition_tx),
        get_transaction_request(&prev_tx),
    ]);
    sandbox.add_height(&[]);

    sandbox.broadcast(lect.clone());
    client.expect(vec![confirmations_request(&transition_tx, 100)]);
    sandbox.add_height(&[lect]);

    // Create next anchoring tx proposal
    client.expect(vec![confirmations_request(&transition_tx, 100)]);
    sandbox.add_height(&[]);
    let signatures = {
        let height = sandbox.latest_anchoring_height();
        sandbox
            .gen_anchoring_tx_with_signatures(
                height,
                block_hash_on_height(&sandbox, height),
                &[],
                None,
                &following_addr,
            )
            .1
    };
    let anchored_tx = sandbox.latest_anchored_tx();
    sandbox.broadcast(signatures[0].clone());
    // Commit anchoring transaction to bitcoin blockchain
    client.expect(vec![
        confirmations_request(&transition_tx, 1000),
        request! {
            method: "getrawtransaction",
            params: [&anchored_tx.txid(), 1],
            error: RpcError::NoInformation("Unable to find tx".to_string()),
        },
        request! {
            method: "sendrawtransaction",
            params: [&anchored_tx.to_hex()],
            response: json!(&anchored_tx.to_hex())
        },
    ]);
    sandbox.add_height(&signatures);

    let lect = gen_service_tx_lect(&sandbox, 0, &anchored_tx, lects_count(&sandbox, 0));
    sandbox.broadcast(lect.clone());
    sandbox.add_height(&[lect.raw().clone()]);

    let lects = dump_lects(&sandbox, 0);
    assert_eq!(lects.last().unwrap(), &lect.tx());
}

// We commit a new configuration and take actions to transit tx chain to the new address.
// Also we check chain with the anchoring observer.
// problems:
// - none
// result: success
#[test]
fn test_anchoring_transit_changed_self_key_observer() {
    let cfg_change_height = 16;

    init_logger();
    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);

    let (cfg_tx, following_cfg) = gen_following_cfg(&sandbox, cfg_change_height, None);
    let (_, following_addr) = following_cfg.redeem_script();

    // Check insufficient confirmations case
    let first_anchored_tx = sandbox.latest_anchored_tx();
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr, "multisig", false, false]
        },
        confirmations_request(&first_anchored_tx, 10),
    ]);
    sandbox.add_height(&[cfg_tx]);

    // Check enough confirmations case
    client.expect(vec![confirmations_request(&first_anchored_tx, 100)]);

    let following_multisig = following_cfg.redeem_script();
    let (_, signatures) = sandbox.gen_anchoring_tx_with_signatures(
        0,
        first_anchored_tx.payload().block_hash,
        &[],
        None,
        &following_multisig.1,
    );
    let transition_tx = sandbox.latest_anchored_tx();

    sandbox.add_height(&[]);
    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &transition_tx, 2)
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());

    client.expect(vec![confirmations_request(&transition_tx, 0)]);
    sandbox.add_height(&lects);

    for i in sandbox.current_height()..(cfg_change_height - 1) {
        client.expect(vec![confirmations_request(&transition_tx, 15 + i)]);
        sandbox.add_height(&[]);
    }
    // Update cfg
    sandbox.set_anchoring_cfg(following_cfg);
    // Wait for check lect
    sandbox.fast_forward_to_height(sandbox.next_check_lect_height());
    // Gen lect for transition_tx
    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&following_multisig.1.to_base58check()]],
            response: [
                listunspent_entry(&transition_tx, &following_addr, 30),
            ]
        },
        get_transaction_request(&transition_tx),
        get_transaction_request(&first_anchored_tx),
    ]);
    sandbox.add_height(&[]);

    let transition_lect =
        gen_service_tx_lect(&sandbox, 0, &transition_tx, lects_count(&sandbox, 0))
            .raw()
            .clone();
    client.expect(vec![confirmations_request(&transition_tx, 1000)]);

    sandbox.broadcast(transition_lect.clone());
    sandbox.add_height(&[transition_lect]);

    let signatures = {
        let height = 10;
        sandbox
            .gen_anchoring_tx_with_signatures(
                height,
                block_hash_on_height(&sandbox, height),
                &[],
                None,
                &following_multisig.1,
            )
            .1
    };
    sandbox.broadcast(signatures[0].raw().clone());
    client.expect(vec![confirmations_request(&transition_tx, 100)]);
    sandbox.add_height(&signatures[0..1]);

    // We reached a new anchoring height and we should create a new `anchoring_tx`.
    client.expect(vec![confirmations_request(&transition_tx, 10000)]);
    sandbox.add_height(&[]);

    let signatures = {
        let height = 20;
        sandbox.set_latest_anchored_tx(Some((transition_tx.clone(), vec![])));

        sandbox
            .gen_anchoring_tx_with_signatures(
                height,
                block_hash_on_height(&sandbox, height),
                &[],
                None,
                &following_multisig.1,
            )
            .1
    };
    let third_anchored_tx = sandbox.latest_anchored_tx();
    sandbox.broadcast(signatures[0].raw().clone());
    client.expect(vec![
        confirmations_request(&transition_tx, 20000),
        confirmations_request(&third_anchored_tx, 0),
    ]);
    sandbox.add_height(&signatures);

    let lects = (0..4)
        .map(|id| {
            gen_service_tx_lect(&sandbox, id, &third_anchored_tx, lects_count(&sandbox, id))
                .raw()
                .clone()
        })
        .collect::<Vec<_>>();
    sandbox.broadcast(lects[0].clone());
    sandbox.add_height(&lects);

    let anchoring_addr = sandbox.current_addr();
    let mut observer = AnchoringChainObserver::new_with_client(
        sandbox.blockchain_ref().clone(),
        AnchoringRpc(SandboxClient::default()),
        0,
    );

    observer.client().expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, [&anchoring_addr.to_base58check()]],
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
        get_transaction_request(&sandbox.current_funding_tx()),
    ]);

    observer.check_anchoring_chain().unwrap();

    /// Checks that all anchoring transaction successfuly commited to `anchoring_tx_chain` table.
    let blockchain = observer.blockchain().clone();
    let snapshot = blockchain.snapshot();
    let anchoring_schema = AnchoringSchema::new(&snapshot);
    let tx_chain_index = anchoring_schema.anchoring_tx_chain();

    assert_eq!(tx_chain_index.get(&0), Some(first_anchored_tx));
    assert_eq!(tx_chain_index.get(&20), Some(third_anchored_tx));
}
