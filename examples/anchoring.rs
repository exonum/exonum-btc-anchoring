extern crate exonum;
extern crate btc_anchoring_service;
//extern crate configuration_service;
use exonum::helpers::fabric::NodeBuilder;
use exonum::helpers;
use btc_anchoring_service::AnchoringService;
//use configuration_service::ConfigurationService;

fn main() {
    exonum::crypto::init();
    helpers::init_logger().unwrap();
    let node = NodeBuilder::new().with_service::<AnchoringService>();
    node.run();
}
