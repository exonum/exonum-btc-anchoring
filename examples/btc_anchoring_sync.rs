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

use anyhow::{anyhow, bail};
use async_trait::async_trait;
use bitcoincore_rpc::{Auth as BitcoinRpcAuth, Client as BitcoinRpcClient};
use exonum::crypto::Hash;
use exonum_btc_anchoring::{
    api::{AnchoringChainLength, AnchoringProposalState, IndexQuery, PrivateApi},
    blockchain::SignInput,
    btc,
    config::Config as AnchoringConfig,
    sync::{AnchoringChainUpdateTask, ChainUpdateError, SyncWithBitcoinError, SyncWithBitcoinTask},
};
use serde::{de::DeserializeOwned, ser::Serialize};
use serde_derive::{Deserialize, Serialize};
use structopt::StructOpt;
use tokio::time::delay_for;

use std::{
    collections::HashMap,
    convert::TryFrom,
    fs::{self, File},
    io::prelude::*,
    path::{Path, PathBuf},
    time::Duration,
};

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

    async fn get<R>(&self, endpoint: &str) -> Result<R, reqwest::Error>
    where
        R: DeserializeOwned + Send + 'static,
    {
        self.client
            .get(&self.endpoint(endpoint))
            .send()
            .await?
            .json()
            .await
    }

    async fn get_query<Q, R>(&self, endpoint: &str, query: &Q) -> Result<R, reqwest::Error>
    where
        Q: Serialize,
        R: DeserializeOwned + Send + 'static,
    {
        self.client
            .get(&self.endpoint(endpoint))
            .query(query)
            .send()
            .await?
            .json()
            .await
    }

    async fn post<Q, R>(&self, endpoint: &str, body: &Q) -> Result<R, reqwest::Error>
    where
        Q: Serialize,
        R: DeserializeOwned + Send + 'static,
    {
        self.client
            .post(&self.endpoint(endpoint))
            .json(&body)
            .send()
            .await?
            .json()
            .await
    }
}

#[async_trait]
impl PrivateApi for ApiClient {
    type Error = reqwest::Error;

    async fn sign_input(&self, sign_input: SignInput) -> Result<Hash, Self::Error> {
        self.post("sign-input", &sign_input).await
    }

    async fn add_funds(&self, transaction: btc::Transaction) -> Result<Hash, Self::Error> {
        self.post("add-funds", &transaction).await
    }

    async fn anchoring_proposal(&self) -> Result<AnchoringProposalState, Self::Error> {
        self.get("anchoring-proposal").await
    }

    async fn config(&self) -> Result<AnchoringConfig, Self::Error> {
        self.get("config").await
    }

    async fn transaction_with_index(
        &self,
        index: u64,
    ) -> Result<Option<btc::Transaction>, Self::Error> {
        self.get_query("transaction", &IndexQuery { index }).await
    }

    async fn transactions_count(&self) -> Result<AnchoringChainLength, Self::Error> {
        self.get("transactions-count").await
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

/// Generates a new Bitcoin key pair and add them to the key pool of the specified
/// configuration file.
#[derive(Debug, StructOpt)]
struct GenerateKeypairCommand {
    /// Path to a sync utility configuration file.
    #[structopt(long, short = "c")]
    config: PathBuf,
}

#[derive(Debug, StructOpt)]
enum Commands {
    /// Generate initial configuration for the btc anchoring sync utility.
    GenerateConfig(GenerateConfigCommand),
    /// Run btc anchoring sync utility.
    Run(RunCommand),
    /// Generate a new Bitcoin key pair and add them to the key pool of the specified
    /// configuration file.
    GenerateKeypair(GenerateKeypairCommand),
}

#[derive(Debug, Serialize, Deserialize)]
struct SyncConfig {
    exonum_private_api: String,
    instance_name: String,
    #[serde(with = "flatten_keypairs")]
    bitcoin_key_pool: HashMap<btc::PublicKey, btc::PrivateKey>,
    bitcoin_rpc_config: Option<BitcoinRpcConfig>,
}

impl SyncConfig {
    /// Extracts Bitcoin network type from the one of Bitcoin private keys in this config.
    fn bitcoin_network(&self) -> Option<bitcoin::Network> {
        self.bitcoin_key_pool
            .values()
            .next()
            .map(|key| key.0.network)
    }

    fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut file = File::open(path)?;
        let mut toml = String::new();
        file.read_to_string(&mut toml)?;
        toml::de::from_str(&toml).map_err(From::from)
    }

    fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();

        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }

