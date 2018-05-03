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

use exonum::blockchain::{Schema, ServiceContext};
use exonum::helpers::Height;
use exonum::encoding::serialize::encode_hex;

use error::Error as ServiceError;
use details::btc;
use details::btc::transactions::{AnchoringTx, RawBitcoinTx, TransactionBuilder};
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::schema::AnchoringSchema;
use blockchain::dto::{MsgAnchoringSignature, MsgAnchoringUpdateLatest};

use super::{collect_signatures, AnchoringHandler, LectKind, MultisigAddress};

#[doc(hidden)]
impl AnchoringHandler {
    pub fn handle_anchoring_state(
        &mut self,
        cfg: &AnchoringConfig,
        context: &ServiceContext,
    ) -> Result<(), ServiceError> {
        let multisig = self.multisig_address(cfg);
        trace!("Anchoring state, addr={}", multisig.addr.to_string());

        if context.height().0 % self.node.check_lect_frequency == 0 {
            // First of all we try to update our lect and actual configuration
            self.update_our_lect(&multisig, context)?;
        }
        // Now if we have anchoring tx proposal we must try to finalize it
        if let Some(proposal) = self.proposal_tx.clone() {
            self.try_finalize_proposal_tx(proposal, &multisig, context)?;
        } else {
            // Or try to create proposal
            self.try_create_proposal_tx(&multisig, context)?;
        }
        Ok(())
    }

    pub fn try_create_proposal_tx(
        &mut self,
        multisig: &MultisigAddress,
        context: &ServiceContext,
    ) -> Result<(), ServiceError> {
        let lect = self.collect_lects_for_validator(
            self.anchoring_key(multisig.common, context),
            multisig.common,
            context,
        );
        match lect {
            LectKind::Funding(_) => self.try_create_anchoring_tx_chain(multisig, None, context),
            LectKind::Anchoring(tx) => {
                let anchored_height = tx.payload().block_height;
                let latest_anchored_height =
                    multisig.common.latest_anchoring_height(context.height());
                if latest_anchored_height > anchored_height {
                    return self.create_proposal_tx(&tx, multisig, latest_anchored_height, context);
                }
                Ok(())
            }
            LectKind::None => {
                warn!("Unable to reach consensus in the lect");
                Ok(())
            }
        }
    }

    // Create first anchoring tx proposal from funding tx in AnchoringNodeConfig
    pub fn try_create_anchoring_tx_chain(
        &mut self,
        multisig: &MultisigAddress,
        prev_tx_chain: Option<btc::TxId>,
        context: &ServiceContext,
    ) -> Result<(), ServiceError> {
        trace!("Create tx chain");
        if let Some(funding_tx) = self.available_funding_tx(multisig)? {
            // Create anchoring proposal
            let height = multisig.common.latest_anchoring_height(context.height());
            let hash = Schema::new(context.snapshot())
                .block_hashes_by_height()
                .get(height.0)
                .unwrap();

            let out = funding_tx.find_out(&multisig.addr).unwrap();
            let proposal = TransactionBuilder::with_prev_tx(&funding_tx, out)
                .fee(multisig.common.fee)
                .payload(height, hash)
                .prev_tx_chain(prev_tx_chain)
                .send_to(multisig.addr.clone())
                .into_transaction()?;

            trace!("initial_proposal={:?}", proposal,);

            // Sign proposal
            self.sign_proposal_tx(proposal, &[funding_tx.0], multisig, context)?;
        } else {
            warn!("Funding transaction is not suitable.");
        }
        Ok(())
    }

