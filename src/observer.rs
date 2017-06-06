//! An anchoring transactions' chain observer.

use std::time::Duration;
use std::thread::sleep;

use byteorder::{BigEndian, ByteOrder};
use bitcoin::util::base58::ToBase58;
use iron::Handler;
use router::Router;

use exonum::blockchain::Blockchain;
use exonum::storage::{Map, MapTable, MerkleTable, View};

use details::rpc::{AnchoringRpc, AnchoringRpcConfig};
use details::btc::transactions::{AnchoringTx, BitcoinTx, TxKind};
use blockchain::schema::AnchoringSchema;
use blockchain::consensus_storage::AnchoringConfig;
use error::Error as ServiceError;

pub type Milliseconds = u64;

/// An anchoring observer configuration.
#[derive(Debug, Deserialize)]
pub struct ObserverConfig {
    /// A frequency of anchoring chain checks.
    pub check_frequency: Milliseconds,
    /// A rpc configuration.
    pub rpc: AnchoringRpcConfig,
}

/// An anchoring chain observer. Periodically checks the state of the anchor chain and keeps
/// the verified transactions in database.
pub struct AnchoringChainObserver {
    blockchain: Blockchain,
    client: AnchoringRpc,
    check_frequency: Milliseconds,
}

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
        info!("Anchoring chain observer runs with check frequency: {}ms", self.check_frequency);
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
        let anchoring_schema = AnchoringSchema::new(&view);
        let cfg = anchoring_schema.actual_anchoring_config()?;

        if let Some(lect) = self.find_lect(&view, &cfg)? {
            self.update_anchoring_chain(&view, &cfg, lect)?;
        }
        Ok(())
    }

    pub fn returns_api_handler(&self) -> Box<Handler> {
        unimplemented!();
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
            if anchoring_schema.find_lect_position(key, &txid)?.is_some() {
                lect_count += 1;
            }
        }
        Ok(lect_count >= actual_cfg.majority_count())
    }
}