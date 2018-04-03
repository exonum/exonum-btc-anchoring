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

//! Basic clap factory implementation.
//! This module collect all basic `CommandExtension` that
//! we can use in `anchoring` bootstrapping process.
//!
use std::collections::BTreeMap;
use std::str::FromStr;

use bitcoin::network::constants::Network;
use bitcoin::util::base58::ToBase58;
use failure;
use toml::Value;

use exonum::helpers::fabric::{keys, Argument, CommandExtension, CommandName, Context,
                              ServiceFactory};
use exonum::blockchain::Service;
use exonum::node::NodeConfig;
use exonum::encoding::serialize::FromHex;

use service::AnchoringService;
use super::{gen_btc_keypair, AnchoringConfig, AnchoringNodeConfig, AnchoringRpcConfig};
use details::btc::{self, PrivateKey, PublicKey};
use details::rpc::{BitcoinRelay, RpcClient};
use bitcoin::util::base58::FromBase58;
use observer::AnchoringObserverConfig;

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Anchoring configuration that should be saved into the file
pub struct AnchoringServiceConfig {
    /// `AnchoringConfig` is a common for all nodes part.
    pub genesis: AnchoringConfig,
    /// `AnchoringNodeConfig` is a unique for each node.
    pub node: AnchoringNodeConfig,
}

struct GenerateNodeConfig;

impl CommandExtension for GenerateNodeConfig {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named(
                "ANCHORING_RPC_HOST",
                true,
                "Host of bitcoind.",
                None,
                "anchoring-host",
                false,
            ),
            Argument::new_named(
                "ANCHORING_RPC_USER",
                false,
                "User to login into bitcoind.",
                None,
                "anchoring-user",
                false,
            ),
            Argument::new_named(
                "ANCHORING_RPC_PASSWD",
                false,
                "Password to login into bitcoind.",
                None,
                "anchoring-password",
                false,
            ),
            Argument::new_named(
                "ANCHORING_OBSERVER_CHECK_INTERVAL",
                false,
                "This option enables anchoring chain observer with the given check interval \
                 (in milliseconds).",
                None,
                "anchoring-observer-check-interval",
                false,
            ),
        ]
    }

    fn execute(&self, mut context: Context) -> Result<Context, failure::Error> {
        let host = context
            .arg("ANCHORING_RPC_HOST")
            .expect("Expected ANCHORING_RPC_HOST");
        let user = context.arg("ANCHORING_RPC_USER").ok();
        let passwd = context.arg("ANCHORING_RPC_PASSWD").ok();
        let observer_check_interval = context.arg("ANCHORING_OBSERVER_CHECK_INTERVAL").ok();

        let config = context.get(keys::COMMON_CONFIG).unwrap();

        let network: String = config
            .services_config
            .get("anchoring_network")
            .expect("No network name found.")
            .clone()
            .try_into()
            .unwrap();
        let network = match network.as_str() {
            "testnet" => Network::Testnet,
            "bitcoin" => Network::Bitcoin,
            _ => panic!("Wrong network type"),
        };

        let (p, s) = gen_btc_keypair(network);
        let mut services_public_configs = context
            .get(keys::SERVICES_PUBLIC_CONFIGS)
            .unwrap_or_default();
        services_public_configs.extend(
            vec![
                (
                    "anchoring_pub_key".to_owned(),
                    Value::try_from(p.to_string()).unwrap(),
                ),
            ].into_iter(),
        );

        let rpc_config = AnchoringRpcConfig {
            host,
            username: user,
            password: passwd,
        };
        let observer_config = {
            let mut observer_config = AnchoringObserverConfig::default();
            if let Some(interval) = observer_check_interval {
                observer_config.enabled = true;
                observer_config.check_interval = interval;
            }
            observer_config
        };
        //TODO: Replace this by structure.
        let mut services_secret_configs = context
            .get(keys::SERVICES_SECRET_CONFIGS)
            .unwrap_or_default();
        services_secret_configs.extend(
            vec![
                (
                    "anchoring_sec_key".to_owned(),
                    Value::try_from(s.to_base58check()).unwrap(),
                ),
                (
                    "anchoring_pub_key".to_owned(),
                    Value::try_from(p.to_string()).unwrap(),
                ),
                (
                    "rpc_config".to_owned(),
                    Value::try_from(rpc_config).unwrap(),
                ),
                (
                    "observer_config".to_owned(),
                    Value::try_from(observer_config).unwrap(),
                ),
            ].into_iter(),
        );

        context.set(keys::SERVICES_PUBLIC_CONFIGS, services_public_configs);
        context.set(keys::SERVICES_SECRET_CONFIGS, services_secret_configs);
        Ok(context)
    }
}

