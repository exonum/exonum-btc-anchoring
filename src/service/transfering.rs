use exonum::blockchain::NodeState;

use bitcoin::util::base58::ToBase58;

use config::{AnchoringConfig};

use super::{AnchoringHandler, MultisigAddress, LectKind};
use super::error::Error as ServiceError;

impl AnchoringHandler {
    pub fn handle_transfering_state(&mut self,
                                    from: AnchoringConfig,
                                    to: AnchoringConfig,
                                    state: &mut NodeState)
                                    -> Result<(), ServiceError> {
        debug!("Transfering state");
        let multisig: MultisigAddress = {
            let mut multisig = self.multisig_address(&from);
            multisig.addr = to.redeem_script().1;
            multisig
        };

        // Точно так же обновляем lect каждые n блоков
        if state.height() % self.node.check_lect_frequency == 0 {
            // First of all we try to update our lect and actual configuration
            self.update_our_lect(&multisig, state)?;
        }

        // Now if we have anchoring tx proposal we must try to finalize it
        if let Some(proposal) = self.proposal_tx.clone() {
            self.try_finalize_proposal_tx(proposal, &multisig, state)?;
        } else {
            // Or try to create proposal
            match self.collect_lects(state).unwrap() {
                LectKind::Anchoring(lect) => {
                    debug!("lect={:#?}", lect);
                    // в этом случае ничего делать не нужно
                    if lect.output_address(multisig.genesis.network()) == multisig.addr {
                        return Ok(());
                    }

                    debug!("lect_addr={}",
                           lect.output_address(multisig.genesis.network()).to_base58check());
                    debug!("following_addr={:?}", multisig.addr);
                    // проверяем, что нам хватает подтверждений
                    let info = lect.get_info(&self.client)?.unwrap();
                    debug!("info={:?}", info);
                    if info.confirmations.unwrap() as u64 >= multisig.genesis.utxo_confirmations {
                        // FIXME зафиксировать высоту для анкоринга
                        let height = multisig.genesis.nearest_anchoring_height(state.height());
                        self.create_proposal_tx(lect, &multisig, height, state)?;
                    } else {
                        warn!("Insufficient confirmations for create transfer transaction")
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
                                cfg: AnchoringConfig,
                                state: &mut NodeState)
                                -> Result<(), ServiceError> {
        debug!("waiting after transfer state");
        let multisig: MultisigAddress = self.multisig_address(&cfg);
        // Точно так же обновляем lect каждые n блоков
        if state.height() % self.node.check_lect_frequency == 0 {
            // First of all we try to update our lect and actual configuration
            self.update_our_lect(&multisig, state)?;
        }
        Ok(())
    }
}