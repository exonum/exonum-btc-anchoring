//! Anchoring transactions' chain observer.

use std::time::Duration;
use std::thread::sleep;

use bitcoin::util::base58::ToBase58;

use exonum::blockchain::{Blockchain, Schema};
use exonum::storage::{Fork, List, Map, MapTable, U64Key, View};

use details::rpc::{AnchoringRpc, AnchoringRpcConfig};
use details::btc::transactions::{AnchoringTx, BitcoinTx, TxKind};
use blockchain::schema::AnchoringSchema;
use blockchain::consensus_storage::AnchoringConfig;
use error::Error as ServiceError;

/// Type alias for milliseconds.
pub type Milliseconds = u64;
/// Type alias for block height.
pub type Height = U64Key;

/// Anchoring observer configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnchoringObserverConfig {
    /// A frequency of anchoring chain checks.
    pub check_frequency: Milliseconds,
}

/// Anchoring chain observer. Periodically checks the state of the anchor chain and keeps
/// the verified transactions in database.
#[derive(Debug)]
pub struct AnchoringChainObserver {
    blockchain: Blockchain,
    client: AnchoringRpc,
    check_frequency: Milliseconds,
}

impl<'a> AnchoringSchema<'a> {
    /// Returns table that maps anchoring transactions to their heights.
    pub fn anchoring_tx_chain(&self) -> MapTable<View, Height, AnchoringTx> {
        let prefix = self.gen_table_prefix(128, None);
        MapTable::new(prefix, self.view)
    }
}

impl AnchoringChainObserver {
    /// Constructs observer for the given `blockchain`.
    pub fn new(blockchain: Blockchain,
               rpc: AnchoringRpcConfig,
               observer: AnchoringObserverConfig)
               -> AnchoringChainObserver {
        AnchoringChainObserver {
            blockchain: blockchain,
            client: AnchoringRpc::new(rpc),
            check_frequency: observer.check_frequency,
        }
    }

    #[doc(hidden)]
    pub fn new_with_client(blockchain: Blockchain,
                           client: AnchoringRpc,
                           check_frequency: Milliseconds)
                           -> AnchoringChainObserver {
        AnchoringChainObserver {
            blockchain: blockchain,
            client: client,
            check_frequency: check_frequency,
        }
    }

    /// Runs obesrver in infinity loop.
    pub fn run(&self) -> Result<(), ServiceError> {
        info!("Launching anchoring chain observer with polling frequency {} ms",
              self.check_frequency);
        let duration = Duration::from_millis(self.check_frequency);
        loop {
            if let Err(e) = self.check_anchoring_chain() {
                error!("An error during `check_anchoring_chain` occured, msg={:?}",
                       e);
            }
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
            if !self.lect_payload_is_correct(&view, &lect)? {
                error!("Received lect with incorrect payload, content={:#?}", lect);
                return Ok(());
            }

            self.update_anchoring_chain(&view, &cfg, lect)?;
            let changes = view.changes();
            self.blockchain.merge(&changes)?;
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn client(&self) -> &AnchoringRpc {
        &self.client
    }

    #[doc(hidden)]
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
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

            if anchoring_schema.anchoring_tx_chain().get(&height)?.as_ref() == Some(&lect) {
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
            if self.transaction_is_lect(view, actual_cfg, &tx)? {
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
        for key in &actual_cfg.anchoring_keys {
            if anchoring_schema.find_lect_position(key, &txid)?.is_some() {
                lect_count += 1;
            }
        }
        Ok(lect_count >= actual_cfg.majority_count())
    }

    fn lect_payload_is_correct(&self,
                               view: &View,
                               lect: &AnchoringTx)
                               -> Result<bool, ServiceError> {
        let core_schema = Schema::new(view);
        let payload = lect.payload();
        let block_hash = core_schema.block_hash_by_height(payload.block_height)?;
        Ok(block_hash == Some(payload.block_hash))
    }

    fn is_blockchain_inited(&self, view: &View) -> Result<bool, ServiceError> {
        let schema = Schema::new(view);
        let len = schema.block_hashes_by_height().len()?;
        Ok(len > 0)
    }
}
