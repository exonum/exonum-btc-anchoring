use bitcoin::util::base58::ToBase58;

use exonum::blockchain::NodeState;

use error::Error as ServiceError;
use details::btc::transactions::BitcoinTx;
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::schema::AnchoringSchema;

use super::{AnchoringHandler, MultisigAddress, LectKind};

#[doc(hidden)]
impl AnchoringHandler {
    pub fn handle_transition_state(&mut self,
                                   from: AnchoringConfig,
                                   to: AnchoringConfig,
                                   state: &mut NodeState)
                                   -> Result<(), ServiceError> {
        let multisig: MultisigAddress = {
            let mut multisig = self.multisig_address(&from);
            multisig.addr = to.redeem_script().1;
            multisig
        };
        trace!("Transition state, addr={}, following_config={:#?}",
               multisig.addr.to_base58check(),
               to);

        // Similar we update lect each n blocks
        if state.height() % self.node.check_lect_frequency == 0 {
            // First of all we try to update our lect and actual configuration
            self.update_our_lect(&multisig, state)?;
        }

        // Now if we have anchoring tx proposal we must try to finalize it
        if let Some(proposal) = self.proposal_tx.clone() {
            self.try_finalize_proposal_tx(proposal, &multisig, state)?;
        } else {
            // Or try to create proposal
            match self.collect_lects(state)? {
                LectKind::Anchoring(lect) => {
                    if lect.output_address(multisig.common.network) == multisig.addr {
                        return Ok(());
                    }
                    // check that we have enougth confirmations
                    let confirmations = lect.confirmations(&self.client)?.unwrap_or_else(|| 0);
                    if confirmations >= multisig.common.utxo_confirmations {
                        let height = multisig.common.nearest_anchoring_height(state.height());
                        self.create_proposal_tx(lect, &multisig, height, state)?;
                    } else {
                        warn!("Insufficient confirmations for create transition transaction, \
                               tx={:#?}, confirmations={}",
                              lect,
                              confirmations);
                    }
                }
                LectKind::Funding(_) => panic!("We must not to change genesis configuration!"),
                LectKind::None => {
                    warn!("Unable to reach consensus in a lect");
                }
            }
        }
        Ok(())
    }

    pub fn handle_waiting_state(&mut self,
                                lect: BitcoinTx,
                                confirmations: Option<u64>)
                                -> Result<(), ServiceError> {
        trace!("Waiting for enough confirmations for the lect={:#?}, current={:?}",
               lect,
               confirmations);
        if confirmations.is_none() {
            trace!("Resend transition transaction, txid={}", lect.txid());
            self.client.send_transaction(lect)?;
        }
        Ok(())
    }

    pub fn handle_recovering_state(&mut self,
                                   cfg: AnchoringConfig,
                                   state: &mut NodeState)
                                   -> Result<(), ServiceError> {
        let multisig: MultisigAddress = self.multisig_address(&cfg);

        trace!("Starting a new tx chain to addr={} from scratch",
               multisig.addr.to_base58check());

        let lect_id = AnchoringSchema::new(state.view())
            .lect(self.validator_id(state))?
            .id();
        self.try_create_anchoring_tx_chain(&multisig, Some(lect_id), state)?;

        // Try to finalize new tx chain propose if it exist
        if let Some(proposal) = self.proposal_tx.clone() {
            self.try_finalize_proposal_tx(proposal, &multisig, state)?;
        }
        Ok(())
    }
}
