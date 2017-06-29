//! Anchoring rest api implementation.

use router::Router;
use iron::prelude::*;
use bitcoin::util::base58::ToBase58;

use exonum::blockchain::Blockchain;
use exonum::crypto::Hash;
use exonum::storage::{List, Map};
use exonum::api::{Api, ApiError};

use details::btc;
use details::btc::TxId;
use details::btc::transactions::{AnchoringTx, BitcoinTx, TxKind};
use blockchain::schema::AnchoringSchema;
use blockchain::dto::LectContent;
use observer::Height;

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
        let view = self.blockchain.view();
        let schema = AnchoringSchema::new(&view);
        let actual_cfg = &schema.actual_anchoring_config()?;
        Ok(schema.collect_lects(actual_cfg)?.map(AnchoringInfo::from))
    }

    /// Returns current lect for validator with given `id`.
    ///
    /// `GET /{api_prefix}/v1/actual_lect/:id`
    pub fn current_lect_of_validator(&self, id: u32) -> Result<LectInfo, ApiError> {
        let view = self.blockchain.view();
        let schema = AnchoringSchema::new(&view);

        let actual_cfg = schema.actual_anchoring_config()?;
        if let Some(key) = actual_cfg.anchoring_keys.get(id as usize) {
            if let Some(lect) = schema.lects(key).last()? {
                return Ok(LectInfo::from(lect));
            }
        }
        Err(error::Error::UnknownValidatorId(id).into())
    }

    /// Returns actual anchoring address.
    ///
    /// `GET /{api_prefix}/v1/address/actual`
    pub fn actual_address(&self) -> Result<btc::Address, ApiError> {
        let view = self.blockchain.view();
        let schema = AnchoringSchema::new(&view);
        Ok(schema.actual_anchoring_config()?.redeem_script().1)
    }

    /// Returns the following anchoring address if the node is in a transition state.
    ///
    /// `GET /{api_prefix}/v1/address/actual`
    pub fn following_address(&self) -> Result<Option<btc::Address>, ApiError> {
        let view = self.blockchain.view();
        let schema = AnchoringSchema::new(&view);
        let following_addr = schema
            .following_anchoring_config()?
            .map(|cfg| cfg.redeem_script().1);
        Ok(following_addr)
    }

    /// Returns the anchoring transaction for the nearest block with
    /// a height greater than the given.
    ///
    /// `GET /{api_prefix}/v1/nearest_lect/:height`
    pub fn nearest_lect(&self, height: u64) -> Result<Option<AnchoringTx>, ApiError> {
        let view = self.blockchain.view();
        let anchoring_schema = AnchoringSchema::new(&view);
        let tx_chain = anchoring_schema.anchoring_tx_chain();

        if let Some(nearest_height_bytes) = tx_chain.find_key(&height.into())? {
            let nearest_height = Height::from_vec(nearest_height_bytes);
            let tx = tx_chain.get(&nearest_height)?;
            Ok(tx)
        } else {
            Ok(None)
        }
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
