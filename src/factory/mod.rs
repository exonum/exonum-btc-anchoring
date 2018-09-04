// Copyright 2018 The Exonum Team
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

use exonum::blockchain::Service;

use exonum::helpers::fabric::{
    self, keys, Argument, Command, CommandExtension, CommandName, Context, ServiceFactory,
};
use exonum::node::NodeConfig;

use bitcoin::network::constants::Network;
use failure;
use toml;
use {BtcAnchoringService, BTC_ANCHORING_SERVICE_NAME};

use std::collections::{BTreeMap, HashMap};

use self::args::{Hash, NamedArgumentOptional, NamedArgumentRequired, TypedArgument};
use btc::{gen_keypair, Privkey, PublicKey};
use config::{Config, GlobalConfig, LocalConfig};
use rpc::{BitcoinRpcClient, BitcoinRpcConfig, BtcRelay};

use std::sync::{Arc, RwLock};
mod args;

const BTC_ANCHORING_NETWORK: NamedArgumentRequired<Network> = NamedArgumentRequired {
    name: "btc_anchoring_network",
    short_key: Some("n"),
    long_key: "btc-anchoring-network",
    help: "BTC Anchoring network type.",
    default: None,
};

const BTC_ANCHORING_INTERVAL: NamedArgumentRequired<u64> = NamedArgumentRequired {
    name: "btc_anchoring_interval",
    short_key: None,
    long_key: "btc-anchoring-interval",
    help: "Interval in blocks between anchored blocks.",
    default: Some(5000),
};

const BTC_ANCHORING_FEE: NamedArgumentRequired<u64> = NamedArgumentRequired {
    name: "btc_anchoring_fee",
    short_key: None,
    long_key: "btc-anchoring-fee",
    help: "Transaction fee per byte in satoshi that anchoring nodes should use.",
    default: Some(100),
};

const BTC_ANCHORING_UTXO_CONFIRMATIONS: NamedArgumentRequired<u64> = NamedArgumentRequired {
    name: "btc_anchoring_utxo_confirmations",
    short_key: None,
    long_key: "btc-anchoring-utxo-confirmations",
    help: "The minimum number of confirmations for funding transactions.",
    default: Some(2),
};

struct GenerateCommonConfig;

impl CommandExtension for GenerateCommonConfig {
    fn args(&self) -> Vec<Argument> {
        vec![
            BTC_ANCHORING_NETWORK.to_argument(),
            BTC_ANCHORING_INTERVAL.to_argument(),
            BTC_ANCHORING_FEE.to_argument(),
            BTC_ANCHORING_UTXO_CONFIRMATIONS.to_argument(),
        ]
    }

    fn execute(&self, mut context: Context) -> Result<Context, failure::Error> {
        let mut values: BTreeMap<String, toml::Value> = context
            .get(keys::SERVICES_CONFIG)
            .expect("Expected services_config in context.");

        values.extend(
            vec![
                BTC_ANCHORING_NETWORK.input_value_to_toml(&context)?,
                BTC_ANCHORING_INTERVAL.input_value_to_toml(&context)?,
                BTC_ANCHORING_FEE.input_value_to_toml(&context)?,
                BTC_ANCHORING_UTXO_CONFIRMATIONS.input_value_to_toml(&context)?,
            ].into_iter(),
        );

        context.set(keys::SERVICES_CONFIG, values);
        Ok(context)
    }
}

struct GenerateNodeConfig;

const BTC_ANCHORING_RPC_HOST: NamedArgumentRequired<String> = NamedArgumentRequired {
    name: "btc_anchoring_rpc_host",
    short_key: None,
    long_key: "btc-anchoring-rpc-host",
    help: "Host of bitcoind.",
    default: None,
};

const BTC_ANCHORING_RPC_USERNAME: NamedArgumentOptional<String> = NamedArgumentOptional {
    name: "btc_anchoring_rpc_user",
    short_key: None,
    long_key: "btc-anchoring-rpc-user",
    help: "User to login into bitcoind.",
    default: None,
};

const BTC_ANCHORING_RPC_PASSWORD: NamedArgumentOptional<String> = NamedArgumentOptional {
    name: "btc_anchoring_rpc_password",
    short_key: None,
    long_key: "btc-anchoring-rpc-password",
    help: "Password to login into bitcoind.",
    default: None,
};

