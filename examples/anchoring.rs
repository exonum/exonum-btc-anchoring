extern crate env_logger;
extern crate clap;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

extern crate exonum;
extern crate blockchain_explorer;
extern crate anchoring_service;

use clap::{App, Arg};

use exonum::blockchain::{Blockchain, Service};
use exonum::node::{Node, NodeConfig};
use exonum::config::ConfigFile;
use blockchain_explorer::helpers::{GenerateCommand, RunCommand, generate_testnet_config};

use anchoring_service::AnchoringService;
use anchoring_service::config::{AnchoringNodeConfig, AnchoringConfig, AnchoringRpcConfig,
                                generate_anchoring_config};

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
        .subcommand(RunCommand::new());
    let matches = app.get_matches();

    match matches.subcommand() {
        ("generate", Some(matches)) => {
            let count = GenerateCommand::validators_count(matches);
            let dir = GenerateCommand::output_dir(matches);
            let start_port = GenerateCommand::start_port(matches).unwrap_or(2000);

            let host = matches.value_of("ANCHORING_RPC_HOST").unwrap().to_string();
            let user = matches.value_of("ANCHORING_RPC_USER").map(|x| x.to_string());
            let passwd = matches.value_of("ANCHORING_RPC_PASSWD").map(|x| x.to_string());
            let total_funds: u64 = matches.value_of("ANCHORING_FUNDS").unwrap().parse().unwrap();

            let rpc = AnchoringRpcConfig {
                host: host,
                username: user,
                password: passwd,
            };
            let (anchoring_genesis, anchoring_nodes) =
                generate_anchoring_config(&rpc.clone().into_client(), count, total_funds);

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
            // TODO add service with transactions

            let path = RunCommand::node_config_path(matches);
            let db = RunCommand::db(matches);
            let cfg: ServicesConfig = ConfigFile::load(&path).unwrap();

            let anchoring_cfg = cfg.anchoring_service;
            let services: Vec<Box<Service>> =
                vec![Box::new(AnchoringService::new(anchoring_cfg.node.rpc.clone().into_client(),
                                                    anchoring_cfg.genesis,
                                                    anchoring_cfg.node))];
            let blockchain = Blockchain::new(db, services);
            Node::new(blockchain, cfg.node).run().unwrap();
        }
        _ => {
            panic!("Wrong subcommand");
        }
    }
}
