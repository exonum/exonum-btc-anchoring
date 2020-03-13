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

//! Bitcoin Anchoring HTTP API.
//!
//! Anchoring API is divided into public and private parts, with public part intended for
//! unauthorized use, and private part intended to be used by [sync][sync] module.
//! Private part is implementation detail and should not be used directly.
//!
//! [sync]: ../sync/index.html

use anyhow::{anyhow, ensure};
use async_trait::async_trait;
use btc_transaction_utils::{p2wsh, TxInRef};
use exonum::{blockchain::IndexProof, crypto::Hash, helpers::Height};
use exonum_merkledb::ListProof;
use exonum_rust_runtime::{
    api::{self, ServiceApiBuilder, ServiceApiState},
    Broadcaster,
};
use serde_derive::{Deserialize, Serialize};

use std::cmp::{
    self,
    Ordering::{self, Equal, Greater, Less},
};

use crate::{
    blockchain::{AddFunds, BtcAnchoringInterface, Schema, SignInput},
    btc,
    config::Config,
};

/// A proof of existence for an anchoring transaction at the given height.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionProof {
    /// Proof of authenticity for a transactions index within the database.
    pub index_proof: IndexProof,
    /// Proof for the specific transaction in this table.
    pub transaction_proof: ListProof<btc::Transaction>,
}

/// State of the next anchoring transaction proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnchoringProposalState {
    /// There is no anchoring transaction proposal at the time.
    None,
    /// There is a non-finalized anchoring transaction.
    Available {
        /// Proposal content.
        transaction: btc::Transaction,
        // TODO Replace by more lightweight value amounts per input in according of
        // `UnspentTxOutValue::Balance` variant. [ECR-3222]
        /// Input transactions.
        inputs: Vec<btc::Transaction>,
    },
    /// Insufficient funds to create an anchoring transaction proposal. Please fill up an anchoring wallet.
    InsufficientFunds {
        /// Total transaction fee.
        total_fee: u64,
        /// Available balance.
        balance: u64,
    },
    /// Initial funding transaction is absent.
    NoInitialFunds,
}

impl AnchoringProposalState {
    fn try_from_proposal(
        proposal: Option<Result<(btc::Transaction, Vec<btc::Transaction>), btc::BuilderError>>,
    ) -> Result<Self, api::Error> {
        match proposal {
            None => Ok(AnchoringProposalState::None),
            Some(Ok((transaction, inputs))) => Ok(AnchoringProposalState::Available {
                transaction,
                inputs,
            }),
            Some(Err(btc::BuilderError::InsufficientFunds { total_fee, balance })) => {
                Ok(AnchoringProposalState::InsufficientFunds { total_fee, balance })
            }
            Some(Err(btc::BuilderError::NoInputs)) => Ok(AnchoringProposalState::NoInitialFunds),
            Some(Err(e)) => Err(api::Error::internal(e)),
        }
    }
}

/// Total length of anchoring transaction chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnchoringChainLength {
    /// Length value.
    pub value: u64,
}

impl From<u64> for AnchoringChainLength {
    fn from(value: u64) -> Self {
        Self { value }
    }
}

/// Public API client for the Exonum Bitcoin anchoring service.
#[async_trait]
pub trait PublicApi {
    /// Error type for the current API client implementation.
    type Error;
    /// Returns an actual anchoring address.
    ///
    /// | Property    | Value |
    /// |-------------|-------|
    /// | Path        | `/api/services/{btc_anchoring}/address/actual` |
    /// | Method      | GET   |
    /// | Query type  | - |
    /// | Return type | [`btc::Address`] |
    ///
    /// [`btc::Address`]: ../btc/struct.Address.html
    async fn actual_address(&self) -> Result<btc::Address, Self::Error>;
    /// Returns the following anchoring address if the node is in the transition state.
    ///
    /// | Property    | Value |
    /// |-------------|-------|
    /// | Path        | `/api/services/{btc_anchoring}/address/following` |
    /// | Method      | GET   |
    /// | Query type  | - |
    /// | Return type | [`Option<btc::Address>`] |
    ///
    /// [`Option<btc::Address>`]: ../btc/struct.Address.html
    async fn following_address(&self) -> Result<Option<btc::Address>, Self::Error>;
    /// Returns the latest anchoring transaction if the height is not specified,
    /// otherwise, return the anchoring transaction with the height that is greater or equal
    /// to the given one.
    ///
    /// | Property    | Value |
    /// |-------------|-------|
    /// | Path        | `/api/services/{btc_anchoring}/find-transaction` |
    /// | Method      | GET   |
    /// | Query type  | [`FindTransactionQuery`] |
    /// | Return type | [`TransactionProof`] |
    ///
    /// [`FindTransactionQuery`]: struct.FindTransactionQuery.html
    /// [`TransactionProof`]: struct.TransactionProof.html
    async fn find_transaction(
        &self,
        height: Option<Height>,
    ) -> Result<TransactionProof, Self::Error>;
    /// Returns an actual anchoring configuration.
    ///
    /// | Property    | Value |
    /// |-------------|-------|
    /// | Path        | `/api/services/{btc_anchoring}/config` |
    /// | Method      | GET   |
    /// | Query type  | - |
    /// | Return type | [`Config`] |
    ///
    /// [`config`]: ../config/struct.Config.html
    async fn config(&self) -> Result<Config, Self::Error>;
}

