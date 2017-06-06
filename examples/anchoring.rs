extern crate clap;
#[macro_use]
extern crate serde_derive;
extern crate iron;
extern crate router;
extern crate bitcoin;

extern crate exonum;
extern crate anchoring_btc_service;
extern crate configuration_service;

use clap::{App, Arg, SubCommand};
use bitcoin::network::constants::Network;
use bitcoin::util::base58::ToBase58;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::{Node, NodeConfig};
use exonum::config::ConfigFile;
use exonum::crypto::HexValue;
use exonum::helpers::clap::{GenerateCommand, RunCommand};
use exonum::helpers::generate_testnet_config;
use exonum::helpers;
use configuration_service::ConfigurationService;
use anchoring_btc_service::{AnchoringService, AnchoringRpc};
use anchoring_btc_service::observer::{AnchoringChainObserver, ObserverConfig};
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

fn main() {
    exonum::crypto::init();
    helpers::init_logger().unwrap();

    let app = App::new("Simple anchoring service demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .subcommand(GenerateCommand::new()
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
        .subcommand(SubCommand::with_name("keypair")
                        .help("Generates a new bitcoin keypair")
                        .arg(Arg::with_name("NETWORK")
                                 .help("Keypair network")
                                 .required(true)
                                 .index(1)))
        .subcommand(RunCommand::new().arg(Arg::with_name("HTTP_PORT")
                                              .short("p")
                                              .long("port")
                                              .help("Run http server on given port")
                                              .takes_value(true)));
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => {
            let count = GenerateCommand::validators_count(matches);
            let dir = GenerateCommand::output_dir(matches);
            let start_port = GenerateCommand::start_port(matches).unwrap_or(2000);

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
                    Box::new(AnchoringService::new(anchoring_cfg.common, anchoring_cfg.node.clone())),
                    Box::new(ConfigurationService::new()),
                ];
            let blockchain = Blockchain::new(db, services);

            let observer_cfg = ObserverConfig {
                rpc: anchoring_cfg.node.rpc.unwrap().clone(),
                check_frequency: 300000,
            };
            let observer = AnchoringChainObserver::new(observer_cfg, blockchain.clone());
            let observer_thread = ::std::thread::spawn(move || {
                observer.run().unwrap();
            });

            let mut node = Node::new(blockchain, cfg.node);
            node.run().unwrap();
            observer_thread.join().unwrap();
        }
        ("keypair", Some(matches)) => {
            let network = match matches.value_of("NETWORK").unwrap() {
                "testnet" => Network::Testnet,
                "bitcoin" => Network::Bitcoin,
                _ => panic!("Wrong network type"),
            };

            let (p, s) = gen_btc_keypair(network);
            println!("pub_key={}", p.to_hex());
            println!("sec_key={}", s.to_base58check());
        }
        _ => {
            panic!("Wrong subcommand");
        }
    }
}
