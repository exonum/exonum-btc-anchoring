#[macro_use]
extern crate exonum;
extern crate sandbox;
extern crate anchoring_btc_service;
extern crate serde;
extern crate serde_json;
extern crate bitcoin;
extern crate bitcoinrpc;
extern crate byteorder;
extern crate secp256k1;
extern crate blockchain_explorer;
#[macro_use]
extern crate log;
extern crate rand;
extern crate libc;

use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard};

pub use bitcoinrpc::RpcError as JsonRpcError;
pub use bitcoinrpc::Error as RpcError;

use rand::{SeedableRng, StdRng};
use bitcoin::util::base58::{ToBase58, FromBase58};

use exonum::crypto::Hash;
use exonum::messages::{Message, RawTransaction};

use sandbox::sandbox_with_services;
use sandbox::sandbox::Sandbox;
use sandbox::timestamping::TimestampingService;
use sandbox::sandbox_tests_helper::VALIDATOR_0;
use sandbox::config_updater::ConfigUpdateService;

use anchoring_btc_service::{gen_anchoring_testnet_config_with_rng, AnchoringConfig,
                            AnchoringNodeConfig, AnchoringRpc, AnchoringService};
use anchoring_btc_service::details::sandbox::{SandboxClient, Request};
use anchoring_btc_service::details::btc;
use anchoring_btc_service::details::btc::transactions::{TransactionBuilder, AnchoringTx, FundingTx};
use anchoring_btc_service::blockchain::dto::MsgAnchoringSignature;
use anchoring_btc_service::handler::{AnchoringHandler, collect_signatures};
use anchoring_btc_service::error::HandlerError;

#[macro_use]
mod macros;
#[cfg(test)]
mod tests;
pub mod helpers;
pub mod secp256k1_hack;

pub const ANCHORING_VALIDATOR: u32 = VALIDATOR_0;
pub const ANCHORING_FREQUENCY: u64 = 10;
pub const ANCHORING_FUNDS: u64 = 4000;
pub const CHECK_LECT_FREQUENCY: u64 = 6;

pub struct AnchoringSandboxState {
    pub common: AnchoringConfig,
    pub nodes: Vec<AnchoringNodeConfig>,
    pub latest_anchored_tx: Option<(AnchoringTx, Vec<MsgAnchoringSignature>)>,
    pub handler: Arc<Mutex<AnchoringHandler>>,
}

impl AnchoringSandboxState {
    pub fn handler(&self) -> MutexGuard<AnchoringHandler> {
        self.handler.lock().unwrap()
    }

    pub fn take_errors(&self) -> Vec<HandlerError> {
        let mut handler = self.handler();
        let v = handler.errors.drain(..).collect::<Vec<_>>();
        v
    }

    pub fn latest_anchored_tx(&self) -> &AnchoringTx {
        &self.latest_anchored_tx.as_ref().unwrap().0
    }

    pub fn latest_anchored_tx_signatures(&self) -> &[MsgAnchoringSignature] {
        self.latest_anchored_tx.as_ref().unwrap().1.as_ref()
    }

    pub fn gen_anchoring_tx_with_signatures(&mut self,
                                            sandbox: &Sandbox,
                                            height: u64,
                                            block_hash: Hash,
                                            funds: &[FundingTx],
                                            prev_tx_chain: Option<btc::TxId>,
                                            addr: &btc::Address)
                                            -> (AnchoringTx, Vec<RawTransaction>) {
        let (propose_tx, signed_tx, signs) = {
            let prev_tx = self.latest_anchored_tx
                .as_ref()
                .map(|x| x.0.deref())
                .unwrap_or(self.common.funding_tx.deref());

            let mut builder = TransactionBuilder::with_prev_tx(prev_tx, 0)
                .payload(height, block_hash)
                .prev_tx_chain(prev_tx_chain)
                .send_to(addr.clone())
                .fee(1000);
            for fund in funds {
                let out = fund.find_out(addr).unwrap();
                builder = builder.add_funds(fund, out);
            }

            let tx = builder.into_transaction().unwrap();
            let signs = self.gen_anchoring_signatures(sandbox, &tx);
            let signed_tx = self.finalize_tx(tx.clone(), signs.as_ref());
            (tx, signed_tx, signs)
        };
        self.latest_anchored_tx = Some((signed_tx, signs.clone()));

        let signs = signs
            .into_iter()
            .map(|tx| tx.raw().clone())
            .collect::<Vec<_>>();
        (propose_tx, signs)
    }

    pub fn finalize_tx(&self, tx: AnchoringTx, signs: &[MsgAnchoringSignature]) -> AnchoringTx {
        let collected_signs = collect_signatures(&tx, &self.common, signs.iter()).unwrap();
        tx.finalize(&self.common.redeem_script().0, collected_signs)
    }

    pub fn gen_anchoring_signatures(&self,
                                    sandbox: &Sandbox,
                                    tx: &AnchoringTx)
                                    -> Vec<MsgAnchoringSignature> {
        let (redeem_script, addr) = self.common.redeem_script();

        let priv_keys = self.priv_keys(&addr);
        let mut signs = Vec::new();
        for (validator, priv_key) in priv_keys.iter().enumerate() {
            for input in tx.inputs() {
                let signature = tx.sign_input(&redeem_script, input, priv_key);
                signs.push(MsgAnchoringSignature::new(&sandbox.p(validator),
                                                      validator as u32,
                                                      tx.clone(),
                                                      input,
                                                      &signature,
                                                      sandbox.s(validator)));
            }
        }
        signs
    }