struct GenerateCommonConfig;

impl CommandExtension for GenerateCommonConfig {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named(
                "ANCHORING_FREQUENCY",
                false,
                "The frequency of anchoring in blocks",
                None,
                "anchoring-frequency",
                false,
            ),
            Argument::new_named(
                "ANCHORING_UTXO_CONFIRMATIONS",
                false,
                "The minimum number of confirmations for anchoring transactions",
                None,
                "anchoring-utxo-confirmations",
                false,
            ),
            Argument::new_named(
                "ANCHORING_FEE",
                true,
                "Fee that anchoring nodes should use.",
                None,
                "anchoring-fee",
                false,
            ),
            Argument::new_named(
                "ANCHORING_NETWORK",
                true,
                "Anchoring network type.",
                None,
                "anchoring-network",
                false,
            ),
        ]
    }

    fn execute(&self, mut context: Context) -> Result<Context, failure::Error> {
        let anchoring_frequency: u64 = context.arg::<u64>("ANCHORING_FREQUENCY").unwrap_or(500);
        let anchoring_utxo_confirmations: u64 = context
            .arg::<u64>("ANCHORING_UTXO_CONFIRMATIONS")
            .unwrap_or(5);
        let fee: u64 = context.arg::<u64>("ANCHORING_FEE").expect(
            "Expected `ANCHORING_FEE` \
             in cmd.",
        );
        let network = context
            .arg::<String>("ANCHORING_NETWORK")
            .expect("No network type found.");

        let mut values: BTreeMap<String, Value> = context.get(keys::SERVICES_CONFIG).expect(
            "Expected services_config \
             in context.",
        );

        values.extend(
            vec![
                (
                    "anchoring_frequency".to_owned(),
                    Value::try_from(anchoring_frequency).unwrap(),
                ),
                (
                    "anchoring_utxo_confirmations".to_owned(),
                    Value::try_from(anchoring_utxo_confirmations).unwrap(),
                ),
                ("anchoring_fee".to_owned(), Value::try_from(fee).unwrap()),
                (
                    "anchoring_network".to_owned(),
                    Value::try_from(network).unwrap(),
                ),
            ].into_iter(),
        );
        context.set(keys::SERVICES_CONFIG, values);
        Ok(context)
    }
}

struct Finalize;

