use router::Router;
use iron::prelude::*;
use serde_json;

use exonum::blockchain::Blockchain;
use exonum::crypto::Hash;
use exonum::storage::List;
use exonum::api::{Api, ApiError};

use details::btc::TxId;
use details::btc::transactions::{BitcoinTx, TxKind};
use blockchain::schema::AnchoringSchema;
use blockchain::dto::LectContent;

pub use details::btc::payload::Payload;

mod error;

/// Public api implementation.
#[derive(Clone)]
pub struct PublicApi {
    pub blockchain: Blockchain,
}

/// Public information about the anchoring transaction in `bitcoin`
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AnchoringInfo {
    /// `Txid` of anchoring transaction.
    pub txid: TxId,
    /// Anchoring transaction payload
    pub payload: Option<Payload>,
}

/// Public information about the `lect` transaction in `exonum`
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct LectInfo {
    /// `Exonum` transaction hash
    pub hash: Hash,
    /// Information about anchoring transaction
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
    /// `GET /{api_prefix}/v1/current_lect/`
    pub fn current_lect(&self) -> Result<Option<AnchoringInfo>, ApiError> {
        let view = self.blockchain.view();
        let schema = AnchoringSchema::new(&view);
        Ok(schema.collect_lects()?.map(AnchoringInfo::from))
    }

    /// Returns current lect for validator with given `id`.
    ///
    /// `GET /{api_prefix}/v1/current_lect/:id`
    pub fn current_lect_of_validator(&self, id: u32) -> Result<LectInfo, ApiError> {
        let view = self.blockchain.view();
        let schema = AnchoringSchema::new(&view);

        if let Some(lect) = schema.lects(id).last()? {
            Ok(LectInfo::from(lect))
        } else {
            Err(error::Error::UnknownValidatorId(id).into())
        }
    }
}

impl Api for PublicApi {
    fn wire(&self, router: &mut Router) {
        let _self = self.clone();
        let current_lect = move |_: &mut Request| -> IronResult<Response> {
            let lect = _self.current_lect()?;
            _self.ok_response(&serde_json::to_value(&lect).unwrap())
        };

        let _self = self.clone();
        let current_lect_of_validator = move |req: &mut Request| -> IronResult<Response> {
            let map = req.extensions.get::<Router>().unwrap();
            match map.find("id") {
                Some(id_str) => {
                    let id: u32 = id_str.parse().map_err(|_| ApiError::IncorrectRequest)?;
                    let info = _self.current_lect_of_validator(id)?;
                    _self.ok_response(&serde_json::to_value(&info).unwrap())
                }
                None => Err(ApiError::IncorrectRequest)?,
            }
        };

        router.get("/v1/current_lect/", current_lect, "current_lect");
        router.get("/v1/current_lect/:id",
                   current_lect_of_validator,
                   "current_lect_of_validator");
    }
}
