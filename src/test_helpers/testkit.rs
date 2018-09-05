// Copyright 2018 The Exonum Team
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

use bitcoin::{self, network::constants::Network, util::address::Address};
use btc_transaction_utils::{multisig::RedeemScript, p2wsh, TxInRef};
use rand::{thread_rng, Rng, SeedableRng, StdRng};

use exonum::blockchain::Transaction;
use exonum::encoding::serialize::FromHex;
use exonum_testkit::{TestKit, TestKitBuilder, TestNetworkConfiguration, TestNode};

use std::env;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::{Arc, RwLock};

use {
    blockchain::{transactions::Signature, BtcAnchoringSchema, BtcAnchoringState},
    btc,
    config::{GlobalConfig, LocalConfig},
    rpc::{BitcoinRpcClient, BitcoinRpcConfig, BtcRelay},
    service::KeyPool,
    test_helpers::rpc::*,
    BtcAnchoringService, BTC_ANCHORING_SERVICE_NAME,
};

/// Generates a fake funding transaction.
pub fn create_fake_funding_transaction(address: &bitcoin::Address, value: u64) -> btc::Transaction {
    // Generates random transaction id
    let mut rng = thread_rng();
    let mut data = [0u8; 32];
    rng.fill_bytes(&mut data);
    // Creates fake funding transaction
    let tx = bitcoin::Transaction {
        version: 2,
        lock_time: 0,
        input: vec![bitcoin::TxIn {
            previous_output: bitcoin::OutPoint {
                vout: 0,
                txid: ::bitcoin::util::hash::Sha256dHash::from_data(&data),
            },
            script_sig: bitcoin::Script::new(),
            sequence: 0,
            witness: vec![],
        }],
        output: vec![bitcoin::TxOut {
            value,
            script_pubkey: address.script_pubkey(),
        }],
    }.into();
    tx
}

pub fn gen_anchoring_config<R: Rng>(
    rpc: Option<&dyn BtcRelay>,
    network: Network,
    count: u16,
    total_funds: u64,
    anchoring_interval: u64,
    rng: &mut R,
) -> (GlobalConfig, Vec<LocalConfig>) {
    let count = count as usize;
    let (public_keys, private_keys): (Vec<_>, Vec<_>) = (0..count)
        .map(|_| btc::gen_keypair_with_rng(network, rng))
        .unzip();

    let mut global = GlobalConfig {
        network,
        public_keys,
        funding_transaction: None,
        anchoring_interval,
        ..Default::default()
    };

    let address = global.anchoring_address();
    let local_cfgs = private_keys
        .iter()
        .map(|sk| LocalConfig {
            rpc: rpc.map(BtcRelay::config),
            private_keys: hashmap!{ address.clone() => sk.clone() },
        })
        .collect();

    let tx = if let Some(rpc) = rpc {
        rpc.watch_address(&address, false).unwrap();
        rpc.send_to_address(&address, total_funds).unwrap()
    } else {
        create_fake_funding_transaction(
            &p2wsh::address(&global.redeem_script(), global.network),
            total_funds,
        )
    };

    global.funding_transaction = Some(tx);
    (global, local_cfgs)
}

// Notorious test kit wrapper
#[derive(Debug)]
pub struct AnchoringTestKit {
    inner: TestKit,
    pub local_private_keys: KeyPool,
    pub node_configs: Vec<LocalConfig>,
    requests: Option<TestRequests>,
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

impl AnchoringTestKit {
    pub fn new<R: Rng>(
        rpc: Option<Box<dyn BtcRelay>>,
        validators_num: u16,
        total_funds: u64,
        anchoring_interval: u64,
        mut rng: R,
        requests: Option<TestRequests>,
    ) -> Self {
        let network = Network::Testnet;
        let (global, locals) = gen_anchoring_config(
            rpc.as_ref().map(|rpc| &**rpc),
            network,
            validators_num,
            total_funds,
            anchoring_interval,
            &mut rng,
        );

        let local = locals[0].clone();
        let private_keys = Arc::new(RwLock::new(local.private_keys));
        let service = BtcAnchoringService::new(global.clone(), Arc::clone(&private_keys), rpc);

        let testkit = TestKitBuilder::validator()
            .with_service(service)
            .with_validators(validators_num)
            .with_logger()
            .create();

        Self {
            inner: testkit,
            local_private_keys: private_keys,
            node_configs: locals,
            requests,
        }
    }

