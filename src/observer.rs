//! An anchoring transactions' chain observer.

use std::time::Duration;
use std::thread::sleep;
use std::net::SocketAddr;

use bitcoin::util::base58::ToBase58;
use byteorder::{BigEndian, ByteOrder};
use router::Router;
use iron::prelude::*;
use iron::Handler;

use exonum::api::{Api, ApiError};
use exonum::blockchain::{Blockchain, Schema};
use exonum::storage::{Fork, List, Map, MapTable, View};

use details::rpc::{AnchoringRpc, AnchoringRpcConfig};
use details::btc::transactions::{AnchoringTx, BitcoinTx, TxKind};
use blockchain::schema::AnchoringSchema;
use blockchain::consensus_storage::AnchoringConfig;
use error::Error as ServiceError;

pub type Milliseconds = u64;

/// An anchoring observer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserverConfig {
    /// A frequency of anchoring chain checks.
    pub check_frequency: Milliseconds,
    /// A rpc configuration.
    pub rpc: AnchoringRpcConfig,
    /// A listen address for api.
    pub api_address: SocketAddr,
}

/// An anchoring chain observer. Periodically checks the state of the anchor chain and keeps
/// the verified transactions in database.
pub struct AnchoringChainObserver {
    blockchain: Blockchain,
    client: AnchoringRpc,
    check_frequency: Milliseconds,
}

/// An anchoring chain observer api implementation.
#[derive(Clone)]
pub struct AnchoringChainObserverApi {
    pub blockchain: Blockchain,
}

#[derive(Debug, Clone)]
struct Height([u8; 8]);

impl Into<Height> for u64 {
    fn into(self) -> Height {
        let mut bytes = [0; 8];
        BigEndian::write_u64(&mut bytes, self);
        Height(bytes)
    }
}

impl Into<u64> for Height {
    fn into(self) -> u64 {
        BigEndian::read_u64(&self.0)
    }
}

impl AsRef<[u8]> for Height {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl<'a> AnchoringSchema<'a> {
    fn anchoring_tx_chain(&self) -> MapTable<View, Height, AnchoringTx> {
        let prefix = self.gen_table_prefix(128, None);
        MapTable::new(prefix, self.view)
    }
}

impl AnchoringChainObserver {
    /// Constructs observer for the given `blockchain`.
    pub fn new(config: ObserverConfig, blockchain: Blockchain) -> AnchoringChainObserver {
        AnchoringChainObserver {
            blockchain: blockchain,
            client: AnchoringRpc::new(config.rpc),
            check_frequency: config.check_frequency,
        }
    }

    /// Runs obesrver in infinity loop.
    pub fn run(&self) -> Result<(), ServiceError> {
        info!("Anchoring chain observer runs with check frequency: {}ms",
              self.check_frequency);
        let duration = Duration::from_millis(self.check_frequency);
        loop {
            self.check_anchoring_chain()?;
            sleep(duration);
        }
    }

    /// Tries to get `lect` for the current anchoring configuration and retrospectively adds
    /// all previosly unknown anchoring transactions.
    pub fn check_anchoring_chain(&self) -> Result<(), ServiceError> {
        let view = self.blockchain.view();
        if !self.is_blockchain_inited(&view)? {
            return Ok(());
        }

        let anchoring_schema = AnchoringSchema::new(&view);
        let cfg = anchoring_schema.actual_anchoring_config()?;
        if let Some(lect) = self.find_lect(&view, &cfg)? {
            self.update_anchoring_chain(&view, &cfg, lect)?;

            let changes = view.changes();
            self.blockchain.merge(&changes)?;
        }
        Ok(())
    }

    /// Returns an api handler.
    pub fn api_handler(&self) -> Box<Handler> {
        let mut router = Router::new();
        let api = AnchoringChainObserverApi { blockchain: self.blockchain.clone() };
        api.wire(&mut router);
        Box::new(router)
    }

