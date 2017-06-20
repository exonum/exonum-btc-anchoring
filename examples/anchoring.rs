extern crate clap;
#[macro_use]
extern crate serde_derive;
extern crate iron;
extern crate router;
extern crate bitcoin;

extern crate exonum;
extern crate anchoring_btc_service;
extern crate configuration_service;

use clap::{App, Arg};
use bitcoin::network::constants::Network;
use bitcoin::util::base58::{ToBase58, FromBase58};

use std::collections::BTreeMap;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::{Node, NodeConfig};
use exonum::config::ConfigFile;
use exonum::crypto::HexValue;
use exonum::helpers::clap::{ConfigTemplate, Value, 
                             GenerateTestnetCommand, RunCommand,
                             InitCommand, GenerateTemplateCommand,
                             KeyGeneratorCommand, AddValidatorCommand };
use exonum::helpers::generate_testnet_config;
use exonum::helpers;
use configuration_service::ConfigurationService;
use anchoring_btc_service::AnchoringService;
use anchoring_btc_service::AnchoringRpc;
use anchoring_btc_service::details::btc::transactions::FundingTx;
use anchoring_btc_service::details::btc::{ PublicKey, PrivateKey};
use anchoring_btc_service::{AnchoringConfig, AnchoringNodeConfig, AnchoringRpcConfig,
                            BitcoinNetwork, gen_anchoring_testnet_config, gen_btc_keypair};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnchoringServiceConfig {
    pub common: AnchoringConfig,
    pub node: AnchoringNodeConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServicesConfig {
    pub node: NodeConfig,
    pub anchoring_service: AnchoringServiceConfig,
}

// if rpc node is in `test` mode,
// and could send money for our purposes.
static ANCHORING_NODE_TEST: bool = true;

fn main() {
    exonum::crypto::init();
    helpers::init_logger().unwrap();

    let app = App::new("Simple anchoring service demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .subcommand(KeyGeneratorCommand::new()
                        .arg(Arg::with_name("NETWORK")
                                 .help("Keypair network")
                                 .required(true)
                                 .index(2)))
        .subcommand(GenerateTemplateCommand::new()
                    .arg(Arg::with_name("ANCHORING_FUNDS")
                                 .long("anchoring-funds")
                                 .takes_value(true)
                                 .required(true))
                    .arg(Arg::with_name("ANCHORING_FEE")
                                 .long("anchoring-fee")
                                 .takes_value(true)
                                 .required(true))
                    .arg(Arg::with_name("NETWORK")
                                 .help("Keypair network")
                                 .required(true)
                                 .index(3)))
        .subcommand(AddValidatorCommand::new())
        .subcommand(InitCommand::new()
                        .arg(Arg::with_name("ANCHORING_RPC_HOST")
                                 .long("anchoring-host")
                                 .takes_value(true)
                                 .required(true))
                        .arg(Arg::with_name("ANCHORING_RPC_USER")
                                 .long("anchoring-user")
                                 .required(false)
                                 .takes_value(true)
                                 .required(true))
                        .arg(Arg::with_name("ANCHORING_RPC_PASSWD")
                                 .long("anchoring-password")
                                 .required(false)
                                 .takes_value(true)
                                 .required(true)))
        .subcommand(GenerateTestnetCommand::new()
                        .arg(Arg::with_name("ANCHORING_RPC_HOST")
                                 .long("anchoring-host")
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_RPC_USER")
                                 .long("anchoring-user")
                                 .required(false)
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_RPC_PASSWD")
                                 .long("anchoring-password")
                                 .required(false)
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_FUNDS")
                                 .long("anchoring-funds")
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_FEE")
                                 .long("anchoring-fee")
                                 .takes_value(true)))
        .subcommand(RunCommand::new().arg(Arg::with_name("HTTP_PORT")
                                              .short("p")
                                              .long("port")
                                              .help("Run http server on given port")
                                              .takes_value(true)));
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => {
            let count = GenerateTestnetCommand::validators_count(matches);
            let dir = GenerateTestnetCommand::output_dir(matches);
            let start_port = GenerateTestnetCommand::start_port(matches).unwrap_or(2000);

            let host = matches.value_of("ANCHORING_RPC_HOST").unwrap().to_string();
            let user = matches
                .value_of("ANCHORING_RPC_USER")
                .map(|x| x.to_string());
            let passwd = matches
                .value_of("ANCHORING_RPC_PASSWD")
                .map(|x| x.to_string());
            let total_funds: u64 = matches
                .value_of("ANCHORING_FUNDS")
                .unwrap()
                .parse()
                .unwrap();
            let fee: u64 = matches.value_of("ANCHORING_FEE").unwrap().parse().unwrap();

            let rpc = AnchoringRpcConfig {
                host: host,
                username: user,
                password: passwd,
            };
            let (mut anchoring_common, anchoring_nodes) =
                gen_anchoring_testnet_config(&AnchoringRpc::new(rpc.clone()),
                                             BitcoinNetwork::Testnet,
                                             count,
                                             total_funds);
            anchoring_common.fee = fee;

            let node_cfgs = generate_testnet_config(count, start_port);
            let dir = dir.join("validators");
            for (idx, node_cfg) in node_cfgs.into_iter().enumerate() {
                let cfg = ServicesConfig {
                    node: node_cfg,
                    anchoring_service: AnchoringServiceConfig {
                        common: anchoring_common.clone(),
                        node: anchoring_nodes[idx].clone(),
                    },
                };
                let file_name = format!("{}.toml", idx);
                ConfigFile::save(&cfg, &dir.join(file_name)).unwrap();
            }
        }
        ("run", Some(matches)) => {
            let path = RunCommand::node_config_path(matches);
            let db = RunCommand::db(matches);
            let cfg: ServicesConfig = ConfigFile::load(path).unwrap();

            let anchoring_cfg = cfg.anchoring_service;
            let services: Vec<Box<Service>> = vec![
                    Box::new(AnchoringService::new(anchoring_cfg.common, anchoring_cfg.node)),
                    Box::new(ConfigurationService::new()),
                ];
            let blockchain = Blockchain::new(db, services);
            let mut node = Node::new(blockchain, cfg.node);
            node.run().unwrap();
        }
        ("generate-template", Some(matches)) => {
            let total_funds: u64 = matches
                .value_of("ANCHORING_FUNDS")
                .unwrap()
                .parse()
                .unwrap();
            let fee: u64 = matches
                .value_of("ANCHORING_FEE")
                .unwrap()
                .parse()
                .unwrap();
            let network = matches.value_of("NETWORK").unwrap().to_string();
            
            let values: BTreeMap<String, Value> = 
                    vec![("ANCHORING_FUNDS".to_owned(), Value::try_from(total_funds).unwrap()),
                          ("ANCHORING_FEE".to_owned(), Value::try_from(fee).unwrap()),
                          ("ANCHORING_NETWORK".to_owned(), Value::try_from(network).unwrap())]
                    .into_iter().collect();
            GenerateTemplateCommand::execute(matches, values);
        }
        ("add-validator", Some(matches)) => {
            AddValidatorCommand::execute(matches, |_, _| Ok(()));
        }
        ("init", Some(matches)) => {
            let host = matches.value_of("ANCHORING_RPC_HOST").unwrap().to_string();
            let user = matches
                .value_of("ANCHORING_RPC_USER")
                .map(|x| x.to_string());
            let passwd = matches
                .value_of("ANCHORING_RPC_PASSWD")
                .map(|x| x.to_string());
            InitCommand::execute(matches, |node_config: NodeConfig, 
                                            template: &ConfigTemplate, 
                                            keys: &BTreeMap<String, Value>| {

            let sec_key: String = keys.get("anchoring_sec_key")
                            .expect("Anchoring secret key not fount")
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
            //\TODO: validate config
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

            let majority_count = ::anchoring_btc_service::majority_count(template.count() as u8);
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

            let cfg = ServicesConfig {
                node: node_config,
                anchoring_service: AnchoringServiceConfig {
                    common: genesis_cfg,
                    node: anchoring_config,
                },
            };
            Ok(Value::try_from(cfg).unwrap())
            });
        }
        ("keygen", Some(matches)) => {
            let network = match matches.value_of("NETWORK").unwrap() {
                "testnet" => Network::Testnet,
                "bitcoin" => Network::Bitcoin,
                _ => panic!("Wrong network type"),
            };
            let (p, s) = gen_btc_keypair(network);
            let pub_map: BTreeMap<String, Value> = 
                        vec![("anchoring_pub_key".to_owned(), 
                                Value::try_from(p.to_hex()).unwrap())].into_iter().collect();
            let sec_map: BTreeMap<String, Value> = 
                        vec![("anchoring_sec_key".to_owned(), 
                                Value::try_from(s.to_base58check()).unwrap()),
                           ("anchoring_pub_key".to_owned(), 
                                Value::try_from(p.to_hex()).unwrap())].into_iter().collect();
            KeyGeneratorCommand::execute(matches, sec_map, pub_map );
        }
        _ => {
            panic!("Wrong subcommand");
        }
    }
}
