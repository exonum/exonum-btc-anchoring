// Copyright 2019 The Exonum Team
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

//! Helpers collection to test the service with the testkit.

use bitcoin::{self, network::constants::Network, util::address::Address};
use bitcoin_hashes::{sha256d::Hash as Sha256dHash, Hash as BitcoinHash};
use btc_transaction_utils::{multisig::RedeemScript, p2wsh, TxInRef};
use exonum::{
    api,
    blockchain::{BlockProof, ConsensusConfig, IndexCoordinates, IndexOwner, Schema as CoreSchema},
    crypto::{self, Hash, PublicKey},
    helpers::Height,
    keys::Keys,
    messages::{AnyTx, Verified},
    runtime::{rust::Transaction, InstanceId},
};
use exonum_merkledb::{MapProof, ObjectHash};
use exonum_testkit::{
    simple_supervisor::SimpleSupervisor, ApiKind, InstanceCollection, TestKit, TestKitApi,
    TestKitBuilder, TestNode,
};
use failure::{ensure, format_err};
use hex::FromHex;
use maplit::hashmap;
use rand::{thread_rng, Rng, SeedableRng, StdRng};

use std::{
    collections::{BTreeMap, HashMap},
    ops::{Deref, DerefMut},
    str::FromStr,
    sync::{Arc, RwLock},
};

use crate::{
    api::{BlockHeaderProof, FindTransactionQuery, HeightQuery, PublicApi, TransactionProof},
    blockchain::{transactions::TxSignature, BtcAnchoringSchema},
    btc,
    config::{GlobalConfig, LocalConfig},
    proto::AnchoringKeys,
    rpc::BtcRelay,
    service::KeyPool,
    test_helpers::rpc::*,
    BtcAnchoringService,
};

pub const ANCHORING_INSTANCE_ID: InstanceId = 14;
pub const ANCHORING_INSTANCE_NAME: &str = "btc_anchoring";

/// Generates a fake funding transaction.
pub fn create_fake_funding_transaction(address: &bitcoin::Address, value: u64) -> btc::Transaction {
    // Generates random transaction id
    let mut rng = thread_rng();
    let mut data = [0_u8; 32];
    rng.fill_bytes(&mut data);
    // Creates fake funding transaction
    bitcoin::Transaction {
        version: 2,
        lock_time: 0,
        input: vec![bitcoin::TxIn {
            previous_output: bitcoin::OutPoint {
                vout: 0,
                txid: Sha256dHash::from_slice(&data).unwrap(),
            },
            script_sig: bitcoin::Script::new(),
            sequence: 0,
            witness: vec![],
        }],
        output: vec![bitcoin::TxOut {
            value,
            script_pubkey: address.script_pubkey(),
        }],
    }
    .into()
}