impl CommandExtension for GenerateNodeConfig {
    fn args(&self) -> Vec<Argument> {
        vec![
            BTC_ANCHORING_RPC_HOST.to_argument(),
            BTC_ANCHORING_RPC_USERNAME.to_argument(),
            BTC_ANCHORING_RPC_PASSWORD.to_argument(),
        ]
    }

    fn execute(&self, mut context: Context) -> Result<Context, failure::Error> {
        let mut services_secret_config: BTreeMap<String, toml::Value> = context
            .get(keys::SERVICES_SECRET_CONFIGS)
            .unwrap_or_default();
        let mut services_public_config: BTreeMap<String, toml::Value> = context
            .get(keys::SERVICES_PUBLIC_CONFIGS)
            .unwrap_or_default();
        let common_config = context.get(keys::COMMON_CONFIG).unwrap_or_default();

        // Inserts bitcoin keypair.
        let network = BTC_ANCHORING_NETWORK.output_value(&common_config.services_config)?;
        let keypair = gen_keypair(network);

        services_public_config.insert(
            "btc_anchoring_public_key".to_owned(),
            toml::Value::try_from(keypair.0)?,
        );
        services_secret_config.extend(
            vec![
                (
                    "btc_anchoring_public_key".to_owned(),
                    toml::Value::try_from(keypair.0)?,
                ),
                (
                    "btc_anchoring_private_key".to_owned(),
                    toml::Value::try_from(keypair.1)?,
                ),
            ].into_iter(),
        );

        // Inserts rpc host.
        let host = BTC_ANCHORING_RPC_HOST.input_value(&context)?;
        let rpc_config = BitcoinRpcConfig {
            host,
            username: BTC_ANCHORING_RPC_USERNAME.input_value(&context)?,
            password: BTC_ANCHORING_RPC_PASSWORD.input_value(&context)?,
        };
        services_secret_config.insert(
            "btc_anchoring_rpc_config".to_owned(),
            toml::Value::try_from(rpc_config)?,
        );
        // Push changes to the context.
        context.set(keys::SERVICES_SECRET_CONFIGS, services_secret_config);
        context.set(keys::SERVICES_PUBLIC_CONFIGS, services_public_config);
        Ok(context)
    }
}

struct Finalize;

const BTC_ANCHORING_CREATE_FUNDING_TX: NamedArgumentOptional<u64> = NamedArgumentOptional {
    name: "btc_anchoring_create_funding_tx",
    short_key: None,
    long_key: "btc-anchoring-create-funding-tx",
    help: "Create initial funding tx with given amount in satoshis",
    default: None,
};

const BTC_ANCHORING_FUNDING_TXID: NamedArgumentOptional<Hash> = NamedArgumentOptional {
    name: "btc_anchoring_funding_txid",
    short_key: None,
    long_key: "btc-anchoring-funding-txid",
    help: "Txid of the initial funding tx",
    default: None,
};

impl CommandExtension for Finalize {
    fn args(&self) -> Vec<Argument> {
        vec![
            BTC_ANCHORING_CREATE_FUNDING_TX.to_argument(),
            BTC_ANCHORING_FUNDING_TXID.to_argument(),
        ]
    }

