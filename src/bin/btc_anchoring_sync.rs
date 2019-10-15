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

use bitcoincore_rpc::{Auth as BitcoinRpcAuth, Client as BitcoinRpcClient};
use exonum::crypto::{self, Hash};
use exonum_btc_anchoring::{
    api::{AnchoringChainLength, AnchoringProposalState, IndexQuery, PrivateApi},
    blockchain::SignInput,
    btc,
    config::{AnchoringKeys, Config as AnchoringConfig},
    sync::{AnchoringChainUpdateTask, ChainUpdateError, SyncWithBitcoinError, SyncWithBitcoinTask},
};
use exonum_cli::io::{load_config_file, save_config_file};
use futures::{Future, IntoFuture};
use serde::{de::DeserializeOwned, ser::Serialize};
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;

use std::{collections::HashMap, convert::TryFrom, path::PathBuf, time::Duration};

/// Client implementation for the API of the anchoring service instance.
#[derive(Debug, Clone)]
pub struct ApiClient {
    /// Complete prefix with the port and the anchoring instance name.
    prefix: String,
    /// Underlying HTTP client.
    client: reqwest::Client,
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
            client: reqwest::Client::new(),
        }
    }

    fn endpoint(&self, name: impl AsRef<str>) -> String {
        format!("{}/{}", self.prefix, name.as_ref())
    }

    fn get<R>(&self, endpoint: &str) -> Result<R, String>
    where
        R: DeserializeOwned + Send + 'static,
    {
        self.client
            .get(&self.endpoint(endpoint))
            .send()
            .and_then(|mut request| request.json())
            .map_err(|e| e.to_string())
    }

    fn get_query<Q, R>(&self, endpoint: &str, query: &Q) -> Result<R, String>
    where
        Q: Serialize,
        R: DeserializeOwned + Send + 'static,
    {
        self.client
            .get(&self.endpoint(endpoint))
            .query(query)
            .send()
            .and_then(|mut request| request.json())
            .map_err(|e| e.to_string())
    }

    fn post<Q, R>(&self, endpoint: &str, body: &Q) -> Result<R, String>
    where
        Q: Serialize,
        R: DeserializeOwned + Send + 'static,
    {
        self.client
            .post(&self.endpoint(endpoint))
            .json(&body)
            .send()
            .and_then(|mut request| request.json())
            .map_err(|e| e.to_string())
    }
}

impl PrivateApi for ApiClient {
    type Error = String;

    fn sign_input(
        &self,
        sign_input: SignInput,
    ) -> Box<dyn Future<Item = Hash, Error = Self::Error>> {
        Box::new(self.post("sign-input", &sign_input).into_future())
    }

    fn add_funds(
        &self,
        transaction: btc::Transaction,
    ) -> Box<dyn Future<Item = Hash, Error = Self::Error>> {
        Box::new(self.post("add-funds", &transaction).into_future())
    }

    fn anchoring_proposal(&self) -> Result<AnchoringProposalState, Self::Error> {
        self.get("anchoring-proposal")
    }

    fn config(&self) -> Result<AnchoringConfig, Self::Error> {
        self.get("config")
    }

    fn transaction_with_index(&self, index: u64) -> Result<Option<btc::Transaction>, Self::Error> {
        self.get_query("transaction", &IndexQuery { index })
    }

    fn transactions_count(&self) -> Result<AnchoringChainLength, Self::Error> {
        self.get("transactions-count")
    }
}

/// Generate initial configuration for the btc anchoring sync utility.
#[derive(Debug, StructOpt)]
struct GenerateConfigCommand {
    /// Path to a sync utility configuration file which will be created after
    /// running this command.
    #[structopt(long, short = "o", default_value = "btc_anchoring_sync.toml")]
    output: PathBuf,
    /// Anchoring node private API url address.
    #[structopt(long, short = "e", default_value = "http://localhost:8081")]
    exonum_private_api: String,
    /// Bitcoin network type.
    #[structopt(long, short = "n", default_value = "testnet")]
    bitcoin_network: bitcoin::Network,
    /// Name of the anchoring service instance.
    #[structopt(long, short = "i", default_value = "anchoring")]
    instance_name: String,
    /// Bitcoin RPC url.
    #[structopt(long)]
    bitcoin_rpc_host: Option<String>,
    /// Bitcoin RPC username.
    #[structopt(long)]
    bitcoin_rpc_user: Option<String>,
    /// Bitcoin RPC password.
    #[structopt(long)]
    bitcoin_rpc_password: Option<String>,
}

#[derive(Debug, StructOpt)]
struct RunCommand {
    /// Path to a sync utility configuration file.
    #[structopt(long, short = "c")]
    config: PathBuf,
}

/// Helper command to compute bitcoin address for the specified bitcoin keys and network.
#[derive(Debug, StructOpt)]
struct AnchoringAddressCommand {
    /// Bitcoin network type.
    #[structopt(long, short = "n")]
    bitcoin_network: bitcoin::Network,
    /// Anchoring keys.
    anchoring_keys: Vec<btc::PublicKey>,
}

#[derive(Debug, StructOpt)]
enum Commands {
    /// Generate initial configuration for the btc anchoring sync utility.
    GenerateConfig(GenerateConfigCommand),
    /// Run btc anchoring sync utility.
    Run(RunCommand),
    /// Helper command to compute bitcoin address for the specified bitcoin keys and network.
    AnchoringAddress(AnchoringAddressCommand),
}

