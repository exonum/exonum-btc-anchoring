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

//! Anchoring HTTP API implementation.

use btc_transaction_utils::{p2wsh, TxInRef};
use exonum::{
    blockchain::{BlockProof, IndexCoordinates, IndexOwner, Schema as CoreSchema},
    crypto::Hash,
    helpers::Height,
    merkledb::{ListProof, MapProof, ObjectHash, Snapshot},
    runtime::{
        api::{self, ServiceApiBuilder, ServiceApiState},
        rust::Transaction,
    },
};
use futures::{Future, IntoFuture, Sink};
use serde_derive::{Deserialize, Serialize};

use std::cmp::{
    self,
    Ordering::{self, Equal, Greater, Less},
};

use crate::{
    blockchain::{BtcAnchoringSchema, SignInput},
    btc,
    config::Config,
};

/// Type alias for the asynchronous result that will be ready in the future.
pub type AsyncResult<I, E> = Box<dyn Future<Item = I, Error = E> + Send>;

pub(crate) trait IntoAsyncResult<I, E> {
    fn into_async(self) -> AsyncResult<I, E>;
}

impl<I, E> IntoAsyncResult<I, E> for Result<I, E>
where
    I: Send + 'static,
    E: Send + 'static,
{
    fn into_async(self) -> AsyncResult<I, E> {
        Box::new(self.into_future())
    }
}

/// A proof of existence for an anchoring transaction at the given height.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionProof {
    /// Latest authorized block in the blockchain.
    pub latest_authorized_block: BlockProof,
    /// Proof for the whole database table.
    pub to_table: MapProof<IndexCoordinates, Hash>,
    /// Proof for the specific transaction in this table.
    pub to_transaction: ListProof<btc::Transaction>,
    /// Anchoring transactions total count.
    pub transactions_count: u64,
}

/// A proof of existence for an anchored or a non-anchored Exonum block at the given height.
#[derive(Debug, Serialize, Deserialize)]
pub struct BlockHeaderProof {
    /// Latest authorized block in the blockchain.
    pub latest_authorized_block: BlockProof,
    /// Proof for the whole database table.
    pub to_table: MapProof<IndexCoordinates, Hash>,
    /// Proof for the specific header in this table.
    pub to_block_header: ListProof<Hash>,
}

/// Next anchoring transaction proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchoringTransactionProposal {
    /// Proposal content.
    pub transaction: btc::Transaction,
    /// Input transactions.
    pub inputs: Vec<btc::Transaction>,
}

/// Public API specification for the Exonum Bitcoin anchoring service.
pub trait PublicApi {
    /// Error type for the current public API implementation.
    type Error;
    /// Returns actual anchoring address.
    ///
    /// `GET /{api_prefix}/address/actual`
    fn actual_address(&self) -> Result<btc::Address, Self::Error>;
    /// Returns the following anchoring address if the node is in the transition state.
    ///
    /// `GET /{api_prefix}/address/following`
    fn following_address(&self) -> Result<Option<btc::Address>, Self::Error>;
    /// Returns the latest anchoring transaction if the height is not specified,
    /// otherwise, returns the anchoring transaction with the height that is greater or equal
    /// to the given one.
    ///
    /// `GET /{api_prefix}/find-transaction`
    fn find_transaction(
        &self,
        height: Option<Height>,
    ) -> Result<Option<TransactionProof>, Self::Error>;
    /// A method that provides cryptographic proofs for Exonum blocks including those anchored to
    /// Bitcoin blockchain. The proof is an apparent evidence of availability of a certain Exonum
    /// block in the blockchain.
    ///
    /// `GET /{api_prefix}/block-header-proof?height={height}`
    fn block_header_proof(&self, height: Height) -> Result<BlockHeaderProof, Self::Error>;
    /// Return an actual anchoring configuration.
    ///
    /// `GET /{api_prefix}/config`
    fn config(&self) -> AsyncResult<Config, Self::Error>;
}

/// Private API specification for the Exonum Bitcoin anchoring service.
pub trait PrivateApi {
    /// Error type for the current public API implementation.
    type Error;
    /// Create and broadcast the `TxSignature` transaction, which is signed
    /// by the current node, and returns its hash.
    ///
    /// `POST /{api_prefix}/sign-input`
    fn sign_input(&self, sign_input: SignInput) -> AsyncResult<Hash, Self::Error>;
    /// Return a proposal for the next anchoring transaction, if it makes sense.
    /// If there is not enough satoshis to create a proposal an error is returned.
    ///
    /// `GET /{api_prefix}/anchoring-proposal`
    fn anchoring_proposal(&self) -> AsyncResult<Option<AnchoringTransactionProposal>, Self::Error>;
    /// Return an actual anchoring configuration.
    ///
    /// `GET /{api_prefix}/config`
    fn config(&self) -> AsyncResult<Config, Self::Error>;
    /// Return an anchoring transaction with the specified index in anchoring transactions chain.
    ///
    /// `GET /{api_prefix}/transaction?index={index}`
    fn transaction_with_index(&self, index: u64) -> Result<Option<btc::Transaction>, Self::Error>;
    /// Return a total number of anchoring transactions in the chain.
    ///
    /// `GET /{api_prefix}/transactions-count`
    fn transactions_count(&self) -> Result<u64, Self::Error>;
}

