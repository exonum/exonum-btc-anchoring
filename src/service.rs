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

use std::sync::{Arc, Mutex};
use std::thread;
use std::ops::Drop;

use bitcoin::util::base58::ToBase58;
use iron::{Handler, Request, Response};
use iron::prelude::IronResult;
use serde_json;
use serde_json::value::Value;
use rand::{Rng, thread_rng};
use router::Router;

use exonum::blockchain::{ApiContext, Blockchain, Service, ServiceContext, Transaction};
use exonum::crypto::Hash;
use exonum::messages::{FromRaw, RawTransaction};
use exonum::encoding::Error as StreamStructError;
use exonum::storage::{Fork, Snapshot};
use exonum::api::Api;

use api::PublicApi;
use details::btc;
use details::rpc::{AnchoringRpc, AnchoringRpcConfig, BitcoinRelay};
use local_storage::AnchoringNodeConfig;
use handler::AnchoringHandler;
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::schema::AnchoringSchema;
use blockchain::dto::{ANCHORING_MESSAGE_LATEST, ANCHORING_MESSAGE_SIGNATURE,
                      MsgAnchoringSignature, MsgAnchoringUpdateLatest};
use error::Error as ServiceError;
#[cfg(not(feature = "sandbox_tests"))]
use handler::error::Error as HandlerError;
use observer::AnchoringChainObserver;

/// Anchoring service id.
pub const ANCHORING_SERVICE_ID: u16 = 3;
/// Anchoring service name.
pub const ANCHORING_SERVICE_NAME: &'static str = "btc_anchoring";

/// Anchoring service implementation for the Exonum blockchain.
#[derive(Debug)]
pub struct AnchoringService {
    genesis: AnchoringConfig,
    handler: Arc<Mutex<AnchoringHandler>>,
}

impl AnchoringService {
    /// Creates a new service instance with the given `consensus` and `local` configurations.
    pub fn new(consensus: AnchoringConfig, local: AnchoringNodeConfig) -> AnchoringService {
        let client = local.rpc.clone().map(AnchoringRpc::new).map(Into::into);
        AnchoringService {
            genesis: consensus,
            handler: Arc::new(Mutex::new(AnchoringHandler::new(client, local))),
        }
    }

    #[doc(hidden)]
    pub fn new_with_client(
        client: Box<BitcoinRelay>,
        genesis: AnchoringConfig,
        local_cfg: AnchoringNodeConfig,
    ) -> AnchoringService {
        AnchoringService {
            genesis: genesis,
            handler: Arc::new(Mutex::new(AnchoringHandler::new(Some(client), local_cfg))),
        }
    }

    /// Returns an internal handler
    pub fn handler(&self) -> Arc<Mutex<AnchoringHandler>> {
        Arc::clone(&self.handler)
    }
}

impl Service for AnchoringService {
    fn service_id(&self) -> u16 {
        ANCHORING_SERVICE_ID
    }

    fn service_name(&self) -> &'static str {
        ANCHORING_SERVICE_NAME
    }

    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
        let schema = AnchoringSchema::new(snapshot);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, StreamStructError> {
        match raw.message_type() {
            ANCHORING_MESSAGE_LATEST => Ok(Box::new(MsgAnchoringUpdateLatest::from_raw(raw)?)),
            ANCHORING_MESSAGE_SIGNATURE => Ok(Box::new(MsgAnchoringSignature::from_raw(raw)?)),
            _ => Err(StreamStructError::IncorrectMessageType {
                message_type: raw.message_type(),
            }),
        }
    }

    fn initialize(&self, fork: &mut Fork) -> Value {
        let mut handler = self.handler.lock().unwrap();
        let cfg = self.genesis.clone();
        let (_, addr) = cfg.redeem_script();
        if handler.client.is_some() {
            handler.import_address(&addr).unwrap();
        }
        AnchoringSchema::new(fork).create_genesis_config(&cfg);
        serde_json::to_value(cfg).unwrap()
    }

    fn handle_commit(&self, state: &ServiceContext) {
        let mut handler = self.handler.lock().unwrap();
        match handler.handle_commit(state) {
            #[cfg(feature = "sandbox_tests")]
            Err(ServiceError::Handler(e)) => {
                error!("An error occured: {:?}", e);
                handler.errors.push(e);
            }
            #[cfg(not(feature = "sandbox_tests"))]
            Err(ServiceError::Handler(e)) => {
                if let HandlerError::IncorrectLect { .. } = e {
                    panic!("A critical error occured: {}", e);
                }
                error!("An error in handler occured: {}", e);
            }
            Err(e) => {
                error!("An error occured: {:?}", e);
            }
            Ok(()) => (),
        }
    }

    /// Public api implementation.
    /// See [`PublicApi`](api/struct.PublicApi.html) for details.
    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        let handler = self.handler.lock().unwrap();
        let router = PublicApiHandler::new(context.blockchain().clone(), &handler.node);
        Some(Box::new(router))
    }
}


