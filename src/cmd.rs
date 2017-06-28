//! Basic clap factory implementation.
//! This module collect all basic `CommandExtension` that
//! we can use in `anchoring` bootstraping process.
//!
use toml::Value;
use exonum::helpers::fabric::{CommandName, Argument, Context, CommandExtension, ServiceFactory};
use exonum::blockchain::Service;

use exonum::node::NodeConfig;
use exonum::crypto::HexValue;
use bitcoin::util::base58::ToBase58;

use bitcoin::network::constants::Network;
use std::error::Error;
use std::collections::BTreeMap;

use service::AnchoringService;
use super::{AnchoringConfig, AnchoringNodeConfig, AnchoringRpcConfig, gen_btc_keypair};
use exonum::helpers::clap::{ConfigTemplate };

use details::btc::{ PublicKey, PrivateKey};

use AnchoringRpc;
use details::btc::transactions::FundingTx;
use bitcoin::util::base58::FromBase58;
                             
#[derive(Clone, Debug, Serialize, Deserialize)]
/// Anchoring configuration that should be saved into file
pub struct AnchoringServiceConfig {
    /// `AnchoringConfig` is a common for all nodes part.
    pub genesis: AnchoringConfig,
    /// `AnchoringNodeConfig` is a unique for each node.
    pub node: AnchoringNodeConfig,
}

struct KeygenCommand;

impl CommandExtension for KeygenCommand {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_positional("NETWORK", true,
            "Anchoring network name."),
        ]
    }

    fn execute(&self, mut context: Context) -> Result<Context, Box<Error>> {
        let network = match context.arg::<String>("NETWORK")
                                   .expect("No network name found.").as_str() {
                "testnet" => Network::Testnet,
                "bitcoin" => Network::Bitcoin,
                _ => panic!("Wrong network type"),
        };
        let (p, s) = gen_btc_keypair(network);
        let mut services_pub_keys: BTreeMap<String, Value> = 
            context.get("services_pub_keys")
                .unwrap_or_default();
        services_pub_keys.extend(vec![("anchoring_pub_key".to_owned(), 
                            Value::try_from(p.to_hex()).unwrap())].into_iter());

        let mut services_sec_keys: BTreeMap<String, Value> = 
            context.get("services_sec_keys")
                .unwrap_or_default();
        services_sec_keys.extend(vec![("anchoring_sec_key".to_owned(), 
                            Value::try_from(s.to_base58check()).unwrap()),
                        ("anchoring_pub_key".to_owned(), 
                            Value::try_from(p.to_hex()).unwrap())].into_iter());

        context.set("services_pub_keys", services_pub_keys);
        context.set("services_sec_keys", services_sec_keys);
        Ok(context)
    }
}

struct GenerateTemplateCommand;

impl CommandExtension for GenerateTemplateCommand {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named("ANCHORING_FREQUENCY", false,
            "The frequency of anchoring in blocks", None, "anchoring-frequency"),
            Argument::new_named("ANCHORING_UTXO_CONFIRMATIONS", false,
            "The minimum number of confirmations for anchoring transactions", None, "anchoring-utxo-confirmations"),
            Argument::new_named("ANCHORING_FEE", true,
            "Fee that anchoring nodes should use.",  None, "anchoring-fee"),
            Argument::new_positional("NETWORK", true,
            "Anchoring network name."),
        ]
    }

    fn execute(&self, mut context: Context) -> Result<Context, Box<Error>> {
            let anchoring_frequency: u64 = context.arg::<u64>("ANCHORING_FREQUENCY")
                                  .unwrap_or(500);
            let anchoring_utxo_confirmations: u64 = context.arg::<u64>("ANCHORING_UTXO_CONFIRMATIONS")
                                  .unwrap_or(5);
            let fee: u64 = context.arg::<u64>("ANCHORING_FEE")
                                  .expect("Expected `ANCHORING_FEE` in cmd.");
            let network = context.arg::<String>("NETWORK")
                                   .expect("No network name found.");
            
            let mut values: BTreeMap<String, Value> = 
                context.get("VALUE")
                    .expect("Expected VALUES in context.");

            values.extend(
                        vec![("ANCHORING_FREQUENCY".to_owned(),
                                Value::try_from(anchoring_frequency).unwrap()),
                            ("ANCHORING_UTXO_CONFIRMATIONS".to_owned(),
                                Value::try_from(anchoring_utxo_confirmations).unwrap()),
                            ("ANCHORING_FEE".to_owned(),
                                Value::try_from(fee).unwrap()),
                            ("ANCHORING_NETWORK".to_owned(),
                                Value::try_from(network).unwrap())]
                        .into_iter());
            context.set("VALUE", values);
            Ok(context)
    }
}

struct InitCommand;

