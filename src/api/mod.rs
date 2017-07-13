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

//! Anchoring rest api implementation.

use router::Router;
use iron::prelude::*;
use bitcoin::util::base58::ToBase58;

use exonum::blockchain::Blockchain;
use exonum::crypto::Hash;
use exonum::api::{Api, ApiError};

use details::btc;
use details::btc::TxId;
use details::btc::transactions::{AnchoringTx, BitcoinTx, TxKind};
use blockchain::schema::AnchoringSchema;
use blockchain::dto::LectContent;

pub use details::btc::payload::Payload;

mod error;

/// Public api implementation.
#[derive(Debug, Clone)]
pub struct PublicApi {
    /// Exonum blockchain instance.
    pub blockchain: Blockchain,
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

impl From<BitcoinTx> for AnchoringInfo {
    fn from(tx: BitcoinTx) -> AnchoringInfo {
        match TxKind::from(tx) {
            TxKind::Anchoring(tx) => {
                AnchoringInfo {
                    txid: tx.id(),
                    payload: Some(tx.payload()),
                }
            }
            TxKind::FundingTx(tx) => {
                AnchoringInfo {
                    txid: tx.id(),
                    payload: None,
                }
            }
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
    /// `GET /{api_prefix}/v1/actual_lect/`
    pub fn actual_lect(&self) -> Result<Option<AnchoringInfo>, ApiError> {
        let snapshot = self.blockchain.snapshot();
        let schema = AnchoringSchema::new(snapshot);
        let actual_cfg = &schema.actual_anchoring_config();
        Ok(schema.collect_lects(actual_cfg).map(AnchoringInfo::from))
    }

    /// Returns current lect for validator with given `id`.
    ///
    /// `GET /{api_prefix}/v1/actual_lect/:id`
    pub fn current_lect_of_validator(&self, id: u32) -> Result<LectInfo, ApiError> {
        let snapshot = self.blockchain.snapshot();
        let schema = AnchoringSchema::new(snapshot);

        let actual_cfg = schema.actual_anchoring_config();
        if let Some(key) = actual_cfg.anchoring_keys.get(id as usize) {
            if let Some(lect) = schema.lects(key).last() {
                return Ok(LectInfo::from(lect));
            }
        }
        Err(error::Error::UnknownValidatorId(id).into())
    }

    /// Returns actual anchoring address.
    ///
    /// `GET /{api_prefix}/v1/address/actual`
    pub fn actual_address(&self) -> Result<btc::Address, ApiError> {
        let snapshot = self.blockchain.snapshot();
        let schema = AnchoringSchema::new(snapshot);
        Ok(schema.actual_anchoring_config().redeem_script().1)
    }

    /// Returns the following anchoring address if the node is in a transition state.
    ///
    /// `GET /{api_prefix}/v1/address/actual`
    pub fn following_address(&self) -> Result<Option<btc::Address>, ApiError> {
        let snapshot = self.blockchain.snapshot();
        let schema = AnchoringSchema::new(snapshot);
        let following_addr = schema
            .following_anchoring_config()
            .map(|cfg| cfg.redeem_script().1);
        Ok(following_addr)
    }

    /// Returns hex of the anchoring transaction for the nearest block with a height greater
    /// or equal than the given.
    ///
    /// `GET /{api_prefix}/v1/nearest_lect/:height`
    pub fn nearest_lect(&self, height: u64) -> Result<Option<AnchoringTx>, ApiError> {
        let snapshot = self.blockchain.snapshot();
        let anchoring_schema = AnchoringSchema::new(&snapshot);
        let tx_chain = anchoring_schema.anchoring_tx_chain();

        // TODO use binary find.
        for (tx_height, tx) in &tx_chain {
            if tx_height >= height {
                return Ok(Some(tx));
            }
        }
        Ok(None)
    }
}

impl Api for PublicApi {
    fn wire(&self, router: &mut Router) {
        let _self = self.clone();
        let actual_lect = move |_: &mut Request| -> IronResult<Response> {
            let lect = _self.actual_lect()?;
            _self.ok_response(&json!(lect))
        };

        let _self = self.clone();
        let current_lect_of_validator = move |req: &mut Request| -> IronResult<Response> {
            let map = req.extensions.get::<Router>().unwrap();
            match map.find("id") {
                Some(id_str) => {
                    let id: u32 = id_str
                        .parse()
                        .map_err(|e| {
                                     let msg = format!("An error during parsing of the validator \
                                                        id occurred: {}",
                                                       e);
                                     ApiError::IncorrectRequest(msg.into())
                                 })?;
                    let info = _self.current_lect_of_validator(id)?;
                    _self.ok_response(&json!(info))
                }
                None => {
                    let msg = "The identifier of the validator is not specified.";
                    Err(ApiError::IncorrectRequest(msg.into()))?
                }
            }
        };

        let _self = self.clone();
        let actual_address = move |_: &mut Request| -> IronResult<Response> {
            let addr = _self.actual_address()?.to_base58check();
            _self.ok_response(&json!(addr))
        };

        let _self = self.clone();
        let following_address = move |_: &mut Request| -> IronResult<Response> {
            let addr = _self.following_address()?.map(|addr| addr.to_base58check());
            _self.ok_response(&json!(addr))
        };

        let _self = self.clone();
        let nearest_lect = move |req: &mut Request| -> IronResult<Response> {
            let map = req.extensions.get::<Router>().unwrap();
            match map.find("height") {
                Some(height_str) => {
                    let height: u64 = height_str
                        .parse()
                        .map_err(|e| {
                                     let msg = format!("An error during parsing of the block \
                                                        height occurred: {}",
                                                       e);
                                     ApiError::IncorrectRequest(msg.into())
                                 })?;
                    let lect = _self.nearest_lect(height)?;
                    _self.ok_response(&json!(lect))
                }
                None => {
                    let msg = "The block height is not specified.";
                    Err(ApiError::IncorrectRequest(msg.into()))?
                }
            }
        };

        router.get("/v1/address/actual", actual_address, "actual_address");
        router.get("/v1/address/following",
                   following_address,
                   "following_address");
        router.get("/v1/actual_lect/", actual_lect, "actual_lect");
        router.get("/v1/actual_lect/:id",
                   current_lect_of_validator,
                   "current_lect_of_validator");
        router.get("/v1/nearest_lect/:height", nearest_lect, "nearest_lect");
    }
}