#[derive(Debug, Serialize, Deserialize)]
struct SyncConfig {
    exonum_private_api: String,
    instance_name: String,
    #[serde(with = "flatten_keypairs")]
    bitcoin_key_pool: HashMap<btc::PublicKey, btc::PrivateKey>,
    bitcoin_rpc_config: Option<BitcoinRpcConfig>,
}

/// `Bitcoind` rpc configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
struct BitcoinRpcConfig {
    /// Bitcoin RPC url.
    host: String,
    /// Bitcoin RPC username.
    user: Option<String>,
    /// Bitcoin RPC password.
    password: Option<String>,
}

impl TryFrom<BitcoinRpcConfig> for BitcoinRpcClient {
    type Error = bitcoincore_rpc::Error;

    fn try_from(value: BitcoinRpcConfig) -> Result<Self, Self::Error> {
        let auth = BitcoinRpcAuth::UserPass(
            value.user.unwrap_or_default(),
            value.password.unwrap_or_default(),
        );
        Self::new(value.host, auth)
    }
}

impl GenerateConfigCommand {
    fn run(self) -> Result<(), failure::Error> {
        let bitcoin_keypair = btc::gen_keypair(self.bitcoin_network);

        let bitcoin_rpc_config = self.bitcoin_rpc_config();
        let sync_config = SyncConfig {
            exonum_private_api: self.exonum_private_api,
            bitcoin_key_pool: std::iter::once(bitcoin_keypair.clone()).collect(),
            instance_name: self.instance_name,
            bitcoin_rpc_config,
        };

        save_config_file(&sync_config, self.output)?;
        log::info!("Generated initial configuration for the btc anchoring sync util.");
        log::trace!(
            "Available Bitcoin keys in key pool: {:?}",
            sync_config.bitcoin_key_pool
        );
        // Print the received Bitcoin public key to use it in scripts.
        println!("{}", bitcoin_keypair.0);
        Ok(())
    }

    fn bitcoin_rpc_config(&self) -> Option<BitcoinRpcConfig> {
        self.bitcoin_rpc_host.clone().map(|host| BitcoinRpcConfig {
            host,
            user: self.bitcoin_rpc_user.clone(),
            password: self.bitcoin_rpc_password.clone(),
        })
    }
}

impl RunCommand {
    fn run(self) -> Result<(), failure::Error> {
        let sync_config: SyncConfig = load_config_file(self.config)?;
        // TODO rewrite on top of tokio or runtime crate [ECR-3222]
        let client = ApiClient::new(sync_config.exonum_private_api, sync_config.instance_name);
        let chain_updater =
            AnchoringChainUpdateTask::new(sync_config.bitcoin_key_pool, client.clone());
        let bitcoin_relay = sync_config
            .bitcoin_rpc_config
            .map(BitcoinRpcClient::try_from)
            .transpose()?
            .map(|relay| SyncWithBitcoinTask::new(relay, client.clone()));

        let mut latest_synced_tx_index: Option<u64> = None;
        loop {
            match chain_updater.process() {
                Ok(_) => {}
                // Client problems most often occurs due to network problems.
                Err(ChainUpdateError::Client(e)) => {
                    log::error!("An error in the anchoring API client occurred. {}", e)
                }
                // Sometimes Bitcoin end in the anchoring wallet.
                Err(ChainUpdateError::InsufficientFunds { total_fee, balance }) => log::warn!(
                    "Insufficient funds to construct a new anchoring transaction, \
                     total fee is {}, total balance is {}",
                    total_fee,
                    balance
                ),
                // For the work of anchoring you need to  replenish anchoring wallet.
                Err(ChainUpdateError::NoInitialFunds) => {
                    let address = match chain_updater.anchoring_config() {
                        Ok(config) => config.anchoring_address(),
                        Err(e) => {
                            log::error!("An error in the anchoring API client occurred. {}", e);
                            continue;
                        }
                    };

                    log::warn!(
                        "Initial funding transaction is absent, you should send some \
                         Bitcoins to the address {}, And then confirm this transaction \
                         using the private `add-funds` API method.",
                        address
                    )
                }
                // Stop execution if an internal error occurred.
                Err(ChainUpdateError::Internal(e)) => return Err(e),
            }

            if let Some(relay) = bitcoin_relay.as_ref() {
                match relay.process(latest_synced_tx_index) {
                    Ok(index) => latest_synced_tx_index = index,

                    Err(SyncWithBitcoinError::Client(e)) => {
                        log::error!("An error in the anchoring API client occurred. {}", e)
                    }

                    Err(SyncWithBitcoinError::Relay(e)) => {
                        log::error!("An error in the Bitcoin relay occurred. {}", e)
                    }

                    Err(SyncWithBitcoinError::UnconfirmedFundingTransaction(id)) => failure::bail!(
                        "Funding transaction with id {} is unconfirmed by Bitcoin network. \
                         This is a serious mistake that can break anchoring process.",
                        id
                    ),

                    // Stop execution if an internal error occurred.
                    Err(SyncWithBitcoinError::Internal(e)) => return Err(e),
                }
            }
            // Don't perform this actions too frequent to avoid DOS attack.
            std::thread::sleep(Duration::from_secs(5));
        }
    }
}

impl AnchoringAddressCommand {
    fn run(self) -> Result<(), failure::Error> {
        // Create fake config to reuse its `anchoring_address` method.
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

    pub fn serialize<S>(keys: &HashMap<PublicKey, PrivateKey>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
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
        D: serde::Deserializer<'de>,
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