impl CommandExtension for InitCommand {
    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::new_named("ANCHORING_RPC_HOST", true,
            "Host of bitcoind.", None, "anchoring-host"),
            Argument::new_named("ANCHORING_RPC_USER", false,
            "User to login into bitcoind.",  None, "anchoring-user"),
            Argument::new_named("ANCHORING_RPC_PASSWD", false,
            "Password to login into bitcoind.",  None, "anchoring-password"),
            Argument::new_named("ANCHORING_FUNDING_TXID", false, 
            "Txid of the initial funding tx", None, "anchoring-funding-txid"),
            Argument::new_named("ANCHORING_CREATE_FUNDING_TX", false, 
            "Create initial funding tx with given amount in satoshis", None, "anchoring-create-funding-tx")
        ]
    }

    fn execute(&self, mut context: Context) -> Result<Context, Box<Error>> {
            let host = context.arg("ANCHORING_RPC_HOST")
                              .expect("Expected ANCHORING_RPC_HOST");
            let user = context.arg("ANCHORING_RPC_USER").ok();
            let passwd = context.arg("ANCHORING_RPC_PASSWD").ok();
            let funding_txid = context.arg::<String>("ANCHORING_FUNDING_TXID").ok();
            let create_funding_tx_with_amount = context.arg::<u64>("ANCHORING_CREATE_FUNDING_TX").ok();

            let mut node_config: NodeConfig = context.get("node_config").unwrap();
            let template: ConfigTemplate = context.get("template").unwrap();
            let keys: BTreeMap<String, Value> = context.get("services_sec_keys").unwrap();

            let sec_key: String = keys.get("anchoring_sec_key")
                            .expect("Anchoring secret key not found")
                            .clone()
                            .try_into()
                            .unwrap();
            let pub_key: String = keys.get("anchoring_pub_key")
                            .expect("Anchoring public key not fount")
                            .clone()
                            .try_into()
                            .unwrap();

            let network: String = template.services.get("ANCHORING_NETWORK")
                            .expect("Anchoring network not fount")
                            .clone()
                            .try_into()
                            .unwrap();
            let utxo_confirmations: u64 = template.services.get("ANCHORING_UTXO_CONFIRMATIONS")
                            .expect("Anchoring utxo confirmations not fount")
                            .clone()
                            .try_into()
                            .unwrap();
            let frequency: u64 = template.services.get("ANCHORING_FREQUENCY")
                            .expect("Anchoring frequency not fount")
                            .clone()
                            .try_into()
                            .unwrap();
            let fee: u64 = template.services.get("ANCHORING_FEE")
                            .expect("Anchoring fee not fount")
                            .clone()
                            .try_into()
                            .unwrap();

            let network = match network.as_ref() {
                "testnet" => Network::Testnet,
                "bitcoin" => Network::Bitcoin,
                _ => panic!("Wrong network type"),
            };

            let priv_key: PrivateKey = PrivateKey::from_base58check(&sec_key).unwrap();
            //\TODO: validate config keys
            let _pub_key: PublicKey = HexValue::from_hex(&pub_key).unwrap();
            let pub_keys: Vec<PublicKey> = template.validators()
                .iter()
                .map(|(_, ref v)|{
                    let key: String = v.keys()
                    .get("anchoring_pub_key")
                    .expect("Anchoring validator public key not fount")
                    .clone()
                    .try_into()
                    .unwrap();
                    HexValue::from_hex(&key).unwrap()
                }).collect();
            let rpc = AnchoringRpcConfig {
                host: host,
                username: user,
                password: passwd,
            };
            let client = AnchoringRpc::new(rpc.clone());
            let mut anchoring_config = AnchoringNodeConfig::new(Some(rpc));

            let majority_count = ::majority_count(template.count() as u8);
            let (_, address) = client
                    .create_multisig_address(network, majority_count, pub_keys.iter())
                    .unwrap();
            

            let mut genesis_cfg = if let Some(total_funds) = create_funding_tx_with_amount {
                let tx = FundingTx::create(&client, &address, total_funds).unwrap();
                println!("Created funding tx with txid {}", tx.txid());
                AnchoringConfig::new_with_funding_tx(network, pub_keys, tx)
            }
            else {
                let txid = funding_txid.expect("Funding txid not fount");
                let tx = client.get_transaction(&txid).unwrap().expect("Funding tx with the given id not fount");
                AnchoringConfig::new_with_funding_tx(network, pub_keys, tx.into())
            };

            anchoring_config
                    .private_keys
                    .insert(address.to_base58check(), priv_key.clone());

            genesis_cfg.fee = fee;
            genesis_cfg.frequency = frequency;
            genesis_cfg.utxo_confirmations = utxo_confirmations;

            node_config.services_configs.insert(
                "anchoring_service".to_owned(), Value::try_from(
                    AnchoringServiceConfig {
                    genesis: genesis_cfg,
                    node: anchoring_config,
                }).expect("could not serialize anchoring service config"));
            context.set("node_config", node_config);
            Ok(context)
    }
}


impl ServiceFactory for AnchoringService {
    #[allow(unused_variables)]
    fn command(command: CommandName) -> Option<Box<CommandExtension>> {
        use exonum::helpers::fabric;
        Some(match command {
            v if v == fabric::KeyGeneratorCommand::name() => Box::new(KeygenCommand),
            v if v == fabric::GenerateTemplateCommand::name() => Box::new(GenerateTemplateCommand),
            v if v == fabric::InitCommand::name() => Box::new(InitCommand),
            _ => return None,
        })
    }
    fn make_service( run_context: &Context) -> Box<Service> {
        let node_config: NodeConfig = run_context.get("node_config")
                                                .unwrap();
        let anchoring_cfg: AnchoringServiceConfig = 
                        node_config.services_configs.get("anchoring_service")
                                                        .unwrap()
                                                        .clone()
                                                        .try_into()
                                                        .unwrap();
        Box::new(AnchoringService::new(anchoring_cfg.genesis, anchoring_cfg.node))
    }
}