/// Generates testnet configuration by given rpc for given nodes amount
/// using given random number generator.
///
/// Note: Bitcoin node that is used by rpc should have enough bitcoin amount to generate
/// funding transaction by given `total_funds`.
pub fn gen_anchoring_testnet_config_with_rng<R>(
    client: &AnchoringRpc,
    network: btc::Network,
    count: u8,
    total_funds: u64,
    rng: &mut R,
) -> (AnchoringConfig, Vec<AnchoringNodeConfig>)
where
    R: Rng,
{
    let rpc = AnchoringRpcConfig {
        host: client.url().into(),
        username: client.username().clone(),
        password: client.password().clone(),
    };
    let mut pub_keys = Vec::new();
    let mut node_cfgs = Vec::new();
    let mut priv_keys = Vec::new();

    for _ in 0..count as usize {
        let (pub_key, priv_key) = btc::gen_btc_keypair_with_rng(network, rng);

        pub_keys.push(pub_key);
        node_cfgs.push(AnchoringNodeConfig::new(Some(rpc.clone())));
        priv_keys.push(priv_key.clone());
    }

    let majority_count = ::majority_count(count);
    let address = btc::RedeemScript::from_pubkeys(&pub_keys, majority_count).compressed(network).to_address(network);
    client.watch_address(&address, false).unwrap();
    let tx = client.send_to_address(&address, total_funds).unwrap();

    let genesis_cfg = AnchoringConfig::new_with_funding_tx(network, pub_keys, tx);
    for (idx, node_cfg) in node_cfgs.iter_mut().enumerate() {
        node_cfg.private_keys.insert(
            address.to_base58check(),
            priv_keys[idx].clone(),
        );
    }

    (genesis_cfg, node_cfgs)
}

/// Same as [`gen_anchoring_testnet_config_with_rng`](fn.gen_anchoring_testnet_config_with_rng.html)
/// but it uses default random number generator.
pub fn gen_anchoring_testnet_config(
    client: &AnchoringRpc,
    network: btc::Network,
    count: u8,
    total_funds: u64,
) -> (AnchoringConfig, Vec<AnchoringNodeConfig>) {
    let mut rng = thread_rng();
    gen_anchoring_testnet_config_with_rng(client, network, count, total_funds, &mut rng)
}

/// Helper class that combines `Router` for public api with the observer thread.
struct PublicApiHandler {
    router: Router,
    observer: Option<thread::JoinHandle<()>>,
}

impl PublicApiHandler {
    /// Creates public api handler instance for the given `blockchain`
    /// and anchoring node `config`.
    pub fn new(blockchain: Blockchain, config: &AnchoringNodeConfig) -> PublicApiHandler {
        let mut router = Router::new();
        let api = PublicApi { blockchain: blockchain.clone() };
        api.wire(&mut router);

        let observer = if config.observer.enabled {
            let rpc_cfg = config.rpc.clone().expect("Rpc config is not setted");
            let mut observer =
                AnchoringChainObserver::new(blockchain.clone(), rpc_cfg, config.observer.clone());

            Some(thread::spawn(move || { observer.run().unwrap(); }))
        } else {
            None
        };

        PublicApiHandler { router, observer }
    }
}

impl Handler for PublicApiHandler {
    fn handle(&self, request: &mut Request) -> IronResult<Response> {
        self.router.handle(request)
    }
}

impl Drop for PublicApiHandler {
    fn drop(&mut self) {
        if let Some(observer_thread) = self.observer.take() {
            observer_thread.join().unwrap()
        }
    }
}