/// Generates a complete anchoring configuration for the given arguments.
pub fn gen_anchoring_config<R: Rng>(
    rpc: Option<&dyn BtcRelay>,
    network: Network,
    service_keys: impl IntoIterator<Item = PublicKey>,
    total_funds: u64,
    anchoring_interval: u64,
    rng: &mut R,
) -> (GlobalConfig, Vec<LocalConfig>) {
    let (anchoring_keys, bitcoin_keys): (Vec<_>, Vec<_>) = service_keys
        .into_iter()
        .map(|service_key| {
            let btc_keypair = btc::gen_keypair_with_rng(rng, network);
            (
                AnchoringKeys {
                    bitcoin_key: btc_keypair.0,
                    service_key,
                },
                btc_keypair,
            )
        })
        .unzip();

    let mut global = GlobalConfig {
        network,
        anchoring_keys,
        funding_transaction: None,
        anchoring_interval,
        ..GlobalConfig::default()
    };

    let address = global.anchoring_address();
    let local_cfgs = bitcoin_keys
        .iter()
        .map(|(pk, sk)| LocalConfig {
            rpc: rpc.map(BtcRelay::config),
            private_keys: hashmap! { pk.clone() => sk.clone() },
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

fn gen_validator_keys() -> Keys {
    let consensus_keypair = crypto::gen_keypair();
    let service_keypair = crypto::gen_keypair();
    Keys::from_keys(
        consensus_keypair.0,
        consensus_keypair.1,
        service_keypair.0,
        service_keypair.1,
    )
}

/// The notorious testkit wrapper with extensions for anchoring service.
#[derive(Debug)]
pub struct AnchoringTestKit {
    /// Private bitcoin keys for the `us` node.
    pub local_private_keys: KeyPool,
    /// List of the local configs of the validators.
    pub node_configs: Vec<LocalConfig>,
    inner: TestKit,
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
    /// Creates a new testkit instance with the extensions for anchoring.
    fn new<R: Rng>(
        rpc: Option<Arc<dyn BtcRelay>>,
        validators_num: u16,
        total_funds: u64,
        anchoring_interval: u64,
        mut rng: R,
        requests: Option<TestRequests>,
    ) -> Self {
        let network = Network::Testnet;

        let validator_keys = (0..validators_num)
            .map(|_| {
                // TODO Make decision about rng.
                let consensus_keypair = crypto::gen_keypair();
                let service_keypair = crypto::gen_keypair();
                Keys::from_keys(
                    consensus_keypair.0,
                    consensus_keypair.1,
                    service_keypair.0,
                    service_keypair.1,
                )
            })
            .collect::<Vec<_>>();

        let (global, locals) = gen_anchoring_config(
            rpc.as_ref().map(|rpc| &**rpc),
            network,
            validator_keys.iter().map(Keys::service_pk),
            total_funds,
            anchoring_interval,
            &mut rng,
        );

        let local = locals[0].clone();
        let private_keys = Arc::new(RwLock::new(local.private_keys));

        let testkit = TestKitBuilder::validator()
            .with_service(InstanceCollection::new(BtcAnchoringService).with_instance(
                ANCHORING_INSTANCE_ID,
                ANCHORING_INSTANCE_NAME,
                global,
            ))
            .with_keys(validator_keys)
            .with_logger()
            .create();

        Self {
            inner: testkit,
            local_private_keys: private_keys,
            node_configs: locals,
            requests,
        }
    }

    /// Creates an anchoring testkit for the four validators with the fake rpc client
    /// under the hood.
    pub fn new_with_fake_rpc(anchoring_interval: u64) -> Self {
        let validators_num = 4;
        let total_funds = 7_000;

        let seed: &[_] = &[1, 2, 3, 9];
        let rng: StdRng = SeedableRng::from_seed(seed);
        let fake_relay = FakeBtcRelay::default();
        let requests = fake_relay.requests.clone();

        let addr =
            Address::from_str("tb1q8270svuaqety59gegtp4ujjeam39s83csz7whp9ryn3zxlcee66setkyq0")
                .unwrap();
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
            Some(Arc::new(fake_relay)),
            validators_num,
            total_funds,
            anchoring_interval,
            rng,
            Some(requests.clone()),
        )
    }

    /// Creates an anchoring testkit without rpc client.
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

    /// Updates the private keys pool in testkit for the transition state.
    pub fn renew_address(&mut self) {
        unimplemented!()

        // let snapshot = self.snapshot();
        // let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);

        // if let BtcAnchoringState::Transition {
        //     actual_configuration,
        //     following_configuration,
        // } = schema.actual_state()
        // {
        //     let old_addr = actual_configuration.anchoring_address();
        //     let new_addr = following_configuration.anchoring_address();

        //     let pk = {
        //         let private_keys = self.local_private_keys.read().unwrap();
        //         private_keys.get(&old_addr).unwrap().clone()
        //     };

        //     if old_addr != new_addr {
        //         trace!("Setting new pkey for addr {:?} ", new_addr);
        //         let mut private_keys = self.local_private_keys.write().unwrap();
        //         private_keys.insert(new_addr.clone(), pk.clone());

        //         for local_cfg in &mut self.node_configs.iter_mut() {
        //             let pk = local_cfg.private_keys[&old_addr].clone();
        //             local_cfg.private_keys.insert(new_addr.clone(), pk);
        //         }
        //     }
        // }
    }

    /// Returns the node in the emulated network, from whose perspective the testkit operates and
    /// its local anchoring configuration.
    pub fn anchoring_us(&self) -> (TestNode, LocalConfig) {
        let node = self.inner.us();
        let cfg = self.get_local_cfg(&node);
        (node.clone(), cfg)
    }

    /// Returns the current list of validators with their local anchoring configuration.
    pub fn anchoring_validators(&self) -> Vec<(TestNode, LocalConfig)> {
        let validators = self.inner.network().validators();
        validators
            .iter()
            .map(|validator| (validator.clone(), self.get_local_cfg(validator)))
            .collect::<Vec<(TestNode, LocalConfig)>>()
    }

    /// Returns the redeem script for the current anchoring configuration.
    pub fn redeem_script(&self) -> RedeemScript {
        self.actual_anchoring_configuration().redeem_script()
    }

    /// Returns the current anchoring global configuration.
    pub fn actual_anchoring_configuration(&self) -> GlobalConfig {
        let snapshot = self.blockchain().snapshot();
        let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
        schema.actual_configuration()
    }

    /// Returns the anchoring address for the current anchoring configuration.
    pub fn anchoring_address(&self) -> btc::Address {
        self.actual_anchoring_configuration().anchoring_address()
    }

    /// Returns the latest anchoring transaction.
    pub fn last_anchoring_tx(&self) -> Option<btc::Transaction> {
        let snapshot = self.snapshot();
        let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
        schema.anchoring_transactions_chain().last()
    }

    /// Creates signature transactions for the actual proposed anchoring transaction
    /// for the given number of validators.
    pub fn create_signature_tx_for_validators(
        &self,
        validators_num: u16,
    ) -> Result<Vec<Verified<AnyTx>>, btc::BuilderError> {
        let snapshot = self.snapshot();

        let validators = self
            .network()
            .validators()
            .into_iter()
            .filter(|v| v != &self.us())
            .take(validators_num as usize);

        let mut signatures = Vec::new();

        let redeem_script = self.redeem_script();
        let mut signer = p2wsh::InputSigner::new(redeem_script.clone());

        for validator in validators {
            let validator_id = validator.validator_id().unwrap();
            let (public_key, private_key) = validator.service_keypair();

            let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);

            if let Some(p) = schema.actual_proposed_anchoring_transaction() {
                let (proposal, proposal_inputs) = p?;

                let (anchoring_id, bitcoin_key) = schema
                    .actual_state()
                    .actual_configuration()
                    .find_bitcoin_key(&public_key)
                    .unwrap();
                let btc_private_key = &self.node_configs[anchoring_id].private_keys[&bitcoin_key];

                for (index, proposal_input) in proposal_inputs.iter().enumerate() {
                    let signature = signer
                        .sign_input(
                            TxInRef::new(proposal.as_ref(), index),
                            proposal_input.as_ref(),
                            &btc_private_key.0.key,
                        )
                        .unwrap();

                    signatures.push(
                        TxSignature {
                            transaction: proposal.clone(),
                            input: index as u32,
                            input_signature: signature.into(),
                        }
                        .sign(
                            ANCHORING_INSTANCE_ID,
                            public_key,
                            &private_key,
                        ),
                    );
                }
            }
        }
        Ok(signatures)
    }

    /// Creates a configuration change proposal which excludes
    /// one of validators from the consensus.
    pub fn drop_validator_proposal(&mut self) {
        unimplemented!()
        // let mut proposal = self.configuration_change_proposal();
        // let mut validators = proposal.validators().to_vec();

        // validators.pop();
        // proposal.set_validators(validators);

        // let config: GlobalConfig = proposal.service_config(ANCHORING_INSTANCE_NAME);

        // let mut keys = config.public_keys.clone();

        // keys.pop();

        // let service_configuration = GlobalConfig {
        //     public_keys: keys,
        //     ..config
        // };
        // proposal.set_service_config(ANCHORING_INSTANCE_NAME, service_configuration);
        // proposal
    }

    /// Returns the list of expected requests to the fake rpc.
    pub fn requests(&mut self) -> TestRequests {
        self.requests.clone().unwrap().clone()
    }

    /// Returns the block hash for the given height.
    pub fn block_hash_on_height(&self, height: Height) -> Hash {
        CoreSchema::new(&self.snapshot())
            .block_hashes_by_height()
            .get(height.0)
            .unwrap()
    }

    /// Returns the current snapshot of the btc anchoring information schema.
    // pub fn schema(&self) -> BtcAnchoringSchema<Arc<Box<dyn Snapshot>>> {
    //     let snapshot = Arc::new(self.inner.snapshot());
    //     BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, snapshot)
    // }

    fn get_local_cfg(&self, node: &TestNode) -> LocalConfig {
        self.node_configs[node.validator_id().unwrap().0 as usize].clone()
    }
}

impl PublicApi for TestKitApi {
    type Error = api::Error;

    fn actual_address(&self) -> Result<btc::Address, Self::Error> {
        self.public(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .get("v1/address/actual")
    }

    fn following_address(&self) -> Result<Option<btc::Address>, Self::Error> {
        self.public(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .get("v1/address/following")
    }

    fn find_transaction(
        &self,
        height: Option<Height>,
    ) -> Result<Option<TransactionProof>, Self::Error> {
        self.public(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .query(&FindTransactionQuery { height })
            .get("v1/transaction")
    }

    fn block_header_proof(&self, height: Height) -> Result<BlockHeaderProof, Self::Error> {
        self.public(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .query(&HeightQuery { height })
            .get("v1/block_header_proof")
    }
}

fn validate_table_proof(
    actual_config: &ConsensusConfig,
    latest_authorized_block: &BlockProof,
    to_table: MapProof<IndexCoordinates, Hash>,
) -> Result<(IndexCoordinates, Hash), failure::Error> {
    // Checks precommits.
    for precommit in &latest_authorized_block.precommits {
        let validator_id = precommit.as_ref().validator.0 as usize;
        let _validator_keys = actual_config
            .validator_keys
            .get(validator_id)
            .ok_or_else(|| {
                format_err!(
                    "Unable to find validator with the given id: {}",
                    validator_id
                )
            })?;
        ensure!(
            precommit.as_ref().block_hash() == &latest_authorized_block.block.object_hash(),
            "Block hash doesn't match"
        );
    }

    // Checks state_hash.
    let checked_table_proof = to_table.check()?;
    ensure!(
        checked_table_proof.root_hash() == *latest_authorized_block.block.state_hash(),
        "State hash doesn't match"
    );
    let value = checked_table_proof.entries().map(|(a, b)| (*a, *b)).next();
    value.ok_or_else(|| format_err!("Unable to get `to_block_header` entry"))
}

/// Proof validation extension.
pub trait ValidateProof {
    /// Output value.
    type Output;
    /// Perform the proof validation procedure with the given exonum blockchain configuration.
    fn validate(self, actual_config: &ConsensusConfig) -> Result<Self::Output, failure::Error>;
}

impl ValidateProof for TransactionProof {
    type Output = (u64, btc::Transaction);

    fn validate(self, actual_config: &ConsensusConfig) -> Result<Self::Output, failure::Error> {
        let proof_entry =
            validate_table_proof(actual_config, &self.latest_authorized_block, self.to_table)?;
        let table_location = IndexCoordinates::new(IndexOwner::Service(ANCHORING_INSTANCE_ID), 0);

        ensure!(proof_entry.0 == table_location, "Invalid table location");
        // Validates value.
        let values = self
            .to_transaction
            .validate(proof_entry.1)
            .map_err(|e| format_err!("An error occurred {:?}", e))?;
        ensure!(values.len() == 1, "Invalid values count");

        Ok((values[0].0, values[0].1.clone()))
    }
}

impl ValidateProof for BlockHeaderProof {
    type Output = (u64, Hash);

    fn validate(self, actual_config: &ConsensusConfig) -> Result<Self::Output, failure::Error> {
        let proof_entry =
            validate_table_proof(actual_config, &self.latest_authorized_block, self.to_table)?;
        let table_location = IndexCoordinates::new(IndexOwner::Service(ANCHORING_INSTANCE_ID), 3);
        ensure!(proof_entry.0 == table_location, "Invalid table location");
        // Validates value.
        let values = self
            .to_block_header
            .validate(proof_entry.1)
            .map_err(|e| format_err!("An error occurred {:?}", e))?;
        ensure!(values.len() == 1, "Invalid values count");
        Ok((values[0].0, values[0].1))
    }
}

#[derive(Debug, Default)]
struct AnchoringNodes {
    key_pool: KeyPool,
    inner: BTreeMap<PublicKey, btc::PublicKey>,
}

impl AnchoringNodes {
    fn from_keys(network: Network, keys: &[Keys]) -> Self {
        let mut nodes = Self::default();
        keys.iter().map(Keys::service_pk).for_each(|sk| {
            nodes.add_node(network, sk);
        });
        nodes
    }

    fn add_node(&mut self, network: Network, service_key: PublicKey) -> btc::PublicKey {
        let btc_keypair = btc::gen_keypair(network);
        self.key_pool
            .write()
            .unwrap()
            .insert(btc_keypair.0, btc_keypair.1);
        self.inner.insert(service_key, btc_keypair.0);
        btc_keypair.0
    }

    fn anchoring_keys(&self) -> Vec<AnchoringKeys> {
        self.inner
            .iter()
            .map(|(&service_key, &bitcoin_key)| AnchoringKeys {
                bitcoin_key,
                service_key,
            })
            .collect()
    }

    fn private_key(&self, pk: &btc::PublicKey) -> btc::PrivateKey {
        self.key_pool.read().unwrap()[pk].clone()
    }
}

#[derive(Debug)]
pub struct AnchoringTestKit2 {
    pub inner: TestKit,
    anchoring_nodes: AnchoringNodes,
}

impl AnchoringTestKit2 {
    pub fn new(nodes_num: u16, total_funds: u64, anchoring_interval: u64) -> Self {
        let validator_keys = (0..nodes_num)
            .map(|_| gen_validator_keys())
            .collect::<Vec<_>>();

        let network = Network::Testnet;
        let anchoring_nodes = AnchoringNodes::from_keys(Network::Testnet, &validator_keys);

        let mut anchoring_config = GlobalConfig {
            network,
            anchoring_keys: anchoring_nodes.anchoring_keys(),
            anchoring_interval,
            ..GlobalConfig::default()
        };
        anchoring_config.funding_transaction = Some(create_fake_funding_transaction(
            &anchoring_config.anchoring_address(),
            total_funds,
        ));

        let inner = TestKitBuilder::validator()
            .with_keys(validator_keys)
            .with_logger()
            .with_service(SimpleSupervisor)
            .with_service(InstanceCollection::new(BtcAnchoringService).with_instance(
                ANCHORING_INSTANCE_ID,
                ANCHORING_INSTANCE_NAME,
                anchoring_config,
            ))
            .create();

        Self {
            inner,
            anchoring_nodes,
        }
    }

    pub fn actual_anchoring_config(&self) -> GlobalConfig {
        let snapshot = self.inner.snapshot();
        let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
        schema.actual_configuration()
    }

    /// Return the latest anchoring transaction.
    pub fn last_anchoring_tx(&self) -> Option<btc::Transaction> {
        let snapshot = self.inner.snapshot();
        let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
        schema.anchoring_transactions_chain().last()
    }

    /// Return the proposal of the next anchoring transaction for the actual anchoring state.
    pub fn anchoring_transaction_proposal(
        &self,
    ) -> Option<(btc::Transaction, Vec<btc::Transaction>)> {
        let snapshot = self.inner.snapshot();
        let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);
        schema
            .actual_proposed_anchoring_transaction()
            .map(Result::unwrap)
    }

    pub fn create_signature_tx_for_node(
        &self,
        node: &TestNode,
    ) -> Result<Vec<Verified<AnyTx>>, btc::BuilderError> {
        let service_keypair = node.service_keypair();
        let snapshot = self.inner.snapshot();
        let schema = BtcAnchoringSchema::new(ANCHORING_INSTANCE_NAME, &snapshot);

        let mut signatures = Vec::new();
        if let Some(p) = schema.actual_proposed_anchoring_transaction() {
            let (proposal, proposal_inputs) = p?;

            let actual_config = schema.actual_state().actual_configuration().clone();
            let bitcoin_key = actual_config
                .find_bitcoin_key(&service_keypair.0)
                .unwrap()
                .1;
            let btc_private_key = self.anchoring_nodes.private_key(&bitcoin_key);

            let redeem_script = actual_config.redeem_script();
            let mut signer = p2wsh::InputSigner::new(redeem_script.clone());
            for (index, proposal_input) in proposal_inputs.iter().enumerate() {
                let signature = signer
                    .sign_input(
                        TxInRef::new(proposal.as_ref(), index),
                        proposal_input.as_ref(),
                        &btc_private_key.0.key,
                    )
                    .unwrap();

                signatures.push(
                    TxSignature {
                        transaction: proposal.clone(),
                        input: index as u32,
                        input_signature: signature.into(),
                    }
                    .sign(
                        ANCHORING_INSTANCE_ID,
                        service_keypair.0,
                        &service_keypair.1,
                    ),
                );
            }
        }
        Ok(signatures)
    }

    pub fn create_signature_txs(&self) -> Vec<Vec<Verified<AnyTx>>> {
        let mut signatures = Vec::new();

        for anchoring_keys in self.actual_anchoring_config().anchoring_keys {
            let node = self
                .find_node_by(|node| node.service_keypair().0 == anchoring_keys.service_key)
                .unwrap();

            signatures.push(self.create_signature_tx_for_node(node).unwrap());
        }
        signatures
    }

    pub fn add_node(&mut self) -> AnchoringKeys {
        let service_key = self.inner.network_mut().add_node().service_keypair().0;
        let bitcoin_key = self
            .anchoring_nodes
            .add_node(self.actual_anchoring_config().network, service_key);

        AnchoringKeys {
            bitcoin_key,
            service_key,
        }
    }

    pub fn gen_bitcoin_key(&self) -> btc::PublicKey {
        let keypair = btc::gen_keypair(self.actual_anchoring_config().network);
        self.anchoring_nodes
            .key_pool
            .write()
            .unwrap()
            .insert(keypair.0, keypair.1);
        keypair.0
    }

    /// Return the block hash for the given blockchain height.
    pub fn block_hash_on_height(&self, height: Height) -> Hash {
        CoreSchema::new(&self.inner.snapshot())
            .block_hashes_by_height()
            .get(height.0)
            .unwrap()
    }

    fn find_node_by(&self, predicate: impl FnMut(&&TestNode) -> bool) -> Option<&TestNode> {
        self.inner.network().nodes().iter().find(predicate)
    }
}

impl Default for AnchoringTestKit2 {
    fn default() -> Self {
        Self::new(4, 70000, 5)
    }
}