    pub fn new_with_testnet(
        validators_num: u16,
        total_funds: u64,
        anchoring_interval: u64,
    ) -> Self {
        let rng = thread_rng();

        let rpc_config = BitcoinRpcConfig {
            host: env::var("ANCHORING_RELAY_HOST")
                .unwrap_or_else(|_| String::from("http://127.0.0.1:18332")),
            username: env::var("ANCHORING_USER")
                .ok()
                .or_else(|| Some(String::from("testnet"))),
            password: env::var("ANCHORING_PASSWORD")
                .ok()
                .or_else(|| Some(String::from("testnet"))),
        };

        let client = BitcoinRpcClient::from(rpc_config);
        Self::new(
            Some(Box::from(client)),
            validators_num,
            total_funds,
            anchoring_interval,
            rng,
            None,
        )
    }

    pub fn new_with_fake_rpc(anchoring_interval: u64) -> Self {
        let validators_num = 4;
        let total_funds = 7_000;

        let seed: &[_] = &[1, 2, 3, 9];
        let rng: StdRng = SeedableRng::from_seed(seed);
        let fake_relay = FakeBtcRelay::default();
        let requests = fake_relay.requests.clone();

        let addr = Address::from_str(
            "tb1q8270svuaqety59gegtp4ujjeam39s83csz7whp9ryn3zxlcee66setkyq0",
        ).unwrap();
        requests.expect(vec![
            (
                FakeRelayRequest::WatchAddress {
                    addr: addr.clone(),
                    rescan: false,
                },
                FakeRelayResponse::WatchAddress(Ok(())),
            ),
            (
                FakeRelayRequest::SendToAddress {
                    addr: addr.clone(),
                    satoshis: total_funds,
                },
                FakeRelayResponse::SendToAddress(btc::Transaction::from_hex(
                    "02000000000101140b3f5da041f173d938b8fe778d39cb2ef801f75f\
                     2946e490e34d6bb47bb9ce0000000000feffffff0230025400000000\
                     00160014169fa44a9159f281122bb7f3d43d88d56dfa937e70110100\
                     000000002200203abcf8339d06564a151942c35e4a59eee2581e3880\
                     bceb84a324e2237f19ceb502483045022100e91d46b565f26641b353\
                     591d0c403a05ada5735875fb0f055538bf9df4986165022044b53367\
                     72de8c5f6cbf83bcc7099e31d7dce22ba1f3d1badc2fdd7f8013a122\
                     01210254053f15b44b825bc5dabfe88f8b94cd217372f3f297d2696a\
                     32835b43497397358d1400",
                )),
            ),
        ]);

        Self::new(
            Some(Box::from(fake_relay)),
            validators_num,
            total_funds,
            anchoring_interval,
            rng,
            Some(requests.clone()),
        )
    }

    pub fn new_without_rpc(validators_num: u16, total_funds: u64, anchoring_interval: u64) -> Self {
        let seed: &[_] = &[1, 2, 3, 9];
        let rng: StdRng = SeedableRng::from_seed(seed);

        Self::new(
            None,
            validators_num,
            total_funds,
            anchoring_interval,
            rng,
            None,
        )
    }

    pub fn renew_address(&mut self) {
        let schema = BtcAnchoringSchema::new(self.snapshot());

        if let BtcAnchoringState::Transition {
            actual_configuration,
            following_configuration,
        } = schema.actual_state()
        {
            let old_addr = actual_configuration.anchoring_address();
            let new_addr = following_configuration.anchoring_address();

            let pk = {
                let private_keys = self.local_private_keys.read().unwrap();
                private_keys.get(&old_addr).unwrap().clone()
            };

            if old_addr != new_addr {
                trace!("setting new pkey for addr {:?} ", new_addr);
                let mut private_keys = self.local_private_keys.write().unwrap();
                private_keys.insert(new_addr.clone(), pk.clone());

                for local_cfg in &mut self.node_configs.iter_mut() {
                    let pk = local_cfg.private_keys[&old_addr].clone();
                    local_cfg.private_keys.insert(new_addr.clone(), pk);
                }
            }
        }
    }