/// Private API client for the Exonum Bitcoin anchoring service.
#[async_trait]
pub trait PrivateApi {
    /// Error type for the current API client implementation.
    type Error;
    /// Creates and broadcasts the `TxSignature` transaction, which is signed
    /// by the current node, and returns its hash.
    ///
    /// | Property    | Value |
    /// |-------------|-------|
    /// | Path        | `/api/services/{btc_anchoring}/sign-input` |
    /// | Method      | POST   |
    /// | Query type  | [`SignInput`] |
    /// | Return type | [`Hash`] |
    ///
    /// [`SignInput`]: ../blockchain/struct.SignInput.html
    /// [`Hash`]: https://docs.rs/exonum-crypto/latest/exonum_crypto/struct.Hash.html
    async fn sign_input(&self, sign_input: SignInput) -> Result<Hash, Self::Error>;
    /// Adds funds via suitable funding transaction.
    ///
    /// Bitcoin transaction should have output with value to the current anchoring address.
    /// The transaction will be applied if 2/3+1 anchoring nodes sent it.
    ///
    /// | Property    | Value |
    /// |-------------|-------|
    /// | Path        | `/api/services/{btc_anchoring}/add-funds` |
    /// | Method      | POST   |
    /// | Query type  | [`AddFunds`] |
    /// | Return type | [`Hash`] |
    ///
    /// [`AddFunds`]: ../blockchain/struct.AddFunds.html
    /// [`Hash`]: https://docs.rs/exonum-crypto/latest/exonum_crypto/struct.Hash.html
    async fn add_funds(&self, transaction: btc::Transaction) -> Result<Hash, Self::Error>;
    /// Returns a proposal for the next anchoring transaction, if it makes sense.
    /// If there is not enough satoshis to create a proposal an error is returned.
    ///
    /// | Property    | Value |
    /// |-------------|-------|
    /// | Path        | `/api/services/{btc_anchoring}/anchoring-proposal` |
    /// | Method      | GET   |
    /// | Query type  | - |
    /// | Return type | [`AnchoringProposalState`] |
    ///
    /// [`AnchoringProposalState`]: enum.AnchoringProposalState.html
    async fn anchoring_proposal(&self) -> Result<AnchoringProposalState, Self::Error>;
    /// Returns an actual anchoring configuration.
    ///
    /// | Property    | Value |
    /// |-------------|-------|
    /// | Path        | `/api/services/{btc_anchoring}/config` |
    /// | Method      | GET   |
    /// | Query type  | - |
    /// | Return type | [`Config`] |
    ///
    /// [`config`]: ../config/struct.Config.html
    async fn config(&self) -> Result<Config, Self::Error>;
    /// Returns an anchoring transaction with the specified index in anchoring transactions chain.
    ///
    /// | Property    | Value |
    /// |-------------|-------|
    /// | Path        | `/api/services/{btc_anchoring}/transaction` |
    /// | Method      | GET   |
    /// | Query type  | [`IndexQuery`] |
    /// | Return type | [`Option<btc::Transaction>`] |
    ///
    /// ['IndexQuery']: struct.IndexQuery.html
    /// [`Option<btc::Transaction>`]: ../btc/struct.Transaction.html
    async fn transaction_with_index(
        &self,
        index: u64,
    ) -> Result<Option<btc::Transaction>, Self::Error>;
    /// Returns a total number of anchoring transactions in the chain.
    ///
    /// | Property    | Value |
    /// |-------------|-------|
    /// | Path        | `/api/services/{btc_anchoring}/transactions-count` |
    /// | Method      | GET   |
    /// | Query type  | - |
    /// | Return type | [`AnchoringChainLength`] |
    ///
    /// [`AnchoringChainLength`]: struct.AnchoringChainLength.html
    async fn transactions_count(&self) -> Result<AnchoringChainLength, Self::Error>;
}

struct ApiImpl(ServiceApiState);

impl ApiImpl {
    fn broadcaster(&self) -> api::Result<Broadcaster> {
        self.0.broadcaster().ok_or_else(|| {
            api::Error::bad_request()
                .title("Invalid broadcast request")
                .detail("Node is not a validator")
        })
    }

    fn actual_config(self) -> api::Result<Config> {
        Ok(Schema::new(self.0.service_data()).actual_config())
    }

