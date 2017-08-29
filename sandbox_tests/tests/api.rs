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
extern crate exonum_btc_anchoring;
#[macro_use]
extern crate exonum_btc_anchoring_sandbox;
#[macro_use]
extern crate serde_json;
extern crate bitcoin;
#[macro_use]
extern crate log;
extern crate iron;
extern crate router;
extern crate iron_test;

use bitcoin::util::base58::{FromBase58, ToBase58};
use router::Router;
use iron::Headers;
use iron::prelude::{IronResult, Response as IronResponse};

use exonum::crypto::HexValue;
use exonum::messages::Message;
use exonum::api::Api;
use exonum::blockchain::Blockchain;

use exonum_btc_anchoring::observer::AnchoringChainObserver;
use exonum_btc_anchoring::api::{AnchoringInfo, LectInfo, PublicApi};
use exonum_btc_anchoring::blockchain::dto::MsgAnchoringUpdateLatest;
use exonum_btc_anchoring::details::btc;
use exonum_btc_anchoring::details::btc::transactions::{AnchoringTx, BitcoinTx};
use exonum_btc_anchoring::details::sandbox::{Request, SandboxClient};
use exonum_btc_anchoring::details::rpc::AnchoringRpc;
use exonum_btc_anchoring_sandbox::AnchoringSandbox;
use exonum_btc_anchoring_sandbox::helpers::*;

struct ApiSandbox {
    pub router: Router,
}

impl ApiSandbox {
    fn new(anchoring_sandbox: &AnchoringSandbox) -> ApiSandbox {
        Self::new_with_blockchain(anchoring_sandbox.blockchain_ref().clone())
    }

    fn new_with_blockchain(blockchain: Blockchain) -> ApiSandbox {
        let mut router = Router::new();
        let api = PublicApi { blockchain };
        api.wire(&mut router);

        ApiSandbox { router }
    }

    fn request_get<A: AsRef<str>>(&self, route: A) -> IronResult<IronResponse> {
        request_get(&self.router, route)
    }

    fn get_actual_address(&self) -> btc::Address {
        let response = self.request_get("/v1/address/actual").unwrap();
        let body = response_body(response);
        let addr_str: String = serde_json::from_value(body).unwrap();
        btc::Address::from_base58check(&addr_str).unwrap()
    }

    fn get_following_address(&self) -> Option<btc::Address> {
        let response = self.request_get("/v1/address/following").unwrap();
        let body = response_body(response);
        let addr_str: Option<String> = serde_json::from_value(body).unwrap();
        addr_str.map(|addr_str| {
            btc::Address::from_base58check(&addr_str).unwrap()
        })
    }

    fn get_current_lect(&self) -> Option<AnchoringInfo> {
        let response = self.request_get("/v1/actual_lect/").unwrap();
        let body = response_body(response);
        serde_json::from_value(body).unwrap()
    }

    pub fn get_current_lect_of_validator(&self, validator_id: u32) -> LectInfo {
        let response = self.request_get(format!("/v1/actual_lect/{}", validator_id))
            .unwrap();
        let body = response_body(response);
        serde_json::from_value(body).unwrap()
    }

    pub fn get_nearest_anchoring_tx_for_height(&self, height: u64) -> Option<AnchoringTx> {
        let response = self.request_get(format!("/v1/nearest_lect/{}", height))
            .unwrap();
        let body = response_body(response);
        serde_json::from_value(body).unwrap()
    }
}

fn request_get<A: AsRef<str>>(router: &Router, route: A) -> IronResult<IronResponse> {
    info!(
        "GET request:'{}'",
        format!("http://127.0.0.1:8000{}", route.as_ref())
    );
    iron_test::request::get(
        &format!("http://127.0.0.1:8000{}", route.as_ref()),
        Headers::new(),
        router,
    )
}

fn response_body(response: IronResponse) -> serde_json::Value {
    if let Some(mut body) = response.body {
        let mut buf = Vec::new();
        body.write_body(&mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        trace!("Received response body:'{}'", &s);
        serde_json::from_str(&s).unwrap()
    } else {
        serde_json::Value::Null
    }
}

// Test normal api usage
#[test]
fn test_api_public_common() {
    init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);
    anchor_first_block(&sandbox);

    let lects = (0..4)
        .map(|idx| {
            gen_service_tx_lect(&sandbox, idx, &sandbox.latest_anchored_tx(), 1)
        })
        .collect::<Vec<_>>();

    let api_sandbox = ApiSandbox::new(&sandbox);
    let anchoring_info = AnchoringInfo::from(lects[0].tx());
    assert_eq!(api_sandbox.get_current_lect(), Some(anchoring_info));
    // Check validators lects
    for (id, lect) in lects.iter().enumerate() {
        let lect_info = LectInfo {
            hash: Message::hash(lect),
            content: AnchoringInfo::from(lect.tx()),
        };
        assert_eq!(
            api_sandbox.get_current_lect_of_validator(id as u32),
            lect_info
        );
    }
}

