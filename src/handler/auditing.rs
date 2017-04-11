use exonum::blockchain::{NodeState, Schema};
use exonum::storage::List;

use error::Error as ServiceError;
use details::btc::transactions::AnchoringTx;
use blockchain::consensus_storage::AnchoringConfig;

use super::{AnchoringHandler, LectKind};
use super::error::Error as HandlerError;

#[doc(hidden)]
impl AnchoringHandler {
    pub fn handle_auditing_state(&mut self,
                                 cfg: AnchoringConfig,
                                 state: &NodeState)
                                 -> Result<(), ServiceError> {
        if state.height() % self.node.check_lect_frequency == 0 {
            return match self.collect_lects(state)? {
                       LectKind::Funding(_) => Ok(()),
                       LectKind::None => Err(HandlerError::LectNotFound)?,
                       LectKind::Anchoring(tx) => self.check_anchoring_lect(tx, cfg, state),
                   };
        }
        Ok(())
    }

    fn check_anchoring_lect(&self,
                            tx: AnchoringTx,
                            cfg: AnchoringConfig,
                            state: &NodeState)
                            -> Result<(), ServiceError> {
        let (_, addr) = cfg.redeem_script();
        // Check that tx address is correct
        if tx.output_address(cfg.network) != addr {
            // TODO check following cfg
        }

        // Payload checks
        let (block_height, block_hash) = tx.payload();
        let schema = Schema::new(state.view());
        if Some(block_hash) != schema.heights().get(block_height)? {
            Err(HandlerError::IncorrectLect {
                    reason: "Found lect with wrong payload".to_string(),
                    tx: tx.into(),
                })?;
        }

        // Check that we did not miss more than one anchored height

        Ok(())
    }
}
