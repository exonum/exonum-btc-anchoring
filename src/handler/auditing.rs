use std::collections::hash_map::{HashMap, Entry};

use exonum::blockchain::{NodeState, Schema};
use exonum::storage::List;

use error::Error as ServiceError;
use details::btc::transactions::{AnchoringTx, FundingTx, TxKind};
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::schema::AnchoringSchema;

use super::{AnchoringHandler, LectKind};
use super::error::Error as HandlerError;

#[doc(hidden)]
impl AnchoringHandler {
    pub fn handle_auditing_state(&mut self,
                                 cfg: AnchoringConfig,
                                 state: &NodeState)
                                 -> Result<(), ServiceError> {
        trace!("Auditing state");
        if state.height() % self.node.check_lect_frequency == 0 {
            // Find lect
            let lect = {
                let mut lects = HashMap::new();
                let anchoring_schema = AnchoringSchema::new(state.view());
                let validators_count = cfg.validators.len() as u32;
                for validator_id in 0..validators_count {
                    if let Some(last_lect) = anchoring_schema.lects(validator_id).last()? {
                        // TODO implement hash and eq for transaction
                        match lects.entry(last_lect.0) {
                            Entry::Occupied(mut v) => {
                                *v.get_mut() += 1;
                            }
                            Entry::Vacant(v) => {
                                v.insert(1);
                            }
                        }
                    }
                }

                if let Some((lect, count)) = lects.iter().max_by_key(|&(_, v)| v) {
                    if *count >= ::majority_count(validators_count as u8) {
                        match TxKind::from(lect.clone()) {
                            TxKind::Anchoring(tx) => LectKind::Anchoring(tx),
                            TxKind::FundingTx(tx) => LectKind::Funding(tx),
                            TxKind::Other(tx) => {
                                let e = HandlerError::IncorrectLect {
                                    reason: "Incorrect lect transaction".to_string(),
                                    tx: tx.into(),
                                };
                                return Err(e.into());
                            }
                        }
                    } else {
                        LectKind::None
                    }
                } else {
                    LectKind::None
                }
            };

            let r = match lect {
                LectKind::Funding(tx) => self.check_funding_lect(tx, &cfg, state),
                LectKind::Anchoring(tx) => self.check_anchoring_lect(tx, &cfg, state),
                LectKind::None => Err(HandlerError::LectNotFound.into()),
            };
            return r;
        }
        Ok(())
    }

    fn check_funding_lect(&self,
                          tx: FundingTx,
                          cfg: &AnchoringConfig,
                          _: &NodeState)
                          -> Result<(), ServiceError> {
        let (_, addr) = cfg.redeem_script();
        if tx != cfg.funding_tx {
            let e = HandlerError::IncorrectLect {
                reason: "Initial funding_tx is different than lect".to_string(),
                tx: tx.into(),
            };
            return Err(e.into());
        }
        if tx.find_out(&addr).is_some() {
            let e = HandlerError::IncorrectLect {
                reason: "Initial funding_tx have wrong output address".to_string(),
                tx: tx.into(),
            };
            return Err(e.into());
        }

        // Checks with access to the `bitcoind`
        if let Some(ref client) = self.client {
            if client.get_transaction(&tx.txid())?.is_none() {
                let e = HandlerError::IncorrectLect {
                    reason: "Initial funding_tx does not exists".to_string(),
                    tx: tx.into(),
                };
                return Err(e.into());
            }
        }
        info!("CHECKED_INITIAL_LECT ====== txid={}", tx.txid());
        Ok(())
    }

    fn check_anchoring_lect(&self,
                            tx: AnchoringTx,
                            cfg: &AnchoringConfig,
                            state: &NodeState)
                            -> Result<(), ServiceError> {
        // Verify tx content
        self.check_anchoring_tx_content(&tx, cfg, state)?;
        // Checks with access to the `bitcoind`
        if let Some(ref client) = self.client {
            if client.get_transaction(&tx.txid())?.is_none() {
                let e = HandlerError::IncorrectLect {
                    reason: "Lect does not exists in the bitcoin blockchain".to_string(),
                    tx: tx.into(),
                };
                return Err(e.into());
            }

            // Get previous tx
            let prev_txid = tx.prev_hash().be_hex_string();
            let prev_tx = if let Some(tx) = client.get_transaction(&prev_txid)? {
                tx
            } else {
                let e = HandlerError::IncorrectLect {
                    reason: "Lect's input does not exists in the bitcoin blockchain".to_string(),
                    tx: tx.into(),
                };
                return Err(e.into());
            };

            match TxKind::from(prev_tx) {
                TxKind::Anchoring(prev_tx) => {
                    // TODO Disabled due for lack of `get_configuration_at_height` method in core schema.
                    // self.check_anchoring_tx_content(&prev_tx, cfg, state)?;
                    // TODO Check that we did not miss more than one anchored height
                }
                TxKind::FundingTx(prev_tx) => {
                    self.check_funding_lect(prev_tx, cfg, state)?;
                }
                TxKind::Other(tx) => {
                    let e = HandlerError::IncorrectLect {
                        reason: "Weird input transaction".to_string(),
                        tx: tx.into(),
                    };
                    return Err(e.into());
                }
            }
        }
        info!("CHECKED_LECT ====== txid={}", tx.txid());
        Ok(())
    }

    fn check_anchoring_tx_content(&self,
                                  tx: &AnchoringTx,
                                  cfg: &AnchoringConfig,
                                  state: &NodeState)
                                  -> Result<(), ServiceError> {
        let anchoring_schema = AnchoringSchema::new(state.view());
        // Check that tx address is correct
        let tx_addr = tx.output_address(cfg.network);
        let addr = if let Some(following) = anchoring_schema.following_anchoring_config()? {
            following.config.redeem_script().1
        } else {
            cfg.redeem_script().1
        };

        if tx_addr != addr {
            let e = HandlerError::IncorrectLect {
                reason: "Found lect with wrong output_address".to_string(),
                tx: tx.clone().into(),
            };
            return Err(e.into());
        }
        // Payload checks
        let (block_height, block_hash) = tx.payload();
        let schema = Schema::new(state.view());
        if Some(block_hash) != schema.heights().get(block_height)? {
            let e = HandlerError::IncorrectLect {
                reason: "Found lect with wrong payload".to_string(),
                tx: tx.clone().into(),
            };
            return Err(e.into());
        }
        Ok(())
    }
}