impl CommandExtension for Finalize {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named(
                "ANCHORING_FUNDING_TXID",
                false,
                "Txid of the initial funding tx",
                None,
                "anchoring-funding-txid",
                false,
            ),
            Argument::new_named(
                "ANCHORING_CREATE_FUNDING_TX",
                false,
                "Create initial funding tx with given amount in satoshis",
                None,
                "anchoring-create-funding-tx",
                false,
            ),
        ]
    }

    fn execute(&self, mut context: Context) -> Result<Context, failure::Error> {
        let mut node_config: NodeConfig = context.get(keys::NODE_CONFIG).unwrap();
        let common_config = context.get(keys::COMMON_CONFIG).unwrap();
        let public_config_list = context.get(keys::PUBLIC_CONFIG_LIST).unwrap();
        let services_secret_configs = context.get(keys::SERVICES_SECRET_CONFIGS).unwrap();

        let funding_txid = context.arg::<String>("ANCHORING_FUNDING_TXID").ok();
        let create_funding_tx_with_amount = context.arg::<u64>("ANCHORING_CREATE_FUNDING_TX").ok();
        // Local config section
        let sec_key: String = services_secret_configs
            .get("anchoring_sec_key")
            .expect("Anchoring secret key not found")
            .clone()
            .try_into()?;
        let pub_key: String = services_secret_configs
            .get("anchoring_pub_key")
            .expect("Anchoring public key not found")
            .clone()
            .try_into()?;
        let rpc: AnchoringRpcConfig = services_secret_configs
            .get("rpc_config")
            .expect("Anchoring rpc config not found")
            .clone()
            .try_into()?;
        let observer: AnchoringObserverConfig = services_secret_configs
            .get("observer_config")
            .expect("Anchoring rpc config not found")
            .clone()
            .try_into()?;
        // Global config section
        let network: String = common_config
            .services_config
            .get("anchoring_network")
            .expect("Anchoring network not found")
            .clone()
            .try_into()?;
        let utxo_confirmations: u64 = common_config
            .services_config
            .get("anchoring_utxo_confirmations")
            .expect("Anchoring utxo confirmations not found")
            .clone()
            .try_into()?;
        let frequency: u64 = common_config
            .services_config
            .get("anchoring_frequency")
            .expect("Anchoring frequency not found")
            .clone()
            .try_into()?;
        let fee: u64 = common_config
            .services_config
            .get("anchoring_fee")
            .expect("Anchoring fee not found")
            .clone()
            .try_into()?;

        let network = match network.as_str() {
            "testnet" => Network::Testnet,
            "bitcoin" => Network::Bitcoin,
            _ => panic!("Wrong network type"),
        };

        let priv_key: PrivateKey = PrivateKey::from_base58check(&sec_key).unwrap();
        //TODO: validate config keys
        let _pub_key = PublicKey::from_hex(&pub_key).unwrap();
        let pub_keys: Vec<_> = public_config_list
            .iter()
            .map(|v| {
                let key: String = v.services_public_configs()
                    .get("anchoring_pub_key")
                    .expect("Anchoring validator public key not found")
                    .clone()
                    .try_into()
                    .unwrap();
                PublicKey::from_hex(&key).unwrap()
            })
            .collect();
        let client = RpcClient::from(rpc.clone());
        let mut anchoring_config = AnchoringNodeConfig::new(Some(rpc));
        anchoring_config.observer = observer;

        let majority_count = ::majority_count(public_config_list.len() as u8);
        let address = btc::RedeemScript::from_pubkeys(&pub_keys, majority_count)
            .compressed(network)
            .to_address(network);

        let mut genesis_cfg = if let Some(total_funds) = create_funding_tx_with_amount {
            client.watch_address(&address, false).unwrap();
            let tx = client.send_to_address(&address, total_funds).unwrap();
            println!("Created funding tx with txid {}", tx.id().to_string());
            AnchoringConfig::new_with_funding_tx(network, pub_keys, tx)
        } else {
            let txid = funding_txid.expect("Funding txid not found");
            let txid = btc::TxId::from_str(&txid).expect("Unable to parse funding txid");
            let tx = client.get_transaction(txid).unwrap().expect(
                "Funding tx with the \
                 given id not found",
            );
            AnchoringConfig::new_with_funding_tx(network, pub_keys, tx.into())
        };

        anchoring_config
            .private_keys
            .insert(address.to_base58check(), priv_key.clone());

        genesis_cfg.fee = fee;
        genesis_cfg.frequency = frequency;
        genesis_cfg.utxo_confirmations = utxo_confirmations;

        node_config.services_configs.insert(
            "anchoring_service".to_owned(),
            Value::try_from(AnchoringServiceConfig {
                genesis: genesis_cfg,
                node: anchoring_config,
            }).expect("Could not serialize anchoring service config"),
        );
        context.set(keys::NODE_CONFIG, node_config);
        Ok(context)
    }
}

/// An anchoring service creator for the `NodeBuilder`.
#[derive(Debug)]
pub struct AnchoringServiceFactory;

impl ServiceFactory for AnchoringServiceFactory {
    #[allow(unused_variables)]
    fn command(&mut self, command: CommandName) -> Option<Box<CommandExtension>> {
        use exonum::helpers::fabric;
        Some(match command {
            v if v == fabric::GenerateNodeConfig::name() => Box::new(GenerateNodeConfig),
            v if v == fabric::GenerateCommonConfig::name() => Box::new(GenerateCommonConfig),
            v if v == fabric::Finalize::name() => Box::new(Finalize),
            _ => return None,
        })
    }
    fn make_service(&mut self, run_context: &Context) -> Box<Service> {
        let anchoring_config: AnchoringServiceConfig =
            run_context.get(keys::NODE_CONFIG).unwrap().services_configs["anchoring_service"]
                .clone()
                .try_into()
                .unwrap();
        Box::new(AnchoringService::new(
            anchoring_config.genesis,
            anchoring_config.node,
        ))
    }
}
