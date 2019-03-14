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

//! Anchoring HTTP API implementation.

use exonum::api::{self, ServiceApiBuilder, ServiceApiState};
use exonum::blockchain::{BlockProof, Schema as CoreSchema};
use exonum::crypto::Hash;
use exonum::helpers::Height;
use exonum::storage::{ListProof, MapProof};

use failure::Fail;

use std::cmp::{
    self,
    Ordering::{self, Equal, Greater, Less},
};

use crate::blockchain::BtcAnchoringSchema;
use crate::btc;
use crate::BTC_ANCHORING_SERVICE_ID;

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
    pub height: u64,
}

/// A proof of existence for an anchoring transaction at the given height.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionProof {
    /// Latest authorized block in the blockchain.
    pub latest_authorized_block: BlockProof,
    /// Proof for the whole database table.
    pub to_table: MapProof<Hash, Hash>,
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
    pub to_table: MapProof<Hash, Hash>,
    /// Proof for the specific header in this table.
    pub to_block_header: ListProof<Hash>,
}

/// Public API specification for the Exonum Bitcoin anchoring service.
pub trait PublicApi {
    /// Error type for the current public API implementation.
    type Error: Fail;

    /// Returns actual anchoring address.
    ///
    /// `GET /{api_prefix}/v1/address/actual`
    fn actual_address(&self, _query: ()) -> Result<btc::Address, Self::Error>;

    /// Returns the following anchoring address if the node is in the transition state.
    ///
    /// `GET /{api_prefix}/v1/address/following`
    fn following_address(&self, _query: ()) -> Result<Option<btc::Address>, Self::Error>;

    /// Returns the latest anchoring transaction if the height is not specified,
    /// otherwise, returns the anchoring transaction with the height that is greater or equal
    /// to the given one.
    ///
    /// `GET /{api_prefix}/v1/transaction`
    fn find_transaction(
        &self,
        query: FindTransactionQuery,
    ) -> Result<Option<TransactionProof>, Self::Error>;

    /// A method that provides cryptographic proofs for Exonum blocks including those anchored to
    /// Bitcoin blockchain. The proof is an apparent evidence of availability of a certain Exonum
    /// block in the blockchain.
    ///
    /// `GET /{api_prefix}/v1/block_header_proof?height={height}`
    fn block_header_proof(&self, query: HeightQuery) -> Result<BlockHeaderProof, Self::Error>;
}

impl PublicApi for ServiceApiState {
    type Error = api::Error;

    fn actual_address(&self, _query: ()) -> Result<btc::Address, Self::Error> {
        let snapshot = self.snapshot();
        let schema = BtcAnchoringSchema::new(snapshot);
        Ok(schema.actual_configuration().anchoring_address())
    }

    fn following_address(&self, _query: ()) -> Result<Option<btc::Address>, Self::Error> {
        let snapshot = self.snapshot();
        let schema = BtcAnchoringSchema::new(snapshot);
        Ok(schema
            .following_configuration()
            .map(|config| config.anchoring_address()))
    }

    fn find_transaction(
        &self,
        query: FindTransactionQuery,
    ) -> Result<Option<TransactionProof>, Self::Error> {
        let snapshot = self.snapshot();
        let anchoring_schema = BtcAnchoringSchema::new(&snapshot);
        let tx_chain = anchoring_schema.anchoring_transactions_chain();

        if tx_chain.is_empty() {
            return Ok(None);
        }

        let tx_index = if let Some(height) = query.height {
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

        let core_schema = CoreSchema::new(&snapshot);
        let max_height = core_schema.block_hashes_by_height().len() - 1;
        let latest_authorized_block = core_schema
            .block_and_precommits(Height(max_height))
            .unwrap();
        let to_table: MapProof<Hash, Hash> =
            core_schema.get_proof_to_service_table(BTC_ANCHORING_SERVICE_ID, 0);
        let to_transaction = tx_chain.get_proof(tx_index);

        Ok(Some(TransactionProof {
            latest_authorized_block,
            to_table,
            to_transaction,
            transactions_count: tx_chain.len(),
        }))
    }

    fn block_header_proof(&self, query: HeightQuery) -> Result<BlockHeaderProof, Self::Error> {
        let view = self.snapshot();
        let core_schema = CoreSchema::new(&view);
        let anchoring_schema = BtcAnchoringSchema::new(&view);

        let max_height = core_schema.block_hashes_by_height().len() - 1;

        let latest_authorized_block = core_schema
            .block_and_precommits(Height(max_height))
            .unwrap();
        let to_table: MapProof<Hash, Hash> =
            core_schema.get_proof_to_service_table(BTC_ANCHORING_SERVICE_ID, 3);
        let to_block_header = anchoring_schema.anchored_blocks().get_proof(query.height);

        Ok(BlockHeaderProof {
            latest_authorized_block,
            to_table,
            to_block_header,
        })
    }
}

pub(crate) fn wire(builder: &mut ServiceApiBuilder) {
    builder
        .public_scope()
        .endpoint("v1/address/actual", ServiceApiState::actual_address)
        .endpoint("v1/address/following", ServiceApiState::following_address)
        .endpoint("v1/transaction", ServiceApiState::find_transaction)
        .endpoint("v1/block_header_proof", ServiceApiState::block_header_proof);
}