        let mut file = File::create(path)?;
        let value_toml = toml::Value::try_from(&self)?;
        file.write_all(value_toml.to_string().as_bytes())?;
        Ok(())
    }
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
    fn run(self) -> anyhow::Result<()> {
        let bitcoin_keypair = btc::gen_keypair(self.bitcoin_network);

        let bitcoin_rpc_config = self.bitcoin_rpc_config();
        let sync_config = SyncConfig {
            exonum_private_api: self.exonum_private_api,
            bitcoin_key_pool: std::iter::once(bitcoin_keypair.clone()).collect(),
            instance_name: self.instance_name,
            bitcoin_rpc_config,
        };

        sync_config.save(self.output)?;
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
    async fn run(self) -> anyhow::Result<()> {
        let sync_config = SyncConfig::load(self.config)?;
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
            match chain_updater.process().await {
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
                // For the work of anchoring you need to replenish anchoring wallet.
                Err(ChainUpdateError::NoInitialFunds) => {
                    let address = match chain_updater.anchoring_config().await {
                        Ok(config) => config.anchoring_address(),
                        Err(e) => {
                            log::error!("An error in the anchoring API client occurred. {}", e);
                            continue;
                        }
                    };

                    log::warn!(
                        "Initial funding transaction is absent, you should send some \
                         Bitcoins to the address {}",
                        address
                    );
                    log::warn!(
                        "And then confirm this transaction using the private \
                         `add-funds` API method."
                    )
                }
                // Stop execution if an internal error occurred.
                Err(ChainUpdateError::Internal(e)) => return Err(e),
            }

            if let Some(relay) = bitcoin_relay.as_ref() {
                match relay.process(latest_synced_tx_index).await {
                    Ok(index) => latest_synced_tx_index = index,

                    Err(SyncWithBitcoinError::Client(e)) => {
                        log::error!("An error in the anchoring API client occurred. {}", e)
                    }

                    Err(SyncWithBitcoinError::Relay(e)) => {
                        log::error!("An error in the Bitcoin relay occurred. {}", e)
                    }

                    Err(SyncWithBitcoinError::UnconfirmedFundingTransaction(id)) => bail!(
                        "Funding transaction with id {} is unconfirmed by Bitcoin network. \
                         This is a serious mistake that can break anchoring process.",
                        id
                    ),

                    // Stop execution if an internal error occurred.
                    Err(SyncWithBitcoinError::Internal(e)) => return Err(e),
                }
            }

            // Don't perform this actions too frequent to avoid DOS attack.
            delay_for(Duration::from_secs(5)).await
        }
    }
}

impl GenerateKeypairCommand {
    fn run(self) -> anyhow::Result<()> {
        let mut sync_config = SyncConfig::load(&self.config)?;

        let network = sync_config.bitcoin_network().ok_or_else(|| {
            anyhow!(
                "Unable to determine Bitcoin network type from config.\
                 Perhaps pool of keys in config is empty."
            )
        })?;
        let bitcoin_keypair = btc::gen_keypair(network);
        let bitcoin_pub_key = bitcoin_keypair.0;

        sync_config
            .bitcoin_key_pool
            .extend(std::iter::once(bitcoin_keypair));
        sync_config.save(self.config)?;
        // Print the received Bitcoin public key to use it in scripts.
        println!("{}", bitcoin_pub_key);
        Ok(())
    }
}

impl Commands {
    async fn run(self) -> anyhow::Result<()> {
        match self {
            Commands::GenerateConfig(cmd) => cmd.run(),
            Commands::GenerateKeypair(cmd) => cmd.run(),
            Commands::Run(cmd) => cmd.run().await,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    exonum::helpers::init_logger()?;
    Commands::from_args().run().await
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
