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

//! Set of helpers for btc anchoring testing.

use bitcoin::{self, network::constants::Network};
use bitcoin_hashes::{sha256d::Hash as Sha256dHash, Hash as BitcoinHash};
use btc_transaction_utils::{p2wsh, TxInRef};
use exonum::{
    blockchain::{config::InstanceInitParams, BlockProof, ConsensusConfig},
    crypto::{Hash, KeyPair, PublicKey},
    helpers::Height,
    keys::Keys,
    messages::{AnyTx, Verified},
    runtime::{InstanceId, SnapshotExt, SUPERVISOR_INSTANCE_ID},
};
use exonum_merkledb::{access::Access, MapProof, ObjectHash, Snapshot};
use exonum_rust_runtime::{api, ServiceFactory};
use exonum_supervisor::{ConfigPropose, Supervisor, SupervisorInterface};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder, TestNode};
use failure::{ensure, format_err};
use rand::{thread_rng, Rng};

use std::collections::BTreeMap;

use crate::{
    api::{
        AnchoringChainLength, AnchoringProposalState, FindTransactionQuery, IndexQuery, PrivateApi,
        PublicApi, TransactionProof,
    },
    blockchain::{AddFunds, BtcAnchoringInterface, Schema, SignInput},
    btc,
    config::Config,
    proto::AnchoringKeys,
    BtcAnchoringService,
};

/// Default anchoring instance ID.
pub const ANCHORING_INSTANCE_ID: InstanceId = 14;
/// Default anchoring instance name.
pub const ANCHORING_INSTANCE_NAME: &str = "btc_anchoring";

/// Generates a fake funding transaction.
pub fn create_fake_funding_transaction(address: &btc::Address, value: u64) -> btc::Transaction {
    // Generate random transaction id.
    let mut rng = thread_rng();
    let mut data = [0_u8; 32];
    rng.fill_bytes(&mut data);
    // Create fake funding transaction.
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
            script_pubkey: address.0.script_pubkey(),
        }],
    }
    .into()
}

fn gen_validator_keys() -> Keys {
    let consensus_keypair = KeyPair::random();
    let service_keypair = KeyPair::random();
    Keys::from_keys(consensus_keypair, service_keypair)
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

    fn anchoring_keypairs(&self) -> Vec<(btc::PublicKey, btc::PrivateKey)> {
        self.inner
            .iter()
            .map(|(_, &bitcoin_key)| (bitcoin_key, self.private_key(&bitcoin_key)))
            .collect()
    }

    fn private_key(&self, pk: &btc::PublicKey) -> btc::PrivateKey {
        self.key_pool[pk].clone()
    }
}

/// Convenient wrapper around testkit with the built-in bitcoin key pool for the each
/// anchoring node.
#[derive(Debug)]
pub struct AnchoringTestKit {
    /// Underlying testkit instance.
    pub inner: TestKit,
    anchoring_nodes: AnchoringNodes,
}

/// Returns an anchoring schema instance used in Testkit.
pub fn get_anchoring_schema<'a>(snapshot: &'a dyn Snapshot) -> Schema<impl Access + 'a> {
    Schema::new(snapshot.for_service(ANCHORING_INSTANCE_NAME).unwrap())
}

impl AnchoringTestKit {
    /// Creates an anchoring testkit instance for the specified number of anchoring nodes,
    /// and interval between anchors.
    pub fn new(nodes_num: u16, anchoring_interval: u64) -> Self {
        let validator_keys = (0..nodes_num)
            .map(|_| gen_validator_keys())
            .collect::<Vec<_>>();

        let network = Network::Testnet;
        let anchoring_nodes = AnchoringNodes::from_keys(Network::Testnet, &validator_keys);

        let anchoring_config = Config {
            network,
            anchoring_keys: anchoring_nodes.anchoring_keys(),
            anchoring_interval,
            ..Config::default()
        };

        let anchoring_artifact = BtcAnchoringService.artifact_id();
        let inner = TestKitBuilder::validator()
            .with_keys(validator_keys)
            // Supervisor
            .with_rust_service(Supervisor)
            .with_artifact(Supervisor.artifact_id())
            .with_instance(Supervisor::simple())
            // Anchoring
            .with_rust_service(BtcAnchoringService)
            .with_artifact(anchoring_artifact.clone())
            .with_instance(InstanceInitParams::new(
                ANCHORING_INSTANCE_ID,
                ANCHORING_INSTANCE_NAME,
                anchoring_artifact,
                anchoring_config,
            ))
            .build();

        Self {
            inner,
            anchoring_nodes,
        }
    }

    /// Returns the actual anchoring configuration.
    pub fn actual_anchoring_config(&self) -> Config {
        get_anchoring_schema(&self.inner.snapshot()).actual_config()
    }

    /// Returns the latest anchoring transaction.
    pub fn last_anchoring_tx(&self) -> Option<btc::Transaction> {
        get_anchoring_schema(&self.inner.snapshot())
            .transactions_chain
            .last()
    }