struct ApiImpl<'a>(&'a ServiceApiState<'a>);

impl<'a> ApiImpl<'a> {
    fn anchoring_schema(&self) -> BtcAnchoringSchema<&'a dyn Snapshot> {
        BtcAnchoringSchema::new(self.0.instance.name, self.0.snapshot())
    }

    fn broadcast_transaction(
        &self,
        transaction: impl Transaction,
    ) -> impl Future<Item = Hash, Error = failure::Error> {
        use exonum::node::ExternalMessage;

        let keypair = self.0.service_keypair;
        let signed = transaction.sign(self.0.instance.id, keypair.0, &keypair.1);

        let hash = signed.object_hash();
        // TODO Move this code to core
        self.0
            .sender()
            .clone()
            .0
            .send(ExternalMessage::Transaction(signed))
            .map(move |_| hash)
            .map_err(From::from)
    }

    fn actual_config(&self) -> Result<Config, failure::Error> {
        Ok(BtcAnchoringSchema::new(self.0.instance.name, self.0.snapshot()).actual_configuration())
    }

    fn verify_sign_input(&self, sign_input: &SignInput) -> Result<(), failure::Error> {
        let schema = self.anchoring_schema();
        let (proposal, inputs) = schema
            .actual_proposed_anchoring_transaction()
            .ok_or_else(|| failure::format_err!("Anchoring transaction proposal is absent."))??;

        // Verify transaction content.
        let input = inputs.get(sign_input.input as usize).ok_or_else(|| {
            failure::format_err!("Missing input with index: {}", sign_input.input)
        })?;
        failure::ensure!(
            proposal == sign_input.transaction,
            "Invalid anchoring proposal"
        );

        // Find corresponding Bitcoin key.
        let config = schema.actual_configuration();
        let bitcoin_key = config
            .find_bitcoin_key(&self.0.service_keypair.0)
            .ok_or_else(|| failure::format_err!("This node is not an anchoring node."))?
            .1;

        // Verify input signature.
        p2wsh::InputSigner::new(config.redeem_script())
            .verify_input(
                TxInRef::new(sign_input.transaction.as_ref(), sign_input.input as usize),
                input.as_ref(),
                &bitcoin_key.0,
                sign_input.input_signature.as_ref(),
            )
            .map_err(|e| failure::format_err!("Input signature verification failed: {}", e))
    }

    fn transaction_proof(&self, tx_index: u64) -> TransactionProof {
        let blockchain_schema = CoreSchema::new(self.0.snapshot());
        let max_height = blockchain_schema.block_hashes_by_height().len() - 1;
        let latest_authorized_block = blockchain_schema
            .block_and_precommits(Height(max_height))
            .unwrap();

        let tx_chain = self.anchoring_schema().anchoring_transactions_chain();

        let to_table = blockchain_schema
            .state_hash_aggregator()
            .get_proof(IndexOwner::Service(self.0.instance.id).coordinate_for(0));

        let to_transaction = tx_chain.get_proof(tx_index);

        TransactionProof {
            latest_authorized_block,
            to_table,
            to_transaction,
            transactions_count: tx_chain.len(),
        }
    }
}

impl<'a> PublicApi for ApiImpl<'a> {
    type Error = api::Error;

    fn actual_address(&self) -> Result<btc::Address, Self::Error> {
        Ok(self
            .anchoring_schema()
            .actual_configuration()
            .anchoring_address())
    }

    fn following_address(&self) -> Result<Option<btc::Address>, Self::Error> {
        Ok(self
            .anchoring_schema()
            .following_configuration()
            .map(|config| config.anchoring_address()))
    }

    fn find_transaction(
        &self,
        height: Option<Height>,
    ) -> Result<Option<TransactionProof>, Self::Error> {
        let snapshot = self.0.snapshot();
        let anchoring_schema = BtcAnchoringSchema::new(self.0.instance.name, snapshot);
        let tx_chain = anchoring_schema.anchoring_transactions_chain();

        if tx_chain.is_empty() {
            return Ok(None);
        }

        let tx_index = if let Some(height) = height {
            // Handmade binary search.
            let f = |index| -> Ordering {
                // index is always in [0, size), that means index is >= 0 and < size.
                // index >= 0: by definition
                // index < size: index = size / 2 + size / 4 + size / 8 ...
                let other = tx_chain
                    .get(index)
                    .unwrap()
                    .anchoring_payload()
                    .unwrap()
                    .block_height;
                other.cmp(&height)
            };

            let mut base = 0;
            let mut size = tx_chain.len();
            while size > 1 {
                let half = size / 2;
                let mid = base + half;
                let cmp = f(mid);
                base = if cmp == Greater { base } else { mid };
                size -= half;
            }
            // Don't forget to check base value.
            let cmp = f(base);
            if cmp == Equal {
                base
            } else {
                cmp::min(base + (cmp == Less) as u64, tx_chain.len() - 1)
            }
        } else {
            tx_chain.len() - 1
        };

        Ok(Some(self.transaction_proof(tx_index)))
    }

