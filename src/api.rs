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

//! Anchoring rest API implementation.
//!
use exonum::api::{self, ServiceApiBuilder, ServiceApiState};
use exonum::blockchain::{BlockProof, Schema as CoreSchema};
use exonum::crypto::Hash;
use exonum::helpers::Height;
use exonum::storage::{ListProof, MapProof};

use failure::Fail;

use blockchain::BtcAnchoringSchema;
use btc;
use BTC_ANCHORING_SERVICE_ID;

/// Find transaction query parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FindTransactionQuery {
    /// Exonum block height.
    pub height: Option<Height>,
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
    /// Anchoring transaction payload.
    pub payload: btc::Payload,
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
    /// Error type for the current public API implementation
    type Error: Fail;
    /// Returns actual anchoring address.
    ///
    /// `GET /{api_prefix}/v1/address/actual`
    fn actual_address(&self, _query: ()) -> Result<btc::Address, Self::Error>;
    /// Returns the following anchoring address if the node is in a transition state.
    ///
    /// `GET /{api_prefix}/v1/address/following`
    fn following_address(&self, _query: ()) -> Result<Option<btc::Address>, Self::Error>;
    /// Returns for the current anchoring transaction or lookups for the anchoring transaction
    /// with a height greater or equal than the given.
    /// `GET /{api_prefix}/v1/transaction`
    fn find_transaction(
        &self,
        query: FindTransactionQuery,
    ) -> Result<Option<TransactionProof>, Self::Error>;
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
        let core_schema = CoreSchema::new(&snapshot);
        let anchoring_schema = BtcAnchoringSchema::new(&snapshot);
        let tx_chain = anchoring_schema.anchoring_transactions_chain();

        if tx_chain.len() == 0 {
            return Ok(None);
        }

        let tx_index = if let Some(height) = query.height {
            // Handmade binary search
            let get_tx_height = |index| -> Height {
                tx_chain
                    .get(index)
                    .unwrap()
                    .anchoring_payload()
                    .unwrap()
                    .block_height
            };

            let mut base = 0;
            let mut mid = 0;
            let mut size = tx_chain.len();
            while size > 1 {
                let half = size / 2;
                mid = base + half;
                match get_tx_height(mid) {
                    value if value == height => break,
                    value if value < height => base = mid,
                    value if value > height => base = base,
                    _ => unreachable!(),
                }
                size -= half;
            }
            // Don't forget to check base value.
            if get_tx_height(base) == height {
                base
            } else {
                mid
            }
        } else {
            tx_chain.len() - 1
        };

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
            payload: tx_chain.get(tx_index).unwrap().anchoring_payload().unwrap(),
        }))
    }
}

pub(crate) fn wire(builder: &mut ServiceApiBuilder) {
    builder
        .public_scope()
        .endpoint("v1/address/actual", ServiceApiState::actual_address)
        .endpoint("v1/address/following", ServiceApiState::following_address)
        .endpoint("v1/transaction", ServiceApiState::find_transaction);
}
