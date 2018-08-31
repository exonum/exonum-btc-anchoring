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

use std::ops::{Deref, DerefMut};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};

use rand::{SeedableRng, StdRng};
use serde_json;

use exonum::blockchain::{Schema, Transaction};
use exonum::crypto::Hash;
use exonum::helpers::{Height, ValidatorId};
use exonum_testkit::{TestKit, TestKitBuilder};

pub use self::rpc::{TestClient, TestRequest, TestRequests};
use exonum_btc_anchoring::blockchain::dto::MsgAnchoringSignature;
use exonum_btc_anchoring::details::btc;
use exonum_btc_anchoring::details::btc::transactions::{
    AnchoringTx, FundingTx, RawBitcoinTx, TransactionBuilder,
};
use exonum_btc_anchoring::error::HandlerError;
use exonum_btc_anchoring::handler::{collect_signatures, AnchoringHandler};
use exonum_btc_anchoring::{
    gen_anchoring_testnet_config_with_rng, AnchoringConfig, AnchoringNodeConfig, AnchoringService,
    ANCHORING_SERVICE_NAME,
};

#[macro_use]
mod macros;
pub mod helpers;
mod rpc;
pub mod secp256k1_hack;

pub const ANCHORING_FREQUENCY: u64 = 10;
pub const ANCHORING_UTXO_CONFIRMATIONS: u64 = 24;
pub const ANCHORING_FUNDS: u64 = 4000;
pub const CHECK_LECT_FREQUENCY: u64 = 6;

#[derive(Debug)]
pub struct AnchoringTestKit {
    inner: TestKit,
    requests: TestRequests,
    handler: Arc<Mutex<AnchoringHandler>>,
    errors_receiver: mpsc::Receiver<HandlerError>,
    nodes: Vec<AnchoringNodeConfig>,
    latest_anchored_tx: Option<(AnchoringTx, Vec<MsgAnchoringSignature>)>,
}