    fn block_header_proof(&self, height: Height) -> Result<BlockHeaderProof, Self::Error> {
        let snapshot = self.0.snapshot();
        let blockchain_schema = CoreSchema::new(snapshot);
        let anchoring_schema = BtcAnchoringSchema::new(self.0.instance.name, snapshot);

        let max_height = blockchain_schema.block_hashes_by_height().len() - 1;

        let latest_authorized_block = blockchain_schema
            .block_and_precommits(Height(max_height))
            .unwrap();
        let to_table = blockchain_schema
            .state_hash_aggregator()
            .get_proof(IndexOwner::Service(self.0.instance.id).coordinate_for(3));
        let to_block_header = anchoring_schema.anchored_blocks().get_proof(height.0);

        Ok(BlockHeaderProof {
            latest_authorized_block,
            to_table,
            to_block_header,
        })
    }

    fn config(&self) -> AsyncResult<Config, Self::Error> {
        self.actual_config().map_err(From::from).into_async()
    }
}

impl<'a> PrivateApi for ApiImpl<'a> {
    type Error = api::Error;

    fn sign_input(&self, sign_input: SignInput) -> AsyncResult<Hash, Self::Error> {
        // Verify Bitcoin signature.
        if let Err(e) = self.verify_sign_input(&sign_input) {
            return Err(api::Error::BadRequest(e.to_string())).into_async();
        }
        Box::new(self.broadcast_transaction(sign_input).map_err(From::from))
    }

    fn anchoring_proposal(&self) -> AsyncResult<Option<AnchoringTransactionProposal>, Self::Error> {
        self.anchoring_schema()
            .actual_proposed_anchoring_transaction()
            .transpose()
            .map(|x| x.map(AnchoringTransactionProposal::from))
            .map_err(Self::Error::from)
            .into_async()
    }

    fn config(&self) -> AsyncResult<Config, Self::Error> {
        self.actual_config().map_err(From::from).into_async()
    }

    fn transaction_with_index(&self, index: u64) -> Result<Option<btc::Transaction>, Self::Error> {
        Ok(
            BtcAnchoringSchema::new(self.0.instance.name, self.0.snapshot())
                .anchoring_transactions_chain()
                .get(index),
        )
    }

    fn transactions_count(&self) -> Result<u64, Self::Error> {
        Ok(
            BtcAnchoringSchema::new(self.0.instance.name, self.0.snapshot())
                .anchoring_transactions_chain()
                .len(),
        )
    }
}

/// Query parameters for the find transaction request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FindTransactionQuery {
    /// Exonum block height.
    pub height: Option<Height>,
}

/// Query parameters for the block header proof request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HeightQuery {
    /// Exonum block height.
    pub height: Height,
}

/// Query parameters for the anchoring transaction request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IndexQuery {
    /// Index of the anchoring transaction.
    pub index: u64,
}

pub(crate) fn wire(builder: &mut ServiceApiBuilder) {
    builder
        .public_scope()
        .endpoint("address/actual", |state, _query: ()| {
            ApiImpl(state).actual_address()
        })
        .endpoint("address/following", |state, _query: ()| {
            ApiImpl(state).following_address()
        })
        .endpoint("find-transaction", |state, query: FindTransactionQuery| {
            ApiImpl(state).find_transaction(query.height)
        })
        .endpoint("block-header-proof", |state, query: HeightQuery| {
            ApiImpl(state).block_header_proof(query.height)
        })
        .endpoint("config", |state, _query: ()| {
            PublicApi::config(&ApiImpl(state))
        });
    builder
        .private_scope()
        .endpoint_mut("sign-input", |state, query: SignInput| {
            ApiImpl(state).sign_input(query)
        })
        .endpoint("anchoring-proposal", |state, _query: ()| {
            ApiImpl(state).anchoring_proposal()
        })
        .endpoint("config", |state, _query: ()| {
            PrivateApi::config(&ApiImpl(state))
        })
        .endpoint("transaction", |state, query: IndexQuery| {
            ApiImpl(state).transaction_with_index(query.index)
        })
        .endpoint("transactions-count", |state, _query: ()| {
            ApiImpl(state).transactions_count()
        });
}

impl<T> std::fmt::Debug for dyn PublicApi<Error = T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicApi").finish()
    }
}

impl<T> std::fmt::Debug for dyn PrivateApi<Error = T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivateApi").finish()
    }
}

impl From<btc::BuilderError> for api::Error {
    fn from(inner: btc::BuilderError) -> Self {
        // TODO Find more idiomatic error code.
        api::Error::BadRequest(inner.to_string())
    }
}

impl From<(btc::Transaction, Vec<btc::Transaction>)> for AnchoringTransactionProposal {
    fn from((transaction, inputs): (btc::Transaction, Vec<btc::Transaction>)) -> Self {
        Self {
            transaction,
            inputs,
        }
    }
}
