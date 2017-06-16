use std::sync::{Arc, Mutex};

use bitcoin::util::base58::ToBase58;
use iron::Handler;
use serde_json;
use serde_json::value::Value;
use rand::{Rng, thread_rng};
use router::Router;

use exonum::blockchain::{ApiContext, NodeState, Service, Transaction};
use exonum::crypto::Hash;
use exonum::messages::{FromRaw, RawTransaction};
use exonum::encoding::Error as StreamStructError;
use exonum::storage::{Error as StorageError, View};
use exonum::api::Api;

use api::PublicApi;
use details::btc;
use details::rpc::{AnchoringRpc, AnchoringRpcConfig};
use details::btc::transactions::FundingTx;
use local_storage::AnchoringNodeConfig;
use handler::AnchoringHandler;
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::schema::AnchoringSchema;
use blockchain::dto::AnchoringMessage;
use error::Error as ServiceError;
#[cfg(not(feature="sandbox_tests"))]
use handler::error::Error as HandlerError;

/// Anchoring service id.
pub const ANCHORING_SERVICE_ID: u16 = 3;
/// Anchoring service name.
pub const ANCHORING_SERVICE_NAME: &'static str = "btc_anchoring";

/// An anchoring service implementation for `Exonum` blockchain.
pub struct AnchoringService {
    genesis: AnchoringConfig,
    handler: Arc<Mutex<AnchoringHandler>>,
}

impl AnchoringService {
    /// Creates a new service instance with the given `consensus` and `local` configurations.
    pub fn new(consensus_cfg: AnchoringConfig, local_cfg: AnchoringNodeConfig) -> AnchoringService {
        let client = local_cfg.rpc.clone().map(AnchoringRpc::new);
        AnchoringService {
            genesis: consensus_cfg,
            handler: Arc::new(Mutex::new(AnchoringHandler::new(client, local_cfg))),
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
        "btc_anchoring"
    }

    fn state_hash(&self, view: &View) -> Result<Vec<Hash>, StorageError> {
        AnchoringSchema::new(view).state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, StreamStructError> {
        AnchoringMessage::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn handle_genesis_block(&self, view: &View) -> Result<Value, StorageError> {
        let handler = self.handler.lock().unwrap();
        let cfg = self.genesis.clone();
        let (_, addr) = cfg.redeem_script();
        if let Some(ref client) = handler.client {
            client
                .importaddress(&addr.to_base58check(), "multisig", false, false)
                .unwrap();
        }
        AnchoringSchema::new(view).create_genesis_config(&cfg)?;
        Ok(serde_json::to_value(cfg).unwrap())
    }

    fn handle_commit(&self, state: &mut NodeState) -> Result<(), StorageError> {
        let mut handler = self.handler.lock().unwrap();
        match handler.handle_commit(state) {
            Err(ServiceError::Storage(e)) => Err(e),
            #[cfg(feature="sandbox_tests")]
            Err(ServiceError::Handler(e)) => {
                error!("An error occured: {:?}", e);
                handler.errors.push(e);
                Ok(())
            }
            #[cfg(not(feature="sandbox_tests"))]
            Err(ServiceError::Handler(e)) => {
                if let HandlerError::IncorrectLect { .. } = e {
                    panic!("A critical error occured: {}", e);
                }
                error!("An error in handler occured: {}", e);
                Ok(())
            }
            Err(e) => {
                error!("An error occured: {:?}", e);
                Ok(())
            }
            Ok(()) => Ok(()),
        }
    }

    /// Public api implementation.
    /// See [`PublicApi`](api/struct.PublicApi.html) for details.
    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = PublicApi { blockchain: context.blockchain().clone() };
        api.wire(&mut router);
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

        pub_keys.push(pub_key.clone());
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
