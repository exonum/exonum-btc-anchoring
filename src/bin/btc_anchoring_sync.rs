// Copyright 2019 The Exonum Team
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

use exonum::{
    crypto::{self, Hash},
    node::NodeConfig,
};
use exonum_btc_anchoring::{
    api::{AnchoringTransactionProposal, AsyncResult, IndexQuery, PrivateApi},
    blockchain::SignInput,
    btc,
    config::{AnchoringKeys, Config as AnchoringConfig},
    sync::AnchoringChainUpdater,
};
use exonum_cli::io::{load_config_file, save_config_file};
use futures::Future;
use serde::{de::DeserializeOwned, ser::Serialize};
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;
use tokio::timer::Delay;

use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

const EMPTY_QUERY: &() = &();

/// Client implementation for the API of the anchoring service instance.
#[derive(Debug, Clone)]
pub struct ApiClient {
    /// Complete prefix with the port and the anchoring instance name.
    prefix: String,
    /// Underlying HTTP client.
    client: reqwest::r#async::Client,
}

impl ApiClient {
    /// Create a new anchoring API relay with the specified host and name of instance.
    /// Hostname should be in form `{http|https}://{address}:{port}`.
    pub fn new(hostname: impl AsRef<str>, instance_name: impl AsRef<str>) -> Self {
        Self {
            prefix: format!(
                "{}/api/services/{}",
                hostname.as_ref(),
                instance_name.as_ref()
            ),
            client: reqwest::r#async::Client::new(),
        }
    }

    fn endpoint(&self, name: impl AsRef<str>) -> String {
        format!("{}/{}", self.prefix, name.as_ref())
    }

    fn get_json<Q, R>(&self, endpoint: &str, query: &Q) -> AsyncResult<R, String>
    where
        Q: Serialize,
        R: DeserializeOwned + Send + 'static,
    {
        Box::new(
            self.client
                .get(&self.endpoint(endpoint))
                .query(query)
                .send()
                .and_then(|mut request| request.json())
                .map_err(|e| e.to_string()),
        )
    }

    fn post_json<Q, R>(&self, endpoint: &str, body: &Q) -> AsyncResult<R, String>
    where
        Q: Serialize,
        R: DeserializeOwned + Send + 'static,
    {
        Box::new(
            self.client
                .post(&self.endpoint(endpoint))
                .json(&body)
                .send()
                .and_then(|mut request| request.json())
                .map_err(|e| e.to_string()),
        )
    }
}

impl PrivateApi for ApiClient {
    type Error = String;

    fn sign_input(&self, sign_input: SignInput) -> AsyncResult<Hash, Self::Error> {
        self.post_json("sign-input", &sign_input)
    }

    fn anchoring_proposal(&self) -> AsyncResult<Option<AnchoringTransactionProposal>, Self::Error> {
        self.get_json("anchoring-interval", EMPTY_QUERY)
    }

    fn config(&self) -> AsyncResult<AnchoringConfig, Self::Error> {
        self.get_json("config", EMPTY_QUERY)
    }

    fn transaction_with_index(&self, index: u64) -> Result<Option<btc::Transaction>, Self::Error> {
        self.get_json("transaction", &IndexQuery { index }).wait()
    }

    fn transactions_count(&self) -> Result<u64, Self::Error> {
        self.get_json("transactions-count", EMPTY_QUERY).wait()
    }
}

/// Generate initial configuration for the btc anchoring sync utility.
#[derive(Debug, StructOpt)]
struct GenerateConfig {
    /// Path to a node configuration file.
    #[structopt(long, short = "c")]
    node_config: PathBuf,
    /// Path to a sync utility configuration file which will be created after
    /// running this command.
    #[structopt(long, short = "o")]
    output: PathBuf,
    /// Bitcoin network type.
    #[structopt(long, short = "n")]
    bitcoin_network: bitcoin::Network,
    /// Anchoring instance name.
    #[structopt(long, short = "i")]
    instance_name: String,
}

#[derive(Debug, StructOpt)]
struct Run {
    /// Path to a sync utility configuration file.
    #[structopt(long, short = "c")]
    config: PathBuf,
}

/// Helper command to compute bitcoin address for the specified bitcoin keys and network.
#[derive(Debug, StructOpt)]
struct AnchoringAddress {
    /// Bitcoin network type.
    #[structopt(long, short = "n")]
    bitcoin_network: bitcoin::Network,
    /// Anchoring keys.
    anchoring_keys: Vec<btc::PublicKey>,
}

