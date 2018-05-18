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
use exonum::helpers::fabric::{self, keys, Argument, CommandExtension, CommandName, Context,
                              ServiceFactory};
use exonum::node::NodeConfig;

use bitcoin::network::constants::Network;
use failure;
use serde::Serialize;
use serde::de::DeserializeOwned;
use toml;

use std::collections::BTreeMap;
use std::io;
use std::str::FromStr;

use self::args::{NamedArgumentOptional, NamedArgumentRequired, TypedArgument};
use btc::gen_keypair;
use rpc::BitcoinRpcConfig;

mod args;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum BtcNetwork {
    Bitcoin,
    Testnet,
}

impl From<BtcNetwork> for Network {
    fn from(n: BtcNetwork) -> Network {
        match n {
            BtcNetwork::Bitcoin => Network::Bitcoin,
            BtcNetwork::Testnet => Network::Testnet,
        }
    }
}

impl FromStr for BtcNetwork {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bitcoin" => Ok(BtcNetwork::Bitcoin),
            "testnet" => Ok(BtcNetwork::Testnet),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid network type {}", s),
            )),
        }
    }
}

const BTC_ANCHORING_NETWORK: NamedArgumentRequired<BtcNetwork> = NamedArgumentRequired {
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

struct GenerateCommonConfig;

impl CommandExtension for GenerateCommonConfig {
    fn args(&self) -> Vec<Argument> {
        vec![
            BTC_ANCHORING_NETWORK.to_argument(),
            BTC_ANCHORING_INTERVAL.to_argument(),
            BTC_ANCHORING_FEE.to_argument(),
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
            ].into_iter(),
        );

        context.set(keys::SERVICES_CONFIG, values);
        Ok(context)
    }
}

struct GenerateNodeConfig;

const BTC_ANCHORING_RPC_HOST: NamedArgumentOptional<String> = NamedArgumentOptional {
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
        let mut services_secret_configs: BTreeMap<String, toml::Value> = context
            .get(keys::SERVICES_SECRET_CONFIGS)
            .unwrap_or_default();
        let mut services_public_configs: BTreeMap<String, toml::Value> = context
            .get(keys::SERVICES_PUBLIC_CONFIGS)
            .unwrap_or_default();
        let common_configs = context.get(keys::COMMON_CONFIG).unwrap_or_default();

        // Inserts bitcoin keypair.
        let network = BTC_ANCHORING_NETWORK.output_value(&common_configs.services_config)?;
        let keypair = gen_keypair(network.into());

        services_public_configs.insert(
            "btc_anchoring_public_key".to_owned(),
            toml::Value::try_from(keypair.0)?,
        );
        services_secret_configs.extend(
            vec![
                (
                    "btc_anchoring_public_key".to_owned(),
                    toml::Value::try_from(keypair.0)?,
                ),
                (
                    "btc_anchoring_secret_key".to_owned(),
                    toml::Value::try_from(keypair.1)?,
                ),
            ].into_iter(),
        );

        // Inserts rpc host.
        if let Some(host) = BTC_ANCHORING_RPC_HOST.input_value(&context)? {
            let rpc_config = BitcoinRpcConfig {
                host,
                username: BTC_ANCHORING_RPC_USERNAME.input_value(&context)?,
                password: BTC_ANCHORING_RPC_PASSWORD.input_value(&context)?,
            };
            services_secret_configs.insert(
                "btc_anchoring_rpc_config".to_owned(),
                toml::Value::try_from(rpc_config)?,
            );
        };
        // Push changes to the context.
        context.set(keys::SERVICES_SECRET_CONFIGS, services_secret_configs);
        context.set(keys::SERVICES_PUBLIC_CONFIGS, services_public_configs);
        Ok(context)
    }
}

/// A BTC oracle service creator for the `NodeBuilder`.
#[derive(Debug, Copy, Clone)]
pub struct BtcAnchoringFactory;

impl ServiceFactory for BtcAnchoringFactory {
    fn command(&mut self, command: CommandName) -> Option<Box<CommandExtension>> {
        Some(match command {
            v if v == fabric::GenerateCommonConfig::name() => Box::new(GenerateCommonConfig),
            v if v == fabric::GenerateNodeConfig::name() => Box::new(GenerateNodeConfig),
            // v if v == fabric::Finalize::name() => Box::new(Finalize),
            _ => return None,
        })
    }

    fn make_service(&mut self, context: &Context) -> Box<Service> {
        unimplemented!();
        // let node_config = context.get(keys::NODE_CONFIG).unwrap();
        // let btc_oracle_config: BtcOracleConfig = node_config
        //     .services_configs
        //     .get(BTC_ORACLE_SERVICE_NAME)
        //     .map(|x| x.clone().try_into().unwrap())
        //     .unwrap_or_default();

        // Box::new(BtcOracle::new(btc_oracle_config))
    }
}
