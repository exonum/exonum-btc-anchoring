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
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::mpsc;

use rand::{SeedableRng, StdRng};
use serde_json;

use exonum::crypto::Hash;
use exonum::blockchain::{Schema, Transaction};
use exonum::helpers::{Height, ValidatorId};
use exonum_testkit::{TestKit, TestKitBuilder};

use {gen_anchoring_testnet_config_with_rng, AnchoringConfig, AnchoringNodeConfig,
     AnchoringService, ANCHORING_SERVICE_NAME};
use details::btc;
use details::btc::transactions::{AnchoringTx, FundingTx, TransactionBuilder};
use blockchain::dto::MsgAnchoringSignature;
use handler::{collect_signatures, AnchoringHandler};
use error::HandlerError;
pub use self::rpc::{TestClient, TestRequest, TestRequests};

#[macro_use]
mod macros;
mod rpc;
pub mod secp256k1_hack;
mod helpers;
mod test_anchoring;
mod test_auditing;
mod test_transition;
mod test_api;

pub const ANCHORING_FREQUENCY: u64 = 10;
pub const ANCHORING_UTXO_CONFIRMATIONS: u64 = 24;
pub const ANCHORING_FUNDS: u64 = 4000;
pub const CHECK_LECT_FREQUENCY: u64 = 6;

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

        client.requests().expect(vec![
            request! {
                method: "importaddress",
                params: ["2NFGToas8B6sXqsmtGwL1H4kC5fGWSpTcYA", "multisig", false, false]
            },
        ]);
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
            handler: handler,
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
            let (prev_tx, prev_tx_input) = self.latest_anchored_tx
                .clone()
                .map(|x| {
                    let tx = (x.0).0;
                    let input = 0;
                    (tx, input)
                })
                .unwrap_or_else(|| {
                    let cfg = self.current_cfg();
                    let tx = cfg.funding_tx().clone();
                    let input = tx.find_out(&cfg.redeem_script().1).unwrap();
                    (tx.0.clone(), input)
                });

            let mut builder = TransactionBuilder::with_prev_tx(&prev_tx, prev_tx_input)
                .payload(height, block_hash)
                .prev_tx_chain(prev_tx_chain)
                .send_to(addr.clone())
                .fee(1000);
            for fund in funds {
                let out = fund.find_out(addr).unwrap();
                builder = builder.add_funds(fund, out);
            }

            let tx = builder.into_transaction().unwrap();
            let signs = self.gen_anchoring_signatures(&tx);
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

    pub fn gen_anchoring_signatures(&self, tx: &AnchoringTx) -> Vec<MsgAnchoringSignature> {
        let (redeem_script, addr) = self.current_cfg().redeem_script();

        let priv_keys = self.priv_keys(&addr);
        let mut signs = Vec::new();
        for (validator, priv_key) in priv_keys.iter().enumerate() {
            let validator = ValidatorId(validator as u16);
            for input in tx.inputs() {
                let signature = tx.sign_input(&redeem_script, input, priv_key);
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
fn test_anchoring_testkit() {
    AnchoringTestKit::default();
}
