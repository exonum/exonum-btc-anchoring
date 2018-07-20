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

use rand::{thread_rng, Rng};
use serde_json;
use serde_json::value::Value;

use exonum::api::ServiceApiBuilder;
use exonum::blockchain::{Schema as CoreSchema, Service, ServiceContext, Transaction};
use exonum::crypto::Hash;
use exonum::encoding::Error as StreamStructError;
use exonum::messages::RawTransaction;
use exonum::storage::{Fork, Snapshot};

use api;
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::dto;
use blockchain::schema::AnchoringSchema;
use details::btc;
use details::rpc::{BitcoinRelay, RpcClient};
use error::Error as ServiceError;
use handler::{error::Error as HandlerError, observer::AnchoringChainObserver, AnchoringHandler};
use local_storage::AnchoringNodeConfig;

/// Anchoring service id.
pub const ANCHORING_SERVICE_ID: u16 = 3;
/// Anchoring service name.
pub const ANCHORING_SERVICE_NAME: &str = "btc_anchoring";

/// Anchoring service implementation for the Exonum blockchain.
#[derive(Debug)]
pub struct AnchoringService {
    genesis: AnchoringConfig,
    local: AnchoringNodeConfig,
    handler: Arc<Mutex<AnchoringHandler>>,
}

impl AnchoringService {
    /// Creates a new service instance with the given `consensus` and `local` configurations.
    pub fn new(consensus: AnchoringConfig, local: AnchoringNodeConfig) -> AnchoringService {
        let client = local.rpc.clone().map(RpcClient::from).map(Into::into);
        AnchoringService {
            genesis: consensus,
            handler: Arc::new(Mutex::new(AnchoringHandler::new(client, local.clone()))),
            local,
        }
    }

    #[doc(hidden)]
    pub fn new_with_client(
        client: Box<BitcoinRelay>,
        genesis: AnchoringConfig,
        local: AnchoringNodeConfig,
    ) -> AnchoringService {
        AnchoringService {
            genesis,
            handler: Arc::new(Mutex::new(AnchoringHandler::new(
                Some(client),
                local.clone(),
            ))),
            local,
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
        dto::tx_from_raw(raw)
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

    fn before_commit(&self, fork: &mut Fork) {
        // Writes a hash of the latest block to the proof list index.
        let block_header_hash = CoreSchema::new(&fork)
            .block_hashes_by_height()
            .last()
            .expect("An attempt to invoke execute during the genesis block initialization.");

        let fork = {
            let mut schema = AnchoringSchema::new(fork);
            schema.anchored_blocks_mut().push(block_header_hash);
            schema.into_inner()
        };

        if self.local.observer.enabled {
            let height = CoreSchema::new(&fork).height();
            if height.0 % self.local.observer.check_interval.0 == 0 {
                let handler = self.handler.lock().unwrap();
                let client = handler.client();
                let observer = AnchoringChainObserver::new(fork, client);
                if let Err(e) = observer.check_anchoring_chain() {
                    error!(
                        "An error during `check_anchoring_chain` occurred, msg={:?}",
                        e
                    );
                }
            }
        }
    }

    fn after_commit(&self, state: &ServiceContext) {
        let mut handler = self.handler.lock().unwrap();
        match handler.after_commit(state) {
            Err(ServiceError::Handler(e @ HandlerError::IncorrectLect { .. })) => {
                panic!("A critical error occurred: {}", e)
            }
            Err(ServiceError::Handler(e)) => {
                error!("An error in handler occurred: {}", e);
                if let Some(sink) = handler.errors_sink.as_ref() {
                    let res = sink.send(e);
                    if let Err(err) = res {
                        error!("Can't send error to channel: {}", err);
                    }
                }
            }
            Err(e) => {
                error!("An error occurred: {:?}", e);
            }
            Ok(()) => (),
        }
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::wire(builder);
    }
}

/// Generates testnet configuration by given rpc for given nodes amount
/// using given random number generator.
///
/// Note: Bitcoin node that is used by rpc should have enough bitcoin amount to generate
/// funding transaction by given `total_funds`.
pub fn gen_anchoring_testnet_config_with_rng<R>(
    client: &BitcoinRelay,
    network: btc::Network,
    count: u8,
    total_funds: u64,
    rng: &mut R,
) -> (AnchoringConfig, Vec<AnchoringNodeConfig>)
where
    R: Rng,
{
    let rpc = client.config();
    let mut pub_keys = Vec::new();
    let mut node_cfgs = Vec::new();
    let mut priv_keys = Vec::new();

    for _ in 0..count as usize {
        let (pub_key, priv_key) = btc::gen_btc_keypair_with_rng(network, rng);

        pub_keys.push(pub_key);
        node_cfgs.push(AnchoringNodeConfig::new(Some(rpc.clone())));
        priv_keys.push(priv_key.clone());
    }

    let address = {
        let majority_count = ::majority_count(count);
        let keys = pub_keys.iter().map(|x| x.0);
        let redeem_script = btc::RedeemScriptBuilder::with_public_keys(keys)
            .quorum(majority_count as usize)
            .to_script()
            .unwrap();
        btc::Address::from_script(&redeem_script, network)
    };
    client.watch_address(&address, false).unwrap();
    let tx = client.send_to_address(&address, total_funds).unwrap();

    let genesis_cfg = AnchoringConfig::new_with_funding_tx(network, pub_keys, tx);
    for (idx, node_cfg) in node_cfgs.iter_mut().enumerate() {
        node_cfg
            .private_keys
            .insert(address.to_string(), priv_keys[idx].clone());
    }
    (genesis_cfg, node_cfgs)
}

/// Same as [`gen_anchoring_testnet_config_with_rng`](fn.gen_anchoring_testnet_config_with_rng.html)
/// but it uses default random number generator.
pub fn gen_anchoring_testnet_config(
    client: &BitcoinRelay,
    network: btc::Network,
    count: u8,
    total_funds: u64,
) -> (AnchoringConfig, Vec<AnchoringNodeConfig>) {
    let mut rng = thread_rng();
    gen_anchoring_testnet_config_with_rng(client, network, count, total_funds, &mut rng)
}
