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
use details::rpc::{AnchoringRpc, AnchoringRpcConfig};
use details::btc::transactions::FundingTx;
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
        let client = local.rpc.clone().map(AnchoringRpc::new);
        AnchoringService {
            genesis: consensus,
            handler: Arc::new(Mutex::new(AnchoringHandler::new(client, local))),
        }
    }

    #[doc(hidden)]
    pub fn new_with_client(client: AnchoringRpc,
                           genesis: AnchoringConfig,
                           local_cfg: AnchoringNodeConfig)
                           -> AnchoringService {
        AnchoringService {
            genesis: genesis,
            handler: Arc::new(Mutex::new(AnchoringHandler::new(Some(client), local_cfg))),
        }
    }

    /// Returns an internal handler
    pub fn handler(&self) -> Arc<Mutex<AnchoringHandler>> {
        self.handler.clone()
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
            _ => Err(StreamStructError::IncorrectMessageType { message_type: raw.message_type() }),
        }
    }

    fn handle_genesis_block(&self, fork: &mut Fork) -> Value {
        let mut handler = self.handler.lock().unwrap();
        let cfg = self.genesis.clone();
        let (_, addr) = cfg.redeem_script();
        if handler.client.is_some() {
            handler.import_address(&addr).unwrap();
        }
        AnchoringSchema::new(fork).create_genesis_config(&cfg);
        serde_json::to_value(cfg).unwrap()
    }

    fn handle_commit(&self, state: &mut ServiceContext) {
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
        let router = PublicApiRouter::new(context.blockchain().clone(), &handler.node);
        Some(Box::new(router))
    }
}


/// Generates testnet configuration by given rpc for given nodes amount
/// using given random number generator.
///
/// Note: Bitcoin node that is used by rpc should have enough bitcoin amount to generate
/// funding transaction by given `total_funds`.
pub fn gen_anchoring_testnet_config_with_rng<R>(client: &AnchoringRpc,
                                                network: btc::Network,
                                                count: u8,
                                                total_funds: u64,
                                                rng: &mut R)
                                                -> (AnchoringConfig, Vec<AnchoringNodeConfig>)
    where R: Rng
{
    let network = network.into();
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
    let (_, address) = client
        .create_multisig_address(network.into(), majority_count, pub_keys.iter())
        .unwrap();
    let tx = FundingTx::create(client, &address, total_funds).unwrap();

    let genesis_cfg = AnchoringConfig::new_with_funding_tx(network, pub_keys, tx);
    for (idx, node_cfg) in node_cfgs.iter_mut().enumerate() {
        node_cfg
            .private_keys
            .insert(address.to_base58check(), priv_keys[idx].clone());
    }

    (genesis_cfg, node_cfgs)
}

/// Same as [`gen_anchoring_testnet_config_with_rng`](fn.gen_anchoring_testnet_config_with_rng.html)
/// but it uses default random number generator.
pub fn gen_anchoring_testnet_config(client: &AnchoringRpc,
                                    network: btc::Network,
                                    count: u8,
                                    total_funds: u64)
                                    -> (AnchoringConfig, Vec<AnchoringNodeConfig>) {
    let mut rng = thread_rng();
    gen_anchoring_testnet_config_with_rng(client, network, count, total_funds, &mut rng)
}

/// Helper class that combines `Router` for public api with the observer thread.
struct PublicApiRouter {
    router: Router,
    observer: Option<thread::JoinHandle<()>>,
}

impl PublicApiRouter {
    /// Creates router instance for the given `blockchain` and anchoring node `config`
    pub fn new(blockchain: Blockchain, config: &AnchoringNodeConfig) -> PublicApiRouter {
        let mut router = Router::new();
        let api = PublicApi { blockchain: blockchain.clone() };
        api.wire(&mut router);

        let observer = config.observer.clone().map(|observer_cfg| {
            let rpc_cfg = config.rpc.clone().expect("Rpc config is not setted");
            let mut observer = AnchoringChainObserver::new(blockchain.clone(), rpc_cfg, observer_cfg);

            thread::spawn(move || { observer.run().unwrap(); })
        });

        PublicApiRouter { router, observer }
    }
}

impl Handler for PublicApiRouter {
    fn handle(&self, request: &mut Request) -> IronResult<Response> {
        self.router.handle(request)
    }
}

impl Drop for PublicApiRouter {
    fn drop(&mut self) {
        if let Some(observer_thread) = self.observer.take() {
            observer_thread.join().unwrap()
        }
    }
}
