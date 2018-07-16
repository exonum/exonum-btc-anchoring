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

//! Anchoring rest API implementation.

use exonum::api::{self, ServiceApiBuilder, ServiceApiState};
use exonum::blockchain::{BlockProof, Schema as CoreSchema};
use exonum::crypto::Hash;
use exonum::helpers::Height;
use exonum::storage::{ListProof, MapProof};

use blockchain::dto::LectContent;
use blockchain::schema::AnchoringSchema;
use details::btc;
use details::btc::transactions::{AnchoringTx, BitcoinTx, TxKind};
use details::btc::TxId;
use ANCHORING_SERVICE_ID;

pub use details::btc::payload::Payload;

mod error;

/// Public API implementation.
#[derive(Debug, Clone)]
pub struct PublicApi;

/// API query parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ValidatorQuery {
    /// Validator identifier.
    pub validator_id: u32,
}

/// API query parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HeightQuery {
    /// Exonum block height.
    pub height: u64,
}

/// Public information about the anchoring transaction in bitcoin.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AnchoringInfo {
    /// `Txid` of anchoring transaction.
    pub txid: TxId,
    /// Anchoring transaction payload.
    pub payload: Option<Payload>,
}

/// Public information about the lect transaction in exonum.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct LectInfo {
    /// `Exonum` transaction hash.
    pub hash: Hash,
    /// Information about anchoring transaction.
    pub content: AnchoringInfo,
}

/// A proof of existence for an anchored or a non-anchored Exonum block at the given height.
#[derive(Debug, Serialize, Deserialize)]
pub struct AnchoredBlockHeaderProof {
    /// Latest authorized block in the blockchain.
    pub latest_authorized_block: BlockProof,
    /// Proof for the whole database table.
    pub to_table: MapProof<Hash, Hash>,
    /// Proof for the specific header in this table.
    pub to_block_header: ListProof<Hash>,
}

impl From<BitcoinTx> for AnchoringInfo {
    fn from(tx: BitcoinTx) -> AnchoringInfo {
        match TxKind::from(tx) {
            TxKind::Anchoring(tx) => AnchoringInfo {
                txid: tx.id(),
                payload: Some(tx.payload()),
            },
            TxKind::FundingTx(tx) => AnchoringInfo {
                txid: tx.id(),
                payload: None,
            },
            TxKind::Other(tx) => panic!("Found incorrect lect transaction, content={:#?}", tx),
        }
    }
}

impl From<LectContent> for LectInfo {
    fn from(content: LectContent) -> LectInfo {
        LectInfo {
            hash: *content.msg_hash(),
            content: AnchoringInfo::from(content.tx()),
        }
    }
}

impl PublicApi {
    /// Returns information about the lect agreed by +2/3 validators if there is one.
    ///
    /// `GET /{api_prefix}/v1/actual_lect`
    pub fn actual_lect(state: &ServiceApiState, _query: ()) -> api::Result<Option<AnchoringInfo>> {
        let snapshot = state.snapshot();
        let schema = AnchoringSchema::new(snapshot);
        let actual_cfg = &schema.actual_anchoring_config();
        Ok(schema.collect_lects(actual_cfg).map(AnchoringInfo::from))
    }

    /// Returns current lect for validator with given `id`.
    ///
    /// `GET /{api_prefix}/v1/actual_lect?validator_id={id}`
    pub fn current_lect_of_validator(
        state: &ServiceApiState,
        query: ValidatorQuery,
    ) -> api::Result<LectInfo> {
        let snapshot = state.snapshot();
        let schema = AnchoringSchema::new(snapshot);

        let actual_cfg = schema.actual_anchoring_config();
        if let Some(key) = actual_cfg.anchoring_keys.get(query.validator_id as usize) {
            if let Some(lect) = schema.lects(key).last() {
                return Ok(LectInfo::from(lect));
            }
        }
        Err(error::Error::UnknownValidatorId(query.validator_id).into())
    }

    /// Returns actual anchoring address.
    ///
    /// `GET /{api_prefix}/v1/address/actual`
    pub fn actual_address(state: &ServiceApiState, _query: ()) -> api::Result<btc::Address> {
        let snapshot = state.snapshot();
        let schema = AnchoringSchema::new(snapshot);
        Ok(schema.actual_anchoring_config().redeem_script().1)
    }

    /// Returns the following anchoring address if the node is in a transition state.
    ///
    /// `GET /{api_prefix}/v1/address/following`
    pub fn following_address(
        state: &ServiceApiState,
        _query: (),
    ) -> api::Result<Option<btc::Address>> {
        let snapshot = state.snapshot();
        let schema = AnchoringSchema::new(snapshot);
        let following_addr = schema
            .following_anchoring_config()
            .map(|cfg| cfg.redeem_script().1);
        Ok(following_addr)
    }

    /// Returns hex of the anchoring transaction for the nearest block with a height greater
    /// or equal than the given.
    ///
    /// `GET /{api_prefix}/v1/nearest_lect?height={height}`
    pub fn nearest_lect(
        state: &ServiceApiState,
        query: HeightQuery,
    ) -> api::Result<Option<AnchoringTx>> {
        let snapshot = state.snapshot();
        let anchoring_schema = AnchoringSchema::new(&snapshot);
        let tx_chain = anchoring_schema.anchoring_tx_chain();

        // TODO use binary find.
        for (tx_height, tx) in &tx_chain {
            if tx_height >= query.height {
                return Ok(Some(tx));
            }
        }
        Ok(None)
    }

    /// A method that provides cryptographic proofs for Exonum blocks including those anchored to
    /// Bitcoin blockchain. The proof is an apparent evidence of availability of a certain Exonum
    /// block in the blockchain.
    ///
    /// `GET /{api_prefix}/v1/block_header_proof?height={height}`
    pub fn anchored_block_header_proof(
        state: &ServiceApiState,
        query: HeightQuery,
    ) -> api::Result<AnchoredBlockHeaderProof> {
        let view = state.snapshot();
        let core_schema = CoreSchema::new(&view);
        let anchoring_schema = AnchoringSchema::new(&view);

        let max_height = core_schema.block_hashes_by_height().len() - 1;

        let latest_authorized_block = core_schema
            .block_and_precommits(Height(max_height))
            .unwrap();
        let to_table: MapProof<Hash, Hash> =
            core_schema.get_proof_to_service_table(ANCHORING_SERVICE_ID, 0);
        let to_block_header = anchoring_schema.anchored_blocks().get_proof(query.height);

        Ok(AnchoredBlockHeaderProof {
            latest_authorized_block,
            to_table,
            to_block_header,
        })
    }
}

pub(crate) fn wire(builder: &mut ServiceApiBuilder) {
    builder
        .public_scope()
        .endpoint("v1/actual_lect", PublicApi::actual_lect)
        .endpoint(
            "v1/actual_lect/validator",
            PublicApi::current_lect_of_validator,
        )
        .endpoint("v1/address/actual", PublicApi::actual_address)
        .endpoint("v1/address/following", PublicApi::following_address)
        .endpoint("v1/nearest_lect", PublicApi::nearest_lect)
        .endpoint(
            "v1/block_header_proof",
            PublicApi::anchored_block_header_proof,
        );
}