    fn execute(&self, mut context: Context) -> Result<Context, failure::Error> {
        let mut node_config: NodeConfig = context.get(keys::NODE_CONFIG)?;
        let public_config_list = context.get(keys::PUBLIC_CONFIG_LIST)?;
        let services_secret_config: BTreeMap<String, toml::Value> = context
            .get(keys::SERVICES_SECRET_CONFIGS)
            .unwrap_or_default();
        let common_config = context.get(keys::COMMON_CONFIG)?;

        // Common part.
        let network = BTC_ANCHORING_NETWORK.output_value(&common_config.services_config)?;
        let interval = BTC_ANCHORING_INTERVAL.output_value(&common_config.services_config)?;
        let fee = BTC_ANCHORING_FEE.output_value(&common_config.services_config)?;
        let confirmations =
            BTC_ANCHORING_UTXO_CONFIRMATIONS.output_value(&common_config.services_config)?;

        // Private part.
        let private_key: Privkey = services_secret_config
            .get("btc_anchoring_private_key")
            .ok_or_else(|| format_err!("BTC private key not found"))?
            .clone()
            .try_into()?;
        let rpc_config: BitcoinRpcConfig = services_secret_config
            .get("btc_anchoring_rpc_config")
            .ok_or_else(|| format_err!("Bitcoin RPC configuration not found"))?
            .clone()
            .try_into()?;

        // Finalize part.
        let funding_tx_amount = BTC_ANCHORING_CREATE_FUNDING_TX.input_value(&context)?;
        let funding_txid = BTC_ANCHORING_FUNDING_TXID
            .input_value(&context)?
            .map(|x| x.0);

        // Gets anchoring public keys.
        let public_keys = {
            let mut public_keys = Vec::new();
            for public_config in public_config_list {
                let public_key: PublicKey = public_config
                    .services_public_configs()
                    .get("btc_anchoring_public_key")
                    .ok_or_else(|| format_err!("BTC public key not found"))?
                    .clone()
                    .try_into()?;
                public_keys.push(public_key);
            }
            public_keys
        };

        // Creates global config.
        let mut global_config = GlobalConfig::new(network, public_keys)?;
        // Generates initial funding transaction.
        let relay = BitcoinRpcClient::from(rpc_config.clone());

        let addr = global_config.anchoring_address();

        let funding_tx = if let Some(funding_txid) = funding_txid {
            let info = relay.transaction_info(&funding_txid)?.ok_or_else(|| {
                format_err!(
                    "Unable to find transaction with the given id {}",
                    funding_txid.to_hex()
                )
            })?;
            ensure!(
                info.confirmations >= confirmations,
                "Not enough confirmations to use funding transaction, actual {}, expected {}",
                info.confirmations,
                confirmations
            );
            info.content
        } else {
            let satoshis = funding_tx_amount
                .ok_or_else(|| format_err!("Expected `btc_anchoring_create_funding_tx` value"))?;
            let transaction = relay.send_to_address(&addr.0, satoshis)?;
            println!("{}", transaction.id().to_hex());
            transaction
        };

        info!("BTC anchoring address is {}", addr);

        global_config.funding_transaction = Some(funding_tx);
        global_config.anchoring_interval = interval;
        global_config.transaction_fee = fee;

        // Creates local config.
        let mut private_keys = HashMap::new();
        private_keys.insert(addr, private_key);

        let local_config = LocalConfig {
            rpc: Some(rpc_config),
            private_keys,
        };

        // Writes complete config to node_config
        let config = Config {
            local: local_config,
            global: global_config,
        };
        node_config.services_configs.insert(
            BTC_ANCHORING_SERVICE_NAME.to_owned(),
            toml::Value::try_from(config)?,
        );
        context.set(keys::NODE_CONFIG, node_config);
        Ok(context)
    }
}

/// A BTC oracle service creator for the `NodeBuilder`.
#[derive(Debug, Copy, Clone)]
pub struct BtcAnchoringFactory;

impl ServiceFactory for BtcAnchoringFactory {
    fn service_name(&self) -> &str {
        "btc_anchoring"
    }

    fn command(&mut self, command: CommandName) -> Option<Box<dyn CommandExtension>> {
        Some(match command {
            v if v == fabric::GenerateCommonConfig.name() => Box::new(GenerateCommonConfig),
            v if v == fabric::GenerateNodeConfig.name() => Box::new(GenerateNodeConfig),
            v if v == fabric::Finalize.name() => Box::new(Finalize),
            _ => return None,
        })
    }

    fn make_service(&mut self, context: &Context) -> Box<dyn Service> {
        let node_config = context.get(keys::NODE_CONFIG).unwrap();
        let btc_anchoring_config: Config = node_config
            .services_configs
            .get(BTC_ANCHORING_SERVICE_NAME)
            .expect("BTC anchoring config not found")
            .clone()
            .try_into()
            .unwrap();

        let btc_relay = btc_anchoring_config
            .local
            .rpc
            .map(BitcoinRpcClient::from)
            .map(Box::<dyn BtcRelay>::from);
        let service = BtcAnchoringService::new(
            btc_anchoring_config.global,
            Arc::new(RwLock::new(btc_anchoring_config.local.private_keys)),
            btc_relay,
        );
        Box::new(service)
    }
}