    pub fn priv_keys(&self, addr: &btc::Address) -> Vec<btc::PrivateKey> {
        self.nodes
            .iter()
            .map(|cfg| cfg.private_keys[&addr.to_base58check()].clone())
            .collect::<Vec<_>>()
    }

    pub fn current_priv_keys(&self) -> Vec<btc::PrivateKey> {
        self.priv_keys(&self.common.redeem_script().1)
    }

    pub fn current_addr(&self) -> btc::Address {
        self.common.redeem_script().1
    }

    pub fn current_redeem_script(&self) -> btc::RedeemScript {
        self.common.redeem_script().0
    }

    pub fn nearest_check_lect_height(&self, sandbox: &Sandbox) -> u64 {
        let height = sandbox.current_height();
        let frequency = self.nodes[0].check_lect_frequency as u64;
        height - height % frequency + frequency
    }

    pub fn nearest_anchoring_height(&self, sandbox: &Sandbox) -> u64 {
        let height = sandbox.current_height();
        self.common.nearest_anchoring_height(height)
    }
}

/// Generates config for 4 validators and 10000 funds
pub fn gen_sandbox_anchoring_config(client: &mut AnchoringRpc)
                                    -> (AnchoringConfig, Vec<AnchoringNodeConfig>) {
    let requests = vec![request! {
            method: "importaddress",
            params: ["2NFGToas8B6sXqsmtGwL1H4kC5fGWSpTcYA", "multisig", false, false]
        },
                        request! {
            method: "sendtoaddress",
            params: [
                "2NFGToas8B6sXqsmtGwL1H4kC5fGWSpTcYA",
                "0.00004"
            ],
            response: "a788a2f0a369f3985c5f713d985bb1e7bd3dfb8b35f194b39a5f3ae7d709af9a"
        },
                        request! {
            method: "getrawtransaction",
            params: [
                "a788a2f0a369f3985c5f713d985bb1e7bd3dfb8b35f194b39a5f3ae7d709af9a",
                0
            ],
            response: "0100000001e56b729856ecd8a9712cb86a8a702bbd05478b0a323f06d2bcfdce373fc9c71b0\
                10000006a4730440220410e697174595270abbf2e2542ce42186ef6d48fc0dcf9a2c26cb639d6d9e89\
                30220735ff3e6f464d426eec6dd5acfda268624ef628aab38124a1a0b82c1670dddd50121032375139\
                6efcc7e842b522b9d95d84a4f0e4663861124150860d0f728c2cc7d56feffffff02a00f00000000000\
                017a914f18eb74087f751109cc9052befd4177a52c9a30a870313d70b000000001976a914eed3fc59a\
                211ef5cbf1986971cae80bcc983d23a88ac35ae1000"
        }];
    client.expect(requests);
    let mut rng: StdRng = SeedableRng::from_seed([1, 2, 3, 4].as_ref());
    gen_anchoring_testnet_config_with_rng(client,
                                          btc::Network::Testnet,
                                          4,
                                          ANCHORING_FUNDS,
                                          &mut rng)
}

/// Initialize sandbox for anchoring node
pub fn initialize_anchoring_sandbox<'a, I>(priv_keys: I)
                                           -> (Sandbox, AnchoringRpc, AnchoringSandboxState)
    where I: IntoIterator<Item = &'a (&'a str, Vec<&'a str>)>
{
    let mut client = AnchoringRpc(SandboxClient::default());
    let (mut common, mut nodes) = gen_sandbox_anchoring_config(&mut client);

    let priv_keys = priv_keys.into_iter().collect::<Vec<_>>();
    // Change default anchoring configs
    common.frequency = ANCHORING_FREQUENCY;
    for &&(ref addr, ref keys) in &priv_keys {
        for (id, key) in keys.iter().enumerate() {
            nodes[id]
                .private_keys
                .insert(addr.to_string(),
                        btc::PrivateKey::from_base58check(key).unwrap());
        }
    }

    for node in &mut nodes {
        node.check_lect_frequency = CHECK_LECT_FREQUENCY;
    }

    client.expect(vec![request! {
            method: "importaddress",
            params: ["2NFGToas8B6sXqsmtGwL1H4kC5fGWSpTcYA", "multisig", false, false]
        }]);
    let service = AnchoringService::new_with_client(AnchoringRpc(client.clone()),
                                                    common.clone(),
                                                    nodes[ANCHORING_VALIDATOR as usize].clone());
    let service_handler = service.handler();
    let sandbox = sandbox_with_services(vec![Box::new(service),
                                             Box::new(TimestampingService::new()),
                                             Box::new(ConfigUpdateService::new())]);
    let info = AnchoringSandboxState {
        common: common,
        nodes: nodes,
        latest_anchored_tx: None,
        handler: service_handler,
    };
    (sandbox, client, info)
}