    fn verify_sign_input(&self, sign_input: &SignInput) -> Result<(), anyhow::Error> {
        let schema = Schema::new(self.0.service_data());
        let (proposal, inputs) = schema
            .actual_proposed_anchoring_transaction(self.0.data().for_core())
            .ok_or_else(|| anyhow!("Anchoring transaction proposal is absent."))??;

        // Verify transaction content.
        let input = inputs
            .get(sign_input.input as usize)
            .ok_or_else(|| anyhow!("Missing input with index: {}", sign_input.input))?;

        // Find corresponding Bitcoin key.
        let config = schema.actual_config();
        let bitcoin_key = config
            .find_bitcoin_key(&self.0.service_key())
            .ok_or_else(|| anyhow!("This node is not an anchoring node."))?
            .1;

        // Verify input signature.
        p2wsh::InputSigner::new(config.redeem_script())
            .verify_input(
                TxInRef::new(proposal.as_ref(), sign_input.input as usize),
                input.as_ref(),
                &bitcoin_key.0,
                sign_input.input_signature.as_ref(),
            )
            .map_err(|e| anyhow!("Input signature verification failed: {}", e))
    }

    fn verify_funding_tx(&self, tx: &btc::Transaction) -> Result<(), anyhow::Error> {
        let txid = tx.id();

        let schema = Schema::new(self.0.service_data());
        let config = schema.actual_config();
        ensure!(
            !schema.spent_funding_transactions.contains(&txid),
            "Funding transaction {} has been already used.",
            txid
        );
        ensure!(
            tx.find_out(&config.anchoring_out_script()).is_some(),
            "Funding transaction {} is not suitable.",
            txid
        );
        Ok(())
    }

    fn transaction_proof(&self, tx_index: u64) -> TransactionProof {
        let index_proof = self
            .0
            .data()
            .proof_for_service_index("transactions_chain")
            .unwrap();
        let transaction_proof = Schema::new(self.0.service_data())
            .transactions_chain
            .get_proof(tx_index);

        TransactionProof {
            index_proof,
            transaction_proof,
        }
    }
}

// Public API implementation
impl ApiImpl {
    async fn actual_address(self) -> api::Result<btc::Address> {
        Ok(Schema::new(self.0.service_data())
            .actual_config()
            .anchoring_address())
    }

    async fn following_address(self) -> api::Result<Option<btc::Address>> {
        Ok(Schema::new(self.0.service_data())
            .following_config()
            .map(|config| config.anchoring_address()))
    }

    async fn find_transaction(self, height: Option<Height>) -> api::Result<TransactionProof> {
        let anchoring_schema = Schema::new(self.0.service_data());
        let tx_chain = anchoring_schema.transactions_chain;

        if tx_chain.is_empty() {
            return Ok(self.transaction_proof(0));
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

        Ok(self.transaction_proof(tx_index))
    }

    async fn config(self) -> api::Result<Config> {
        self.actual_config().map_err(api::Error::internal)
    }
}

/// Private API implementation
impl ApiImpl {
    async fn sign_input(self, sign_input: SignInput) -> Result<Hash, api::Error> {
        // Verify Bitcoin signature.
        self.verify_sign_input(&sign_input).map_err(|e| {
            api::Error::bad_request()
                .title("Sign input request verification has failed")
                .detail(e.to_string())
        })?;

        self.broadcaster()?
            .sign_input((), sign_input)
            .await
            .map_err(|e| api::Error::internal(e).title("Sign input request failed"))
    }

    async fn add_funds(self, transaction: btc::Transaction) -> Result<Hash, api::Error> {
        self.verify_funding_tx(&transaction).map_err(|e| {
            api::Error::bad_request()
                .title("Funding tx verification has failed")
                .detail(e.to_string())
        })?;

        self.broadcaster()?
            .add_funds((), AddFunds { transaction })
            .await
            .map_err(|e| api::Error::internal(e).title("Add funds request failed"))
    }

    async fn anchoring_proposal(self) -> Result<AnchoringProposalState, api::Error> {
        let core_schema = self.0.data().for_core();
        let anchoring_schema = Schema::new(self.0.service_data());

        AnchoringProposalState::try_from_proposal(
            anchoring_schema.actual_proposed_anchoring_transaction(core_schema),
        )
    }

    async fn transaction_with_index(self, index: u64) -> api::Result<Option<btc::Transaction>> {
        Ok(Schema::new(self.0.service_data())
            .transactions_chain
            .get(index))
    }

    async fn transactions_count(self) -> api::Result<AnchoringChainLength> {
        Ok(Schema::new(self.0.service_data())
            .transactions_chain
            .len()
            .into())
    }
}

/// Query parameters for the find transaction request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FindTransactionQuery {
    /// Exonum block height.
    pub height: Option<Height>,
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
        .endpoint("config", |state, _query: ()| ApiImpl(state).config());
    builder
        .private_scope()
        .endpoint_mut("sign-input", |state, query: SignInput| {
            ApiImpl(state).sign_input(query)
        })
        .endpoint_mut("add-funds", |state, query: btc::Transaction| {
            ApiImpl(state).add_funds(query)
        })
        .endpoint("anchoring-proposal", |state, _query: ()| {
            ApiImpl(state).anchoring_proposal()
        })
        .endpoint("config", |state, _query: ()| ApiImpl(state).config())
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
