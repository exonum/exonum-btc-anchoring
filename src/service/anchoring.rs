use exonum::blockchain::{Schema, NodeState};
use exonum::storage::List;
use exonum::crypto::ToHex;

use bitcoin::util::base58::ToBase58;

use btc;
use btc::HexValueEx;
use transactions::{AnchoringTx, TransactionBuilder};
use error::Error as ServiceError;
use service::{AnchoringHandler, AnchoringSchema, LectKind, MultisigAddress, collect_signatures};
use service::schema::{MsgAnchoringSignature, MsgAnchoringUpdateLatest, AnchoringMessage};
use service::config::AnchoringConfig;

impl AnchoringHandler {
    pub fn handle_anchoring_state(&mut self,
                                  cfg: AnchoringConfig,
                                  state: &mut NodeState)
                                  -> Result<(), ServiceError> {
        let multisig = self.multisig_address(&cfg);
        debug!("Anchoring state, addr={}", multisig.addr.to_base58check());

        if state.height() % self.node.check_lect_frequency == 0 {
            // First of all we try to update our lect and actual configuration
            self.update_our_lect(&multisig, state)?;
        }
        // Now if we have anchoring tx proposal we must try to finalize it
        if let Some(proposal) = self.proposal_tx.clone() {
            self.try_finalize_proposal_tx(proposal, &multisig, state)?;
        } else {
            // Or try to create proposal
            self.try_create_proposal_tx(&multisig, state)?;
        }
        Ok(())
    }


    pub fn try_create_proposal_tx(&mut self,
                                  multisig: &MultisigAddress,
                                  state: &mut NodeState)
                                  -> Result<(), ServiceError> {
        match self.collect_lects(state)? {
            LectKind::Funding(_) => self.try_create_anchoring_tx_chain(multisig, None, state),
            LectKind::Anchoring(tx) => {
                let anchored_height = tx.payload().0;
                let nearest_anchored_height = multisig.genesis
                    .nearest_anchoring_height(state.height());
                if nearest_anchored_height > anchored_height {
                    return self.create_proposal_tx(tx, multisig, nearest_anchored_height, state);
                }
                Ok(())
            }
            LectKind::None => {
                warn!("Unable to reach consensus in a lect");
                Ok(())
            }
        }
    }

    // Create first anchoring tx proposal from funding tx in AnchoringNodeConfig
    pub fn try_create_anchoring_tx_chain(&mut self,
                                         multisig: &MultisigAddress,
                                         prev_tx_chain: Option<btc::TxId>,
                                         state: &mut NodeState)
                                         -> Result<(), ServiceError> {
        debug!("Create first proposal tx");
        if let Some(funding_tx) = self.avaliable_funding_tx(multisig)? {
            // Create anchoring proposal
            let height = multisig.genesis.nearest_anchoring_height(state.height());
            let hash = Schema::new(state.view())
                .heights()
                .get(height)?
                .unwrap();

            let out = funding_tx.find_out(&multisig.addr).unwrap();
            let proposal = TransactionBuilder::with_prev_tx(&funding_tx, out).fee(multisig.genesis.fee)
                .payload(height, hash)
                .prev_tx_chain(prev_tx_chain)
                .send_to(multisig.addr.clone())
                .into_transaction()?;

            debug!("initial_proposal={:#?}, txhex={}",
                   proposal,
                   proposal.0.to_hex());

            // Sign proposal
            self.sign_proposal_tx(proposal, multisig, state)?;
        } else {
            warn!("Funding transaction is not suitable.");
        }
        Ok(())
    }

    pub fn create_proposal_tx(&mut self,
                              lect: AnchoringTx,
                              multisig: &MultisigAddress,
                              height: u64,
                              state: &mut NodeState)
                              -> Result<(), ServiceError> {
        let hash = Schema::new(state.view())
            .heights()
            .get(height)?
            .unwrap();

        let proposal = {
            let mut builder = TransactionBuilder::with_prev_tx(&lect, 0)
                .fee(multisig.genesis.fee)
                .payload(height, hash)
                .send_to(multisig.addr.clone());
            if let Some(funds) = self.avaliable_funding_tx(multisig)? {
                let out = funds.find_out(&multisig.addr).expect("Funding tx has multisig output");
                builder = builder.add_funds(&funds, out);
            }
            builder.into_transaction()?
        };

        debug!("proposal={:#?}, to={:?}, height={}, hash={}",
               proposal,
               multisig.addr,
               height,
               hash.to_hex());
        // Sign proposal
        self.sign_proposal_tx(proposal, multisig, state)
    }

    pub fn sign_proposal_tx(&mut self,
                            proposal: AnchoringTx,
                            multisig: &MultisigAddress,
                            state: &mut NodeState)
                            -> Result<(), ServiceError> {
        debug!("sign proposal tx");
        for input in proposal.inputs() {
            let signature = proposal.sign(&multisig.redeem_script, input, &multisig.priv_key);

            let sign_msg = MsgAnchoringSignature::new(state.public_key(),
                                                      state.id(),
                                                      proposal.clone(),
                                                      input,
                                                      &signature,
                                                      state.secret_key());

            debug!("Sign_msg={:#?}, sighex={}", sign_msg, signature.to_hex());
            state.add_transaction(AnchoringMessage::Signature(sign_msg));
        }
        self.proposal_tx = Some(proposal);
        Ok(())
    }

    pub fn try_finalize_proposal_tx(&mut self,
                                    proposal: AnchoringTx,
                                    multisig: &MultisigAddress,
                                    state: &mut NodeState)
                                    -> Result<(), ServiceError> {
        debug!("try finalize proposal tx");
        let txid = proposal.id();

        let proposal_height = proposal.payload().0;
        if multisig.genesis.nearest_anchoring_height(state.height()) !=
           multisig.genesis.nearest_anchoring_height(proposal_height) {
            warn!("Unable to finalize anchoring tx for height={}",
                  proposal_height);
            self.proposal_tx = None;
            return Ok(());
        }

        let msgs = AnchoringSchema::new(state.view()).signatures(&txid).values()?;
        debug!("msgs={:#?}", msgs);
        if let Some(signatures) = collect_signatures(&proposal, multisig.genesis, msgs.iter()) {
            let new_lect = proposal.finalize(&multisig.redeem_script, signatures);
            // Send transaction if it needs
            if self.client
                   .get_transaction_info(&new_lect.txid())?
                   .is_none() {
                self.client.send_transaction(new_lect.clone().into())?;
            }

            debug!("sended signed_tx={:#?}, to={}",
                   new_lect,
                   new_lect.output_address(multisig.genesis.network()).to_base58check());

            info!("ANCHORING ====== anchored_height={}, txid={}, remaining_funds={}",
                  new_lect.payload().0,
                  new_lect.txid().to_hex(),
                  new_lect.amount());

            debug!("LECT ====== txid={}, total_count={}",
                   new_lect.txid().to_hex(),
                   AnchoringSchema::new(state.view()).lects(state.id()).len()?);

            self.proposal_tx = None;

            let lects_count = AnchoringSchema::new(state.view()).lects(state.id()).len()?;
            let lect_msg = MsgAnchoringUpdateLatest::new(state.public_key(),
                                                         state.id(),
                                                         new_lect.into(),
                                                         lects_count,
                                                         state.secret_key());
            state.add_transaction(AnchoringMessage::UpdateLatest(lect_msg));
        } else {
            warn!("Insufficient signatures for proposal={:#?}", proposal);
        }
        Ok(())
    }
}