#[derive(Debug, StructOpt)]
enum Commands {
    /// Generate initial configuration for the btc anchoring sync utility.
    GenerateConfig(GenerateConfig),
    /// Run btc anchoring sync utility.
    Run(Run),
    /// Helper command to compute bitcoin address for the specified bitcoin keys and network.
    AnchoringAddress(AnchoringAddress),
}

#[derive(Debug, Serialize, Deserialize)]
struct SyncConfig {
    private_api_address: String,
    instance_name: String,
    #[serde(with = "flatten_keypairs")]
    bitcoin_key_pool: HashMap<btc::PublicKey, btc::PrivateKey>,
}

fn socket_to_http_address(addr: std::net::SocketAddr) -> String {
    format!("http://{}", addr)
}

impl GenerateConfig {
    fn run(self) -> Result<(), failure::Error> {
        let bitcoin_keypair = btc::gen_keypair(self.bitcoin_network);
        println!("{}", bitcoin_keypair.0);

        let node_config: NodeConfig = load_config_file(self.node_config)?;
        let sync_config = SyncConfig {
            private_api_address: node_config
                .api
                .private_api_address
                .map(socket_to_http_address)
                .ok_or_else(|| {
                    failure::format_err!("Public API address should be exist in the node config")
                })?,
            bitcoin_key_pool: std::iter::once(bitcoin_keypair.clone()).collect(),
            instance_name: self.instance_name,
        };

        save_config_file(&sync_config, self.output)?;
        log::info!("Generated initial configuration for the btc anchoring sync util.");
        log::trace!(
            "Available Bitcoin keys in key pool: {:?}",
            sync_config.bitcoin_key_pool
        );
        Ok(())
    }
}

impl Run {
    fn run(self) -> Result<(), failure::Error> {
        let sync_config: SyncConfig = load_config_file(self.config)?;

        let anchoring_chain_update_task = {
            let client = ApiClient::new(sync_config.private_api_address, sync_config.instance_name);
            let updater = AnchoringChainUpdater::new(sync_config.bitcoin_key_pool, client);
            futures::future::loop_fn(updater, |updater| {
                updater
                    .clone()
                    .process()
                    .and_then(|_| {
                        let when = Instant::now() + Duration::from_secs(1);
                        Delay::new(when).map_err(|e| e.to_string())
                    })
                    .and_then(|_| Ok(futures::future::Loop::Continue(updater)))
            })
        };

        tokio::run(anchoring_chain_update_task.map_err(|e| log::error!("{}", e)));
        Ok(())
    }
}

impl AnchoringAddress {
    fn run(self) -> Result<(), failure::Error> {
        let fake_config = AnchoringConfig {
            anchoring_keys: self
                .anchoring_keys
                .into_iter()
                .map(|bitcoin_key| AnchoringKeys {
                    service_key: crypto::gen_keypair().0,
                    bitcoin_key,
                })
                .collect(),
            network: self.bitcoin_network,
            ..AnchoringConfig::default()
        };
        println!("{}", fake_config.anchoring_address());
        Ok(())
    }
}

impl Commands {
    fn run(self) -> Result<(), failure::Error> {
        match self {
            Commands::GenerateConfig(cmd) => cmd.run(),
            Commands::Run(cmd) => cmd.run(),
            Commands::AnchoringAddress(cmd) => cmd.run(),
        }
    }
}

fn main() -> Result<(), failure::Error> {
    exonum::helpers::init_logger()?;
    Commands::from_args().run()
}

mod flatten_keypairs {
    use crate::btc::{PrivateKey, PublicKey};

    use serde_derive::{Deserialize, Serialize};

    use std::collections::HashMap;

    /// The structure for storing the bitcoin keypair.
    /// It is required for reading data from the .toml file into memory.
    #[derive(Deserialize, Serialize)]
    struct BitcoinKeypair {
        /// Bitcoin public key.
        public_key: PublicKey,
        /// Corresponding private key.
        private_key: PrivateKey,
    }

    pub fn serialize<S>(
        keys: &HashMap<PublicKey, PrivateKey>,
        ser: S,
    ) -> ::std::result::Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        use serde::Serialize;

        let keypairs = keys
            .iter()
            .map(|(&public_key, private_key)| BitcoinKeypair {
                public_key,
                private_key: private_key.clone(),
            })
            .collect::<Vec<_>>();
        keypairs.serialize(ser)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<PublicKey, PrivateKey>, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        use serde::Deserialize;
        Vec::<BitcoinKeypair>::deserialize(deserializer).map(|keypairs| {
            keypairs
                .into_iter()
                .map(|keypair| (keypair.public_key, keypair.private_key))
                .collect()
        })
    }
}
