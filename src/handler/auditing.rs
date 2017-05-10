use bitcoin::util::base58::ToBase58;

use exonum::blockchain::NodeState;

use error::Error as ServiceError;
use details::btc::transactions::{AnchoringTx, FundingTx};
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
            let r = match self.collect_lects(&cfg, state)? {
                LectKind::Funding(tx) => self.check_funding_lect(tx, state),
                LectKind::Anchoring(tx) => self.check_anchoring_lect(tx),
                LectKind::None => {
                    let e = HandlerError::LectNotFound {
                        height: cfg.latest_anchoring_height(state.height()),
                    };
                    Err(e.into())
                }
            };
            return r;
        }
        Ok(())
    }

    fn check_funding_lect(&self, tx: FundingTx, context: &NodeState) -> Result<(), ServiceError> {
        let cfg = AnchoringSchema::new(context.view())
            .anchoring_config_by_height(0)?;
        let (_, addr) = cfg.redeem_script();
        if tx != cfg.funding_tx {
            let e = HandlerError::IncorrectLect {
                reason: "Initial funding_tx from cfg is different than in lect".to_string(),
                tx: tx.into(),
            };
            return Err(e.into());
        }
        if tx.find_out(&addr).is_none() {
            let e = HandlerError::IncorrectLect {
                reason: format!("Initial funding_tx has no outputs with address={}",
                                addr.to_base58check()),
                tx: tx.into(),
            };
            return Err(e.into());
        }

        // Checks with access to the `bitcoind`
        if let Some(ref client) = self.client {
            if client.get_transaction(&tx.txid())?.is_none() {
                let e = HandlerError::IncorrectLect {
                    reason: "Initial funding_tx not found in the bitcoin blockchain".to_string(),
                    tx: tx.into(),
                };
                return Err(e.into());
            }
        }

        info!("CHECKED_INITIAL_LECT ====== txid={}", tx.txid());
        Ok(())
    }

    fn check_anchoring_lect(&self, tx: AnchoringTx) -> Result<(), ServiceError> {
        // Checks with access to the `bitcoind`
        if let Some(ref client) = self.client {
            if client.get_transaction(&tx.txid())?.is_none() {
                let e = HandlerError::IncorrectLect {
                    reason: "Lect not found in the bitcoin blockchain".to_string(),
                    tx: tx.into(),
                };
                return Err(e.into());
            }
        }

        info!("CHECKED_LECT ====== txid={}", tx.txid());
        Ok(())
    }
}
