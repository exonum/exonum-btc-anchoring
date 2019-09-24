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

use bitcoin::{self, network::constants::Network};
use bitcoin_hashes::{sha256d::Hash as Sha256dHash, Hash as BitcoinHash};
use btc_transaction_utils::{p2wsh, TxInRef};
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
use rand::{thread_rng, Rng};

use std::collections::BTreeMap;

use crate::{
    api::{BlockHeaderProof, FindTransactionQuery, HeightQuery, PublicApi, TransactionProof},
    blockchain::{transactions::TxSignature, BtcAnchoringSchema},
    btc,
    config::Config,
    proto::AnchoringKeys,
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

#[derive(Debug, Default)]
struct AnchoringNodes {
    key_pool: BTreeMap<btc::PublicKey, btc::PrivateKey>,
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
        self.key_pool.insert(btc_keypair.0, btc_keypair.1);
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
        self.key_pool[pk].clone()
    }
}

#[derive(Debug)]
pub struct AnchoringTestKit {
    pub inner: TestKit,
    anchoring_nodes: AnchoringNodes,
}

impl AnchoringTestKit {
    pub fn new(nodes_num: u16, total_funds: u64, anchoring_interval: u64) -> Self {
        let validator_keys = (0..nodes_num)
            .map(|_| gen_validator_keys())
            .collect::<Vec<_>>();

        let network = Network::Testnet;
        let anchoring_nodes = AnchoringNodes::from_keys(Network::Testnet, &validator_keys);

        let mut anchoring_config = Config {
            network,
            anchoring_keys: anchoring_nodes.anchoring_keys(),
            anchoring_interval,
            ..Config::default()
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

    pub fn actual_anchoring_config(&self) -> Config {
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

    pub fn gen_bitcoin_key(&mut self) -> btc::PublicKey {
        let keypair = btc::gen_keypair(self.actual_anchoring_config().network);
        self.anchoring_nodes.key_pool.insert(keypair.0, keypair.1);
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

impl Default for AnchoringTestKit {
    fn default() -> Self {
        Self::new(4, 70000, 5)
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
