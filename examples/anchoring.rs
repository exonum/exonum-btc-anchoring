extern crate env_logger;
extern crate clap;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate iron;
extern crate router;
extern crate bitcoin;

extern crate exonum;
extern crate blockchain_explorer;
extern crate anchoring_service;
extern crate configuration_service;

use std::net::SocketAddr;
use std::thread;

use clap::{App, Arg, SubCommand};
use router::Router;
use bitcoin::network::constants::Network;
use bitcoin::util::base58::ToBase58;

use exonum::blockchain::{Blockchain, Service};
use exonum::node::{Node, NodeConfig};
use exonum::config::ConfigFile;
use exonum::crypto::HexValue;
use blockchain_explorer::helpers::{GenerateCommand, RunCommand, generate_testnet_config};
use blockchain_explorer::api::Api;
use configuration_service::ConfigurationService;
use configuration_service::config_api::ConfigApi;
use anchoring_service::AnchoringService;
use anchoring_service::AnchoringRpc;
use anchoring_service::{AnchoringNodeConfig, AnchoringConfig, AnchoringRpcConfig,
                        testnet_generate_anchoring_config};
use anchoring_service::btc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnchoringServiceConfig {
    pub genesis: AnchoringConfig,
    pub node: AnchoringNodeConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServicesConfig {
    pub node: NodeConfig,
    pub anchoring_service: AnchoringServiceConfig,
}

fn run_node(blockchain: Blockchain, node_cfg: NodeConfig, port: Option<u16>) {
    if let Some(port) = port {
        let mut node = Node::new(blockchain.clone(), node_cfg.clone());
        let channel = node.channel();
        let api_thread = thread::spawn(move || {
            let keys = (node_cfg.public_key, node_cfg.secret_key);
            let config_api = ConfigApi {
                channel: channel.clone(),
                blockchain: blockchain.clone(),
                config: keys.clone(),
            };
            let listen_address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
            println!("Anchoring node server started on {}", listen_address);
            let mut router = Router::new();
            config_api.wire(&mut router);
            let chain = iron::Chain::new(router);
            iron::Iron::new(chain).http(listen_address).unwrap();
        });
        node.run().unwrap();
        api_thread.join().unwrap();
    } else {
        Node::new(blockchain, node_cfg).run().unwrap();
    }
}

fn main() {
    exonum::crypto::init();
    blockchain_explorer::helpers::init_logger().unwrap();

    let app = App::new("Simple anchoring service demo program")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Aleksey S. <aleksei.sidorov@xdev.re>")
        .subcommand(GenerateCommand::new()
                        .arg(Arg::with_name("ANCHORING_RPC_HOST")
                                 .long("anchoring-host")
                                 .value_name("ANCHORING_RPC_HOST")
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_RPC_USER")
                                 .long("anchoring-user")
                                 .value_name("ANCHORING_RPC_USER")
                                 .required(false)
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_RPC_PASSWD")
                                 .long("anchoring-password")
                                 .value_name("ANCHORING_RPC_PASSWD")
                                 .required(false)
                                 .takes_value(true))
                        .arg(Arg::with_name("ANCHORING_FUNDS")
                                 .long("anchoring-funds")
                                 .value_name("ANCHORING_FUNDS")
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
                                              .value_name("HTTP_PORT")
                                              .help("Run http server on given port")
                                              .takes_value(true)));
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => {
            let count = GenerateCommand::validators_count(matches);
            let dir = GenerateCommand::output_dir(matches);
            let start_port = GenerateCommand::start_port(matches).unwrap_or(2000);

            let host = matches.value_of("ANCHORING_RPC_HOST").unwrap().to_string();
            let user = matches.value_of("ANCHORING_RPC_USER").map(|x| x.to_string());
            let passwd = matches.value_of("ANCHORING_RPC_PASSWD").map(|x| x.to_string());
            let total_funds: u64 = matches.value_of("ANCHORING_FUNDS")
                .unwrap()
                .parse()
                .unwrap();

            let rpc = AnchoringRpcConfig {
                host: host,
                username: user,
                password: passwd,
            };
            let (anchoring_genesis, anchoring_nodes) =
                testnet_generate_anchoring_config(&AnchoringRpc::new(rpc.clone()),
                                                  btc::Network::Testnet,
                                                  count,
                                                  total_funds);

            let node_cfgs = generate_testnet_config(count, start_port);
            let dir = dir.join("validators");
            for (idx, node_cfg) in node_cfgs.into_iter().enumerate() {
                let cfg = ServicesConfig {
                    node: node_cfg,
                    anchoring_service: AnchoringServiceConfig {
                        genesis: anchoring_genesis.clone(),
                        node: anchoring_nodes[idx].clone(),
                    },
                };
                let file_name = format!("{}.toml", idx);
                ConfigFile::save(&cfg, &dir.join(file_name)).unwrap();
            }
        }
        ("run", Some(matches)) => {
            let port: Option<u16> = matches.value_of("HTTP_PORT").map(|x| x.parse().unwrap());
            let path = RunCommand::node_config_path(matches);
            let db = RunCommand::db(matches);
            let cfg: ServicesConfig = ConfigFile::load(&path).unwrap();

            let anchoring_cfg = cfg.anchoring_service;
            let client = AnchoringRpc::new(anchoring_cfg.node.rpc.clone());
            let services: Vec<Box<Service>> = vec![Box::new(AnchoringService::new(client,
                                                    anchoring_cfg.genesis,
                                                    anchoring_cfg.node)),
                     Box::new(ConfigurationService::new())];
            let blockchain = Blockchain::new(db, services);
            run_node(blockchain, cfg.node, port)
        }
        ("keypair", Some(matches)) => {
            let network = match matches.value_of("NETWORK").unwrap() {
                "testnet" => Network::Testnet,
                "bitcoin" => Network::Bitcoin,
                _ => panic!("Wrong network type"),
            };

            let (p, s) = btc::gen_keypair(network);
            println!("pub_key={}", p.to_hex());
            println!("sec_key={}", s.to_base58check());
        }
        _ => {
            panic!("Wrong subcommand");
        }
    }
}