    fn get_local_cfg(&self, node: &TestNode) -> LocalConfig {
        self.node_configs[node.validator_id().unwrap().0 as usize].clone()
    }

    pub fn anchoring_us(&self) -> (TestNode, LocalConfig) {
        let node = self.inner.us();
        let cfg = self.get_local_cfg(node);
        (node.clone(), cfg)
    }

    pub fn anchoring_validators(&self) -> Vec<(TestNode, LocalConfig)> {
        let validators = self.inner.network().validators();
        validators
            .into_iter()
            .map(|validator| (validator.clone(), self.get_local_cfg(validator)))
            .collect::<Vec<(TestNode, LocalConfig)>>()
    }

    pub fn redeem_script(&self) -> RedeemScript {
        let fork = self.blockchain().fork();
        let schema = BtcAnchoringSchema::new(fork);

        schema.actual_state().actual_configuration().redeem_script()
    }

    pub fn anchoring_address(&self) -> btc::Address {
        let fork = self.blockchain().fork();
        let schema = BtcAnchoringSchema::new(fork);

        schema
            .actual_state()
            .actual_configuration()
            .anchoring_address()
    }

    pub fn rpc_client(&self) -> BitcoinRpcClient {
        let rpc_cfg = self.get_local_cfg(self.us()).rpc.unwrap();
        BitcoinRpcClient::from(rpc_cfg)
    }

    pub fn last_anchoring_tx(&self) -> Option<btc::Transaction> {
        let schema = BtcAnchoringSchema::new(self.snapshot());
        schema.anchoring_transactions_chain().last()
    }

    pub fn create_signature_tx_for_validators(
        &self,
        validators_num: u16,
    ) -> Result<Vec<Box<dyn Transaction>>, btc::BuilderError> {
        let validators = self
            .network()
            .validators()
            .iter()
            .filter(|v| v != &self.us())
            .take(validators_num as usize);

        let mut signatures: Vec<Box<Transaction>> = vec![];

        let redeem_script = self.redeem_script();
        let mut signer = p2wsh::InputSigner::new(redeem_script.clone());

        for validator in validators {
            let validator_id = validator.validator_id().unwrap();
            let (public_key, private_key) = validator.service_keypair();

            let schema = BtcAnchoringSchema::new(self.snapshot());

            if let Some(p) = schema.actual_proposed_anchoring_transaction() {
                let (proposal, proposal_inputs) = p?;

                let address = schema.actual_state().output_address();
                let privkey = &self.node_configs[validator_id.0 as usize].private_keys[&address];

                for (index, proposal_input) in proposal_inputs.iter().enumerate() {
                    let signature = signer
                        .sign_input(
                            TxInRef::new(proposal.as_ref(), index),
                            proposal_input.as_ref(),
                            privkey.0.secret_key(),
                        )
                        .unwrap();

                    let tx = Signature::new(
                        &public_key,
                        validator_id,
                        proposal.clone(),
                        index as u32,
                        signature.as_ref(),
                        &private_key,
                    );
                    signatures.push(tx.into());
                }
            }
        }
        Ok(signatures)
    }

    pub fn drop_validator_proposal(&mut self) -> TestNetworkConfiguration {
        let mut proposal = self.configuration_change_proposal();
        let mut validators = proposal.validators().to_vec();

        validators.pop();
        proposal.set_validators(validators);

        let config: GlobalConfig = proposal.service_config(BTC_ANCHORING_SERVICE_NAME);

        let mut keys = config.public_keys.clone();

        keys.pop();

        let service_configuration = GlobalConfig {
            public_keys: keys,
            ..config
        };
        proposal.set_service_config(BTC_ANCHORING_SERVICE_NAME, service_configuration);
        proposal
    }

    pub fn requests(&mut self) -> TestRequests {
        self.requests.clone().unwrap().clone()
    }
}