// Try to get lect from nonexistent validator id
// result: Panic
#[test]
#[should_panic(expected = "Unknown validator id")]
fn test_api_public_get_lect_nonexistent_validator() {
    init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);
    let api_sandbox = ApiSandbox::new(&sandbox);
    api_sandbox.get_current_lect_of_validator(100);
}

// Try to get current lect when there is no agreed [or consensus] lect.
// result: Returns null
#[test]
fn test_api_public_get_lect_unavailable() {
    init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);

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
    let lects = (0..2)
        .map(|id| {
            MsgAnchoringUpdateLatest::new(
                &sandbox.service_public_key(id as usize),
                id,
                lect_tx.clone(),
                lects_count(&sandbox, id),
                sandbox.service_secret_key(id as usize),
            )
        })
        .collect::<Vec<_>>();
    force_commit_lects(&sandbox, lects);

    let api_sandbox = ApiSandbox::new(&sandbox);
    assert_eq!(api_sandbox.get_current_lect(), None);
}

// Try to get actual anchoring address
#[test]
fn test_api_public_get_current_address() {
    init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);
    let api_sandbox = ApiSandbox::new(&sandbox);
    assert_eq!(api_sandbox.get_actual_address(), sandbox.current_addr());
}

// try to get following address
#[test]
fn test_api_public_get_following_address_existent() {
    init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);
    let client = sandbox.client();

    let mut cfg = sandbox.current_cfg().clone();
    cfg.anchoring_keys.swap_remove(1);
    let cfg_tx = gen_update_config_tx(&sandbox, 12, &cfg);
    let following_addr = cfg.redeem_script().1;

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);
    client.expect(vec![
        request! {
            method: "importaddress",
            params: [&following_addr.to_base58check(), "multisig", false, false]
        },
        confirmations_request(&sandbox.latest_anchored_tx(), 0),
    ]);
    sandbox.add_height(&[cfg_tx]);

    let api_sandbox = ApiSandbox::new(&sandbox);
    assert_eq!(api_sandbox.get_following_address(), Some(following_addr));
}

// try to get following address when it does not exists
// result: Returns null
#[test]
fn test_api_public_get_following_address_nonexistent() {
    init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);
    let api_sandbox = ApiSandbox::new(&sandbox);
    assert_eq!(api_sandbox.get_following_address(), None);
}

// Testing the observer for the existing anchoring chain.
#[test]
fn test_api_anchoring_observer_normal() {
    init_logger();

    let sandbox = AnchoringSandbox::initialize(&[]);
    let anchoring_addr = sandbox.current_addr();

    anchor_first_block(&sandbox);
    anchor_first_block_lect_normal(&sandbox);
    // Anchoring transaction for block with height 0.
    let first_anchored_tx = sandbox.latest_anchored_tx();

    anchor_second_block_normal(&sandbox);
    // Anchoring transaction for block with height 10.
    let second_anchored_tx = sandbox.latest_anchored_tx();

    let mut observer = AnchoringChainObserver::new_with_client(
        sandbox.blockchain_ref().clone(),
        AnchoringRpc(SandboxClient::default()),
        0,
    );
    observer.client().expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_base58check()]],
            response: [
                listunspent_entry(&second_anchored_tx, &anchoring_addr, 10)
            ]
        },
        get_transaction_request(&second_anchored_tx),
        confirmations_request(&second_anchored_tx, 100),
        get_transaction_request(&first_anchored_tx),
        confirmations_request(&first_anchored_tx, 200),
        get_transaction_request(&sandbox.current_funding_tx()),
    ]);
    observer.check_anchoring_chain().unwrap();

    let api_sandbox = ApiSandbox::new_with_blockchain(observer.blockchain().clone());

    // Check that `first_anchored_tx` anchors the block at height 0.
    assert_eq!(
        api_sandbox.get_nearest_anchoring_tx_for_height(0),
        Some(first_anchored_tx)
    );
    // Check that closest anchoring transaction for height 1 is
    // `second_anchored_tx` that anchors the block at height 10.
    assert_eq!(
        api_sandbox.get_nearest_anchoring_tx_for_height(1),
        Some(second_anchored_tx)
    );
    // Check that there are no anchoring transactions for heights that greater than 10
    assert_eq!(api_sandbox.get_nearest_anchoring_tx_for_height(11), None);
}