    /// Returns the proposal of the next anchoring transaction for the actual anchoring state.
    pub fn anchoring_transaction_proposal(
        &self,
    ) -> Option<(btc::Transaction, Vec<btc::Transaction>)> {
        get_anchoring_schema(&self.inner.snapshot())
            .actual_proposed_anchoring_transaction(self.inner.snapshot().for_core())
            .map(Result::unwrap)
    }

    /// Creates signatures for each input of the proposed anchoring transaction signed by the
    /// specified node.
    pub fn create_signature_tx_for_node(
        &self,
        node: &TestNode,
    ) -> Result<Vec<Verified<AnyTx>>, btc::BuilderError> {
        let service_keypair = node.service_keypair();
        let snapshot = self.inner.snapshot();
        let schema = get_anchoring_schema(&snapshot);

        let mut signatures = Vec::new();
        if let Some(p) = schema.actual_proposed_anchoring_transaction(snapshot.for_core()) {
            let (proposal, proposal_inputs) = p?;

            let actual_config = schema.actual_state().actual_config().clone();
            let bitcoin_key = actual_config
                .find_bitcoin_key(&service_keypair.public_key())
                .unwrap()
                .1;
            let btc_private_key = self.anchoring_nodes.private_key(&bitcoin_key);

            let redeem_script = actual_config.redeem_script();
            let mut signer = p2wsh::InputSigner::new(redeem_script);
            for (index, proposal_input) in proposal_inputs.iter().enumerate() {
                let signature = signer
                    .sign_input(
                        TxInRef::new(proposal.as_ref(), index),
                        proposal_input.as_ref(),
                        &btc_private_key.0.key,
                    )
                    .unwrap();

                signatures.push(service_keypair.sign_input(
                    ANCHORING_INSTANCE_ID,
                    SignInput {
                        input: index as u32,
                        input_signature: signature.into(),
                        txid: proposal.id(),
                    },
                ));
            }
        }
        Ok(signatures)
    }

    /// Creates signatures for each input of the proposed anchoring transaction signed by all of
    /// anchoring nodes.
    pub fn create_signature_txs(&self) -> Vec<Vec<Verified<AnyTx>>> {
        let mut signatures = Vec::new();

        for anchoring_keys in self.actual_anchoring_config().anchoring_keys {
            let node = self
                .find_node_by_service_key(anchoring_keys.service_key)
                .unwrap();

            signatures.push(self.create_signature_tx_for_node(node).unwrap());
        }
        signatures
    }

    /// Creates the confirmation transactions with a funding transaction to the current address
    /// with a given amount of Satoshi.
    pub fn create_funding_confirmation_txs(
        &self,
        satoshis: u64,
    ) -> (Vec<Verified<AnyTx>>, btc::Transaction) {
        let funding_transaction = create_fake_funding_transaction(
            &self.actual_anchoring_config().anchoring_address(),
            satoshis,
        );
        (
            self.create_funding_confirmation_txs_with(funding_transaction.clone()),
            funding_transaction,
        )
    }

    /// Creates the confirmation transactions with a specified funding transaction.
    pub fn create_funding_confirmation_txs_with(
        &self,
        transaction: btc::Transaction,
    ) -> Vec<Verified<AnyTx>> {
        let add_funds = AddFunds { transaction };
        self.actual_anchoring_config()
            .anchoring_keys
            .into_iter()
            .map(move |anchoring_keys| {
                let node_keypair = self
                    .find_node_by_service_key(anchoring_keys.service_key)
                    .unwrap()
                    .service_keypair();

                node_keypair.add_funds(ANCHORING_INSTANCE_ID, add_funds.clone())
            })
            .collect()
    }

    /// Creates configuration change transaction for simple supervisor.
    pub fn create_config_change_tx(&self, proposal: ConfigPropose) -> Verified<AnyTx> {
        let initiator_id = self.inner.network().us().validator_id().unwrap();
        let keypair = self.inner.validator(initiator_id).service_keypair();
        keypair.propose_config_change(SUPERVISOR_INSTANCE_ID, proposal)
    }

    /// Adds a new auditor node to the testkit network and create Bitcoin keypair for it.
    pub fn add_node(&mut self) -> AnchoringKeys {
        let service_key = self
            .inner
            .network_mut()
            .add_node()
            .service_keypair()
            .public_key();
        let bitcoin_key = self
            .anchoring_nodes
            .add_node(self.actual_anchoring_config().network, service_key);

        AnchoringKeys {
            bitcoin_key,
            service_key,
        }
    }

    /// Returns a corresponding private Bitcoin key.
    pub fn node_private_key(&self, public_key: &btc::PublicKey) -> btc::PrivateKey {
        self.anchoring_nodes.private_key(public_key)
    }

    /// Generates bitcoin keypair and adds them to the key pool.
    pub fn gen_bitcoin_key(&mut self) -> btc::PublicKey {
        let keypair = btc::gen_keypair(self.actual_anchoring_config().network);
        self.anchoring_nodes.key_pool.insert(keypair.0, keypair.1);
        keypair.0
    }

    /// Returns the block hash for the given blockchain height.
    pub fn block_hash_on_height(&self, height: Height) -> Hash {
        self.inner
            .snapshot()
            .for_core()
            .block_hashes_by_height()
            .get(height.0)
            .unwrap()
    }

