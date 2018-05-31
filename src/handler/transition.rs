// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use exonum::blockchain::ServiceContext;

use blockchain::consensus_storage::AnchoringConfig;
use blockchain::schema::AnchoringSchema;
use details::btc::transactions::BitcoinTx;
use error::Error as ServiceError;

use super::{AnchoringHandler, LectKind, MultisigAddress};

#[doc(hidden)]
impl AnchoringHandler {
    pub fn handle_transition_state(
        &mut self,
        from: &AnchoringConfig,
        to: &AnchoringConfig,
        state: &ServiceContext,
    ) -> Result<(), ServiceError> {
        let multisig: MultisigAddress = {
            let mut multisig = self.multisig_address(from);
            multisig.addr = to.redeem_script().1;
            multisig
        };
        trace!(
            "Transition state, addr={}, following_config={:#?}",
            multisig.addr,
            to
        );

        // Similar we update lect each n blocks
        if state.height().0 % self.node.check_lect_frequency == 0 {
            // First of all we try to update our lect and actual configuration
            self.update_our_lect(&multisig, state)?;
        }

        // Now if we have anchoring tx proposal we must try to finalize it
        if let Some(proposal) = self.proposal_tx.clone() {
            self.try_finalize_proposal_tx(proposal, &multisig, state)?;
        } else {
            // Or try to create proposal
            match self.collect_lects_for_validator(
                self.anchoring_key(multisig.common, state),
                multisig.common,
                state,
            ) {
                LectKind::Anchoring(lect) => {
                    if lect.script_pubkey() == &multisig.addr.script_pubkey() {
                        return Ok(());
                    }
                    // check that we have enough confirmations
                    let confirmations = self.client()
                        .get_transaction_confirmations(lect.id())?
                        .unwrap_or_else(|| 0);
                    if confirmations >= multisig.common.utxo_confirmations {
                        let height = multisig.common.latest_anchoring_height(state.height());
                        self.create_proposal_tx(&lect, &multisig, height, state)?;
                    } else {
                        warn!(
                            "Insufficient confirmations for create transition transaction, \
                             tx={:#?}, confirmations={}",
                            lect, confirmations
                        );
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

    pub fn handle_waiting_state(
        &mut self,
        lect: BitcoinTx,
        confirmations: Option<u64>,
    ) -> Result<(), ServiceError> {
        trace!(
            "Waiting for enough confirmations for the lect={:#?}, current={:?}",
            lect,
            confirmations
        );
        if confirmations.is_none() {
            trace!("Resend transition transaction, txid={}", lect.id());
            self.client().send_transaction(lect)?;
        }
        Ok(())
    }

    pub fn handle_recovering_state(
        &mut self,
        prev_cfg: &AnchoringConfig,
        actual_cfg: &AnchoringConfig,
        state: &ServiceContext,
    ) -> Result<(), ServiceError> {
        let multisig: MultisigAddress = self.multisig_address(actual_cfg);

        if state.height().0 % self.node.check_lect_frequency == 0 {
            // First of all we try to update our lect and actual configuration
            self.update_our_lect(&multisig, state)?;
        }

        trace!(
            "Starting a new tx chain to addr={} from scratch",
            multisig.addr
        );

        let lect_txid = {
            let anchoring_schema = AnchoringSchema::new(state.snapshot());
            if let Some(tx) = anchoring_schema.collect_lects(prev_cfg) {
                tx.id()
            } else {
                // Use initial funding tx as prev chain
                let genesis_cfg = anchoring_schema.genesis_anchoring_config();
                genesis_cfg.funding_tx().id()
            }
        };
        self.try_create_anchoring_tx_chain(&multisig, Some(lect_txid), state)?;

        // Try to finalize new tx chain propose if it exist
        if let Some(proposal) = self.proposal_tx.clone() {
            self.try_finalize_proposal_tx(proposal, &multisig, state)?;
        }
        Ok(())
    }
}
