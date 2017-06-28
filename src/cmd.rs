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
pub struct AnchoringServiceConfig {
    pub common: AnchoringConfig,
    pub node: AnchoringNodeConfig,
}
// if rpc node is in `test` mode,
// and could send money for our purposes.
static ANCHORING_NODE_TEST: bool = true;

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
            Argument::new_named("ANCHORING_FUNDS", true,
            "Funds in anchoring adress.", None, "anchoring-funds"),
            Argument::new_named("ANCHORING_FEE", true,
            "Fee that anchoring nodes should use.",  None, "anchoring-fee"),
            Argument::new_positional("NETWORK", true,
            "Anchoring network name."),
        ]
    }

    fn execute(&self, mut context: Context) -> Result<Context, Box<Error>> {
            let total_funds: u64 = context.arg::<u64>("ANCHORING_FUNDS")
                                  .expect("Expected ANCHORING_FUNDS in cmd.");
            let fee: u64 = context.arg::<u64>("ANCHORING_FEE")
                                  .expect("Expected ANCHORING_FEE in cmd.");
            let network = context.arg::<String>("NETWORK")
                                   .expect("No network name found.");
            
            let mut values: BTreeMap<String, Value> = 
                context.get("VALUES")
                    .expect("Expected VALUES in context.");
            values.extend(
                        vec![("ANCHORING_FUNDS".to_owned(),
                                Value::try_from(total_funds).unwrap()),
                            ("ANCHORING_FEE".to_owned(),
                                Value::try_from(fee).unwrap()),
                            ("ANCHORING_NETWORK".to_owned(),
                                Value::try_from(network).unwrap())]
                        .into_iter());
            context.set("VALUES", values);
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
        ]
    }

    fn execute(&self, mut context: Context) -> Result<Context, Box<Error>> {
            let host = context.arg("ANCHORING_RPC_HOST")
                              .expect("Expected ANCHORING_RPC_HOST");
            let user = context.arg("ANCHORING_RPC_USER").ok();
            let passwd = context.arg("ANCHORING_RPC_PASSWD").ok();
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
            let total_funds: u64 = template.services.get("ANCHORING_FUNDS")
                            .expect("Anchoring funds not fount")
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
            

            let mut genesis_cfg = if ANCHORING_NODE_TEST {
                let tx = FundingTx::create(&client, &address, total_funds).unwrap();
                AnchoringConfig::new_with_funding_tx(network, pub_keys, tx)
            }
            else {
                AnchoringConfig::new(network, pub_keys)
            };

            anchoring_config
                    .private_keys
                    .insert(address.to_base58check(), priv_key.clone());

            genesis_cfg.fee = fee;

            node_config.services_configs.insert(
                "anchoring_service".to_owned(), Value::try_from(
                    AnchoringServiceConfig {
                    common: genesis_cfg,
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
        Box::new(AnchoringService::new(anchoring_cfg.common, anchoring_cfg.node))
    }
}
