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
                            TxKind::Other(_) => LectKind::None,
                        }
                    } else {
                        LectKind::None
                    }
                } else {
                    LectKind::None
                }
            };

            let r = match lect {
                LectKind::Funding(tx) => self.check_funding_lect(tx, cfg, state),
                LectKind::Anchoring(tx) => self.check_anchoring_lect(tx, cfg, state),
                LectKind::None => Err(HandlerError::LectNotFound.into()),
            };
            return r;
        }
        Ok(())
    }

    fn check_funding_lect(&self,
                          tx: FundingTx,
                          cfg: AnchoringConfig,
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
        info!("CHECKED_INITIAL_LECT ====== txid={}", tx.txid());
        Ok(())
    }

    fn check_anchoring_lect(&self,
                            tx: AnchoringTx,
                            cfg: AnchoringConfig,
                            state: &NodeState)
                            -> Result<(), ServiceError> {
        let anchoring_schema = AnchoringSchema::new(state.view());
        let (_, addr) = cfg.redeem_script();
        // Check that tx address is correct
        let tx_addr = tx.output_address(cfg.network);
        if tx_addr != addr {
            let is_address_correct = {
                if let Some(following) = anchoring_schema.following_anchoring_config()? {
                    tx_addr == following.config.redeem_script().1
                } else {
                    false
                }
            };

            if !is_address_correct {
                let e = HandlerError::IncorrectLect {
                    reason: "Found lect with wrong output_address".to_string(),
                    tx: tx.into(),
                };
                return Err(e.into());
            }
        }

        // Payload checks
        let (block_height, block_hash) = tx.payload();
        let schema = Schema::new(state.view());
        if Some(block_hash) != schema.heights().get(block_height)? {
            let e = HandlerError::IncorrectLect {
                reason: "Found lect with wrong payload".to_string(),
                tx: tx.into(),
            };
            return Err(e.into());
        }
        // Check that we did not miss more than one anchored height

        info!("CHECKED_LECT ====== txid={}", tx.txid());
        Ok(())
    }
}