    pub fn create_proposal_tx(
        &mut self,
        lect: &AnchoringTx,
        multisig: &MultisigAddress,
        height: Height,
        context: &ServiceContext,
    ) -> Result<(), ServiceError> {
        let hash = Schema::new(context.snapshot())
            .block_hashes_by_height()
            .get(height.0)
            .unwrap();

        let (proposal, prev_txs) = {
            let mut prev_txs = vec![lect.0.clone()];

            let mut builder = TransactionBuilder::with_prev_tx(lect, 0)
                .fee(multisig.common.fee)
                .payload(height, hash)
                .send_to(multisig.addr.clone());

            if let Some(funds) = self.available_funding_tx(multisig)? {
                let out = funds.find_out(&multisig.addr).expect(
                    "Funding tx has proper \
                     multisig output",
                );
                builder = builder.add_funds(&funds, out);
                prev_txs.push(funds.0);
            }
            (builder.into_transaction()?, prev_txs)
        };

        trace!(
            "proposal={:?}, to={:?}, height={}, hash={}",
            proposal,
            multisig.addr,
            height,
            hash.to_hex()
        );
        self.sign_proposal_tx(proposal, &prev_txs, multisig, context)
    }

    pub fn sign_proposal_tx(
        &mut self,
        proposal: AnchoringTx,
        prev_txs: &[RawBitcoinTx],
        multisig: &MultisigAddress,
        context: &ServiceContext,
    ) -> Result<(), ServiceError> {
        for input in proposal.inputs() {
            let prev_tx = &prev_txs[input as usize];
            let signature =
                proposal.sign_input(&multisig.redeem_script, input, prev_tx, &multisig.priv_key);

            debug_assert_eq!(proposal.input[input as usize].prev_hash, prev_tx.txid());

            debug_assert!(proposal.verify_input(
                &multisig.redeem_script,
                input,
                prev_tx,
                self.anchoring_key(multisig.common, context),
                &signature
            ));

            let sign_msg = MsgAnchoringSignature::new(
                context.public_key(),
                self.validator_id(context),
                proposal.clone(),
                input,
                &signature,
                context.secret_key(),
            );

            trace!(
                "Sign input msg={:?}, sighex={}",
                sign_msg,
                encode_hex(signature)
            );
            context.transaction_sender().send(Box::new(sign_msg))?;
        }
        self.proposal_tx = Some(proposal);
        Ok(())
    }

    pub fn try_finalize_proposal_tx(
        &mut self,
        proposal: AnchoringTx,
        multisig: &MultisigAddress,
        context: &ServiceContext,
    ) -> Result<(), ServiceError> {
        trace!("Try finalize proposal tx");
        let txid = proposal.id();

        let proposal_height = proposal.payload().block_height;
        if multisig.common.latest_anchoring_height(context.height())
            != multisig.common.latest_anchoring_height(proposal_height)
        {
            warn!(
                "Unable to finalize anchoring tx for height={}",
                proposal_height
            );
            self.proposal_tx = None;
            return Ok(());
        }

        let collected_signatures = {
            let anchoring_schema = AnchoringSchema::new(context.snapshot());
            let signatures = anchoring_schema.signatures(&txid);
            collect_signatures(&proposal, multisig.common, &signatures)
        };
        if let Some(signatures) = collected_signatures {
            let new_lect = proposal.finalize(&multisig.redeem_script, signatures);
            // Send transaction if it needs
            if self.client().get_transaction(new_lect.id())?.is_none() {
                self.client().send_transaction(new_lect.clone().into())?;
                trace!("Sent signed_tx={:#?}, to={}", new_lect, multisig.addr,);
            }

            info!(
                "ANCHORING ====== anchored_height={}, txid={}, remaining_funds={}",
                new_lect.payload().block_height,
                new_lect.id(),
                new_lect.amount()
            );

            info!(
                "LECT ====== txid={}, total_count={}",
                new_lect.id(),
                AnchoringSchema::new(context.snapshot())
                    .lects(self.anchoring_key(multisig.common, context))
                    .len()
            );

            self.proposal_tx = None;

            let lects_count = AnchoringSchema::new(context.snapshot())
                .lects(self.anchoring_key(multisig.common, context))
                .len();
            let lect_msg = MsgAnchoringUpdateLatest::new(
                context.public_key(),
                self.validator_id(context),
                new_lect.into(),
                lects_count,
                context.secret_key(),
            );
            context.transaction_sender().send(Box::new(lect_msg))?;
        } else {
            warn!("Insufficient signatures for proposal={:#?}", proposal);
        }
        Ok(())
    }
}
