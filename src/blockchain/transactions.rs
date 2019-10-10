// Copyright 2019 The Exonum Team
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

//! BTC anchoring transactions.

pub use crate::proto::SignInput;

use btc_transaction_utils::{p2wsh::InputSigner, TxInRef};
use exonum::runtime::{rust::TransactionContext, Caller, DispatcherError, ExecutionError};
use exonum_derive::exonum_service;
use log::{info, trace};

use crate::{btc, BtcAnchoringService};

use super::{data_layout::TxInputId, errors::Error, BtcAnchoringSchema};

impl SignInput {
    // Check that input signature is correct.
    fn verify_signature(
        &self,
        input_signer: &InputSigner,
        public_key: &btc::PublicKey,
        proposal: &btc::Transaction,
        inputs: &[btc::Transaction],
    ) -> Result<(), ExecutionError> {
        // Check that input with the specified index exist.
        let input_transaction = inputs.get(self.input as usize).ok_or(Error::NoSuchInput)?;
        input_signer
            .verify_input(
                TxInRef::new(proposal.as_ref(), self.input as usize),
                input_transaction.as_ref(),
                &public_key.0,
                self.input_signature.as_ref(),
            )
            .map_err(|e| (Error::InputVerificationFailed, e).into())
    }
}

/// Exonum BTC anchoring transactions.
#[exonum_service]
pub trait Transactions {
    /// Signs a single input of the anchoring transaction proposal.
    fn sign_input(&self, context: TransactionContext, arg: SignInput)
        -> Result<(), ExecutionError>;
}

impl Transactions for BtcAnchoringService {
    fn sign_input(
        &self,
        context: TransactionContext,
        arg: SignInput,
    ) -> Result<(), ExecutionError> {
        let (author, fork) = context
            .verify_caller(Caller::author)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        let schema = BtcAnchoringSchema::new(context.instance.name, fork);
        // Check that author is authorized to sign inputs of the anchoring proposal.
        let actual_config = schema.actual_state().actual_config().clone();
        let (anchoring_node_id, public_key) = actual_config
            .find_bitcoin_key(&author)
            .ok_or(Error::UnauthorizedAnchoringKey)?;

        // Check that there is an anchoring proposal for the actual blockchain state.
        let (proposal, expected_inputs) = if let Some(proposal) = schema
            .actual_proposed_anchoring_transaction()
            .transpose()
            .map_err(Error::anchoring_builder_error)?
        {
            proposal
        } else {
            // There is no anchoring request at the current blockchain state.
            // Make sure txid is equal to the identifier of the last anchoring transaction.
            let latest_anchoring_txid = schema
                .anchoring_transactions_chain()
                .last()
                // If the anchoring chain is not established, then the proposal must exist.
                .unwrap()
                .id();
            if latest_anchoring_txid == arg.txid {
                return Ok(());
            } else {
                return Err(Error::UnexpectedProposalTxId.into());
            }
        };

        // Make sure txid is equal to the identifier of the anchoring transaction proposal.
        if proposal.id() != arg.txid {
            return Err(Error::UnexpectedProposalTxId.into());
        }

        // Check that input signature is correct.
        let redeem_script = actual_config.redeem_script();
        let quorum = redeem_script.content().quorum;
        let input_signer = InputSigner::new(redeem_script);
        arg.verify_signature(&input_signer, &public_key, &proposal, &expected_inputs)?;

        // All preconditions are correct and we can use this signature.
        let input_id = TxInputId::new(proposal.id(), arg.input);
        let mut input_signatures = schema.input_signatures(&input_id);
        let mut input_signature_len = input_signatures.0.len();
        // Check that we have not reached the quorum yet, otherwise we should not do anything.
        if input_signature_len < quorum {
            // Add signature to schema.
            input_signatures
                .0
                .insert(anchoring_node_id as u16, arg.input_signature.clone());
            schema
                .transaction_signatures()
                .put(&input_id, input_signatures);
            input_signature_len += 1;
        } else {
            return Ok(());
        }

        // If we have enough signatures for specific input we have to check that we also have
        // sufficient signatures to finalize proposal transaction.
        if input_signature_len == quorum {
            let mut finalized_tx: btc::Transaction = proposal.clone();
            // Make sure we reach a quorum for each input.
            for index in 0..expected_inputs.len() {
                let input_id = TxInputId::new(proposal.id(), index as u32);
                let signatures_for_input = schema.input_signatures(&input_id);
                // We have not enough signatures for this input, so we can not finalize this
                // proposal at the moment.
                if signatures_for_input.0.len() != quorum {
                    return Ok(());
                }

                input_signer.spend_input(
                    &mut finalized_tx.0.input[index],
                    signatures_for_input.0.values().map(|x| x.0.clone()),
                );
            }

            let payload = finalized_tx.anchoring_metadata().unwrap().1;

            info!("====== ANCHORING ======");
            info!("txid: {}", finalized_tx.id().to_string());
            info!("height: {}", payload.block_height);
            info!("hash: {}", payload.block_hash.to_hex());
            info!("balance: {}", finalized_tx.0.output[0].value);
            trace!("Anchoring txhex: {}", finalized_tx.to_string());

            // Add finalized transaction to the tail of anchoring transactions.
            schema.push_anchoring_transaction(finalized_tx);
        }
        Ok(())
    }
}