    fn update_anchoring_chain(&self,
                              view: &View,
                              actual_cfg: &AnchoringConfig,
                              mut lect: AnchoringTx)
                              -> Result<(), ServiceError> {
        let anchoring_schema = AnchoringSchema::new(view);

        loop {
            let payload = lect.payload();
            let height = payload.block_height.into();

            if anchoring_schema
                   .anchoring_tx_chain()
                   .get(&height)?
                   .is_some() {
                return Ok(());
            }

            let confirmations = self.client.get_transaction_confirmations(&lect.id())?;
            if confirmations.as_ref() >= Some(&actual_cfg.utxo_confirmations) {
                trace!("Adds transaction to chain, height={}, content={:#?}",
                       payload.block_height,
                       lect);

                anchoring_schema
                    .anchoring_tx_chain()
                    .put(&height, lect.clone().into())?;
            }

            let prev_txid = payload.prev_tx_chain.unwrap_or_else(|| lect.prev_hash());
            if let Some(prev_tx) = self.client.get_transaction(&prev_txid.be_hex_string())? {
                lect = match TxKind::from(prev_tx) {
                    TxKind::Anchoring(lect) => lect,
                    TxKind::FundingTx(_) => return Ok(()),
                    TxKind::Other(tx) => {
                        panic!("Found incorrect lect transaction, content={:#?}", tx)
                    }
                }
            } else {
                return Ok(());
            }
        }
    }

    fn find_lect(&self,
                 view: &View,
                 actual_cfg: &AnchoringConfig)
                 -> Result<Option<AnchoringTx>, ServiceError> {
        let actual_addr = actual_cfg.redeem_script().1;

        trace!("Tries to find lect for the addr: {}",
               actual_addr.to_base58check());

        let unspent_txs: Vec<_> = self.client.unspent_transactions(&actual_addr)?;
        for tx in unspent_txs {
            if self.transaction_is_lect(&view, &actual_cfg, &tx)? {
                if let TxKind::Anchoring(lect) = TxKind::from(tx) {
                    return Ok(Some(lect));
                }
            }
        }
        Ok(None)
    }

    fn transaction_is_lect(&self,
                           view: &View,
                           actual_cfg: &AnchoringConfig,
                           tx: &BitcoinTx)
                           -> Result<bool, ServiceError> {
        let txid = tx.id();
        let anchoring_schema = AnchoringSchema::new(view);

        let mut lect_count = 0;
        for key in &actual_cfg.validators {
            if anchoring_schema
                   .find_lect_position(key, &txid)?
                   .is_some() {
                lect_count += 1;
            }
        }
        Ok(lect_count >= actual_cfg.majority_count())
    }

    fn is_blockchain_inited(&self, view: &View) -> Result<bool, ServiceError> {
        let schema = Schema::new(view);
        let len = schema.block_hashes_by_height().len()?;
        Ok(len > 0)
    }
}

impl AnchoringChainObserverApi {
    /// Returns the anchoring transaction for the nearest block with
    /// a height greater than the given.
    pub fn nearest_lect(&self, height: u64) -> Result<Option<AnchoringTx>, ApiError> {
        let view = self.blockchain.view();
        let anchoring_schema = AnchoringSchema::new(&view);
        let tx_chain = anchoring_schema.anchoring_tx_chain();

        // dump lects
        {
            let mut height = 0;
            let h: Height = height.into();
            debug!("Begin dump heights, height={:?}", h);
            while let Some(nearest_height_bytes) = tx_chain.find_key(&height.into())? {
                let nearest_height = {
                    let mut buf = [0; 8];
                    buf.copy_from_slice(&nearest_height_bytes);
                    Height(buf)
                };

                let tx = tx_chain.get(&nearest_height)?;
                height = nearest_height.into();
                debug!("height={}, tx={:#?}", height, tx);
            }
        }

        if let Some(nearest_height_bytes) = tx_chain.find_key(&height.into())? {
            let nearest_height = {
                let mut buf = [0; 8];
                buf.copy_from_slice(&nearest_height_bytes);
                Height(buf)
            };
            let h: u64 = height.into();
            debug!("nearest height_bytes={:?}, height={}", nearest_height, h);

            let tx = tx_chain.get(&nearest_height)?;
            Ok(tx)
        } else {
            Ok(None)
        }
    }
}

impl Api for AnchoringChainObserverApi {
    fn wire(&self, router: &mut Router) {
        let _self = self.clone();
        let nearest_lect = move |req: &mut Request| -> IronResult<Response> {
            let map = req.extensions.get::<Router>().unwrap();
            match map.find("height") {
                Some(height_str) => {
                    let height: u64 = height_str
                        .parse()
                        .map_err(|_| ApiError::IncorrectRequest)?;
                    let lect = _self.nearest_lect(height)?;
                    _self.ok_response(&json!(lect))
                }
                None => Err(ApiError::IncorrectRequest)?,
            }
        };

        router.get("/v1/nearest_lect/:height", nearest_lect, "nearest_lect");
    }
}