    /// Returns Bitcoin key pairs of anchoring nodes.
    pub fn anchoring_keypairs(
        &self,
    ) -> impl IntoIterator<Item = (btc::PublicKey, btc::PrivateKey)> {
        self.anchoring_nodes.anchoring_keypairs()
    }

    /// Finds anchoring node with the specified bitcoin key.
    pub fn find_anchoring_node(&self, bitcoin_key: &btc::PublicKey) -> Option<&TestNode> {
        self.anchoring_nodes
            .inner
            .iter()
            .find_map(|keypair| {
                if keypair.1 == bitcoin_key {
                    Some(*keypair.0)
                } else {
                    None
                }
            })
            .and_then(|service_key| self.find_node_by_service_key(service_key))
    }

    fn find_node_by_service_key(&self, service_key: PublicKey) -> Option<&TestNode> {
        self.inner
            .network()
            .nodes()
            .iter()
            .find(|node| node.service_keypair().public_key() == service_key)
    }
}

impl Default for AnchoringTestKit {
    /// Creates anchoring testkit instance with the unspent funding transaction.
    ///
    /// To add funds, this instance commit a block with transactions, so in addition to the
    /// genesis block this instance contains one more.
    fn default() -> Self {
        let mut testkit = Self::new(4, 5);
        testkit
            .inner
            .create_block_with_transactions(testkit.create_funding_confirmation_txs(700_000).0);
        testkit
    }
}

impl PublicApi for TestKitApi {
    type Error = api::Error;

    fn actual_address(&self) -> Result<btc::Address, Self::Error> {
        self.public(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .get("address/actual")
    }

    fn following_address(&self) -> Result<Option<btc::Address>, Self::Error> {
        self.public(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .get("address/following")
    }

    fn find_transaction(&self, height: Option<Height>) -> Result<TransactionProof, Self::Error> {
        self.public(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .query(&FindTransactionQuery { height })
            .get("find-transaction")
    }

    fn config(&self) -> Result<Config, Self::Error> {
        self.public(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .get("config")
    }
}

impl PrivateApi for TestKitApi {
    type Error = api::Error;

    fn sign_input(&self, sign_input: SignInput) -> Result<Hash, Self::Error> {
        self.private(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .query(&sign_input)
            .post("sign-input")
    }

    fn add_funds(&self, transaction: btc::Transaction) -> Result<Hash, Self::Error> {
        self.private(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .query(&transaction)
            .post("add-funds")
    }

    fn anchoring_proposal(&self) -> Result<AnchoringProposalState, Self::Error> {
        self.private(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .get("anchoring-proposal")
    }

    fn config(&self) -> Result<Config, Self::Error> {
        self.private(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .get("config")
    }

    fn transaction_with_index(&self, index: u64) -> Result<Option<btc::Transaction>, Self::Error> {
        self.private(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .query(&IndexQuery { index })
            .get("transaction")
    }

    fn transactions_count(&self) -> Result<AnchoringChainLength, Self::Error> {
        self.private(ApiKind::Service(ANCHORING_INSTANCE_NAME))
            .get("transactions-count")
    }
}

fn validate_table_proof(
    actual_config: &ConsensusConfig,
    block_proof: &BlockProof,
    state_proof: MapProof<String, Hash>,
) -> Result<(String, Hash), failure::Error> {
    // Checks precommits.
    for precommit in &block_proof.precommits {
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
            precommit.as_ref().block_hash() == &block_proof.block.object_hash(),
            "Block hash doesn't match"
        );
    }

    // Checks state_hash.
    let checked_table_proof = state_proof.check()?;
    ensure!(
        checked_table_proof.index_hash() == block_proof.block.state_hash,
        "State hash doesn't match"
    );
    let (table_name, table_hash) = checked_table_proof
        .entries()
        .next()
        .ok_or_else(|| format_err!("Unable to get `to_block_header` entry"))?;
    Ok((table_name.to_owned(), *table_hash))
}

/// Proof validation extension.
pub trait ValidateProof {
    /// Output value.
    type Output;
    /// Perform the proof validation procedure with the given exonum blockchain configuration.
    fn validate(self, actual_config: &ConsensusConfig) -> Result<Self::Output, failure::Error>;
}

impl ValidateProof for TransactionProof {
    type Output = Option<(u64, btc::Transaction)>;

    fn validate(self, actual_config: &ConsensusConfig) -> Result<Self::Output, failure::Error> {
        let proof_entry = validate_table_proof(actual_config, &self.block_proof, self.state_proof)?;

        ensure!(
            proof_entry.0 == format!("{}.transactions_chain", ANCHORING_INSTANCE_NAME),
            "Invalid table location"
        );

        // Validate value.
        let values = self
            .transaction_proof
            .check_against_hash(proof_entry.1)
            .map_err(|e| format_err!("An error occurred {:?}", e))?
            .entries();
        ensure!(values.len() <= 1, "Invalid values count");

        Ok(values.first().cloned())
    }
}