impl Deref for AnchoringTestKit {
    type Target = TestKit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for AnchoringTestKit {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Default for AnchoringTestKit {
    fn default() -> Self {
        AnchoringTestKit::new()
    }
}

impl AnchoringTestKit {
    pub fn new() -> AnchoringTestKit {
        let mut client = TestClient::default();
        let (mut common, mut nodes) = gen_sandbox_anchoring_config(&mut client);
        // Change default anchoring configs
        common.frequency = ANCHORING_FREQUENCY;
        common.utxo_confirmations = ANCHORING_UTXO_CONFIRMATIONS;
        for node in &mut nodes {
            node.check_lect_frequency = CHECK_LECT_FREQUENCY;
        }

        client.requests().expect(vec![request! {
            method: "importaddress",
            params: [
                "tb1qn5mmecjkj4us6uhr5tc453k96hrzcwr3l9d8fkc7fg8zwur50y4qfdclp7",
                "multisig",
                false,
                false
            ]
        }]);
        let requests = client.requests();
        let service =
            AnchoringService::new_with_client(Box::new(client), common.clone(), nodes[0].clone());
        let handler = service.handler();
        let testkit = TestKitBuilder::validator()
            .with_validators(4)
            .with_service(service)
            .create();

        let (sender, receiver) = mpsc::channel();
        handler.lock().unwrap().set_errors_sink(Some(sender));

        AnchoringTestKit {
            inner: testkit,
            handler,
            requests,
            nodes,
            latest_anchored_tx: None,
            errors_receiver: receiver,
        }
    }

    pub fn requests(&self) -> TestRequests {
        self.requests.clone()
    }

    pub fn handler(&self) -> MutexGuard<AnchoringHandler> {
        self.handler.lock().unwrap()
    }

    pub fn take_handler_errors(&mut self) -> Vec<HandlerError> {
        self.errors_receiver.try_iter().collect()
    }

    pub fn priv_keys(&self, addr: &btc::Address) -> Vec<btc::PrivateKey> {
        self.nodes()
            .iter()
            .map(|cfg| cfg.private_keys[&addr.to_string()].clone())
            .collect::<Vec<_>>()
    }

    pub fn nodes(&self) -> &[AnchoringNodeConfig] {
        self.nodes.as_slice()
    }

    pub fn nodes_mut(&mut self) -> &mut Vec<AnchoringNodeConfig> {
        &mut self.nodes
    }

    pub fn current_priv_keys(&self) -> Vec<btc::PrivateKey> {
        self.priv_keys(&self.current_cfg().redeem_script().1)
    }

    pub fn current_cfg(&self) -> AnchoringConfig {
        let stored = self.actual_configuration();
        serde_json::from_value(stored.services[ANCHORING_SERVICE_NAME].clone()).unwrap()
    }

    pub fn current_redeem_script(&self) -> btc::RedeemScript {
        self.current_cfg().redeem_script().0
    }

    pub fn current_addr(&self) -> btc::Address {
        self.current_cfg().redeem_script().1
    }

    pub fn current_funding_tx(&self) -> FundingTx {
        self.current_cfg().funding_tx().clone()
    }

    pub fn block_hash_on_height(&self, height: Height) -> Hash {
        Schema::new(&self.snapshot())
            .block_hashes_by_height()
            .get(height.0)
            .unwrap()
    }

    pub fn next_check_lect_height(&self) -> Height {
        let height = self.height().next();
        let frequency = self.nodes[0].check_lect_frequency as u64;
        Height(height.0 - height.0 % frequency + frequency).previous()
    }

    pub fn next_anchoring_height(&self) -> Height {
        let height = self.height().next();
        let frequency = self.current_cfg().frequency as u64;
        Height(height.0 - height.0 % frequency + frequency).previous()
    }

    pub fn latest_anchoring_height(&self) -> Height {
        let height = self.height().next();
        self.current_cfg().latest_anchoring_height(height)
    }

    pub fn latest_anchored_tx(&self) -> AnchoringTx {
        self.latest_anchored_tx.as_ref().unwrap().0.clone()
    }

    pub fn set_latest_anchored_tx(
        &mut self,
        tx: Option<(AnchoringTx, Vec<MsgAnchoringSignature>)>,
    ) {
        self.latest_anchored_tx = tx;
    }

    pub fn latest_anchored_tx_signatures(&self) -> Vec<MsgAnchoringSignature> {
        self.latest_anchored_tx.as_ref().unwrap().1.clone()
    }

    pub fn gen_anchoring_tx_with_signatures(
        &mut self,
        height: Height,
        block_hash: Hash,
        funds: &[FundingTx],
        prev_tx_chain: Option<btc::TxId>,
        addr: &btc::Address,
    ) -> (AnchoringTx, Vec<Box<Transaction>>) {
        let (propose_tx, signed_tx, signs) = {
            let (prev_tx, prev_tx_input) = self
                .latest_anchored_tx
                .clone()
                .map(|x| {
                    let tx = (x.0).0;
                    let input = 0;
                    (tx, input)
                })
                .unwrap_or_else(|| {
                    let cfg = self.current_cfg();
                    let tx = cfg.funding_tx().clone();
                    let input = tx
                        .find_out(&cfg.redeem_script().1)
                        .expect("Unable to find output");
                    (tx.0.clone(), input)
                });

            let mut builder = TransactionBuilder::with_prev_tx(&prev_tx, prev_tx_input)
                .payload(height, block_hash)
                .prev_tx_chain(prev_tx_chain)
                .send_to(addr.clone())
                .fee(1000);

            let mut prev_txs = vec![prev_tx];
            for fund in funds {
                let out = fund.find_out(addr).unwrap();
                builder = builder.add_funds(fund, out);
                prev_txs.push(fund.0.clone());
            }

            let tx = builder.into_transaction().unwrap();
            let signs = self.gen_anchoring_signatures(&tx, &prev_txs);
            let signed_tx = self.finalize_tx(tx.clone(), signs.clone());
            (tx, signed_tx, signs)
        };
        self.latest_anchored_tx = Some((signed_tx, signs.clone()));

        (
            propose_tx,
            signs
                .into_iter()
                .map(|x| Box::new(x) as Box<Transaction>)
                .collect(),
        )
    }

    pub fn finalize_tx<I>(&self, tx: AnchoringTx, signs: I) -> AnchoringTx
    where
        I: IntoIterator<Item = MsgAnchoringSignature>,
    {
        let collected_signs = collect_signatures(&tx, &self.current_cfg(), signs).unwrap();
        tx.finalize(&self.current_redeem_script(), collected_signs)
    }

    pub fn gen_anchoring_signatures(
        &self,
        tx: &AnchoringTx,
        prev_txs: &[RawBitcoinTx],
    ) -> Vec<MsgAnchoringSignature> {
        let (redeem_script, addr) = self.current_cfg().redeem_script();

        let priv_keys = self.priv_keys(&addr);
        let mut signs = Vec::new();
        for (validator, priv_key) in priv_keys.iter().enumerate() {
            let validator = ValidatorId(validator as u16);
            for input in tx.inputs() {
                let prev_tx = &prev_txs[input as usize];
                let signature = tx.sign_input(&redeem_script, input, prev_tx, priv_key);
                let keypair = self.validator(validator).service_keypair();
                signs.push(MsgAnchoringSignature::new(
                    keypair.0,
                    validator,
                    tx.clone(),
                    input,
                    &signature,
                    keypair.1,
                ));
            }
        }
        signs
    }
}

/// Generates config for 4 validators and 4000 funds
fn gen_sandbox_anchoring_config(
    client: &mut TestClient,
) -> (AnchoringConfig, Vec<AnchoringNodeConfig>) {
    let requests = vec![
        request! {
            method: "importaddress",
            params: [
                "tb1qn5mmecjkj4us6uhr5tc453k96hrzcwr3l9d8fkc7fg8zwur50y4qfdclp7",
                "multisig",
                false,
                false
            ]
        },
        request! {
            method: "sendtoaddress",
            params: [
                "tb1qn5mmecjkj4us6uhr5tc453k96hrzcwr3l9d8fkc7fg8zwur50y4qfdclp7",
                "0.00004"
            ],
            response: "5d934477840e1dc36a8900ae91b02ab9e40e47d946e45f3e58288db760c4aeb1"
        },
        request! {
            method: "getrawtransaction",
            params: [
                "5d934477840e1dc36a8900ae91b02ab9e40e47d946e45f3e58288db760c4aeb1",
                0
            ],
            response: "020000000001010d7d0b800827c45ff80603c74d8aec4c62ca1ad12f4115ac1f18c2dba293d\
                25c00000000171600149cd7992b80bda416f5acff608dc1aa2cb7e41d28feffffff02a899700a00000\
                000160014e3dabb36a139f4d5d3712835929d41504a21f9c9a00f0000000000002200209d37bce2569\
                5790d72e3a2f15a46c5d5c62c3871f95a74db1e4a0e277074792a02483045022100bc572cd3b1e2fa8\
                f17487f965920946c940f860be595584a4c5fe273fa01b40502206317b780bbe7fe80ea240572967f7\
                d70530c99a381a794727adadcfc271f5955012103e876e56f29fb47eb260e63a26516079443f854769\
                ffae3fe572835ee6c8ed361c9bc1300"
        },
    ];
    client.requests().expect(requests);
    let mut rng: StdRng = SeedableRng::from_seed([1, 2, 3, 4].as_ref());
    gen_anchoring_testnet_config_with_rng(
        client,
        btc::Network::Testnet,
        4,
        ANCHORING_FUNDS,
        &mut rng,
    )
}

#[test]
fn test_generate_anchoring_config() {
    let mut client = TestClient::default();
    gen_sandbox_anchoring_config(&mut client);
}

#[test]
fn test_create_anchoring_testkit() {
    AnchoringTestKit::default();
}
