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

pub use crate::proto::TxSignature;

use btc_transaction_utils::{p2wsh::InputSigner, InputSignature, TxInRef};
use exonum::runtime::{rust::TransactionContext, Caller, DispatcherError, ExecutionError};
use exonum_derive::exonum_service;
use log::{info, trace};

use crate::{btc, BtcAnchoringService};

use super::{data_layout::TxInputId, errors::SignatureError, BtcAnchoringSchema};

impl TxSignature {
    /// Returns identifier of the signed transaction input.
    pub fn input_id(&self) -> TxInputId {
        TxInputId {
            txid: self.transaction.id(),
            input: self.input,
        }
    }
}

/// Exonum BTC anchoring transactions.
#[exonum_service]
pub trait Transactions {
    /// Exonum message with the signature for the new anchoring transaction.
    fn sign_input(
        &self,
        context: TransactionContext,
        arg: TxSignature,
    ) -> Result<(), ExecutionError>;
}

impl Transactions for BtcAnchoringService {
    fn sign_input(
        &self,
        context: TransactionContext,
        arg: TxSignature,
    ) -> Result<(), ExecutionError> {
        let (author, fork) = context
            .verify_caller(Caller::author)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        let tx = &arg.transaction;
        let schema = BtcAnchoringSchema::new(context.instance.name, fork);
        // Checks that the number of signatures is sufficient to spend.
        if schema
            .anchoring_transactions_chain()
            .last()
            .map(|tx| tx.id())
            == Some(tx.id())
        {
            return Ok(());
        }

        let (expected_transaction, expected_inputs) = schema
            .actual_proposed_anchoring_transaction()
            .ok_or(SignatureError::InTransition)?
            .map_err(SignatureError::TxBuilderError)?;

        if expected_transaction.id() != tx.id() {
            return Err(SignatureError::Unexpected {
                expected_id: expected_transaction.id(),
                received_id: tx.id(),
            }
            .into());
        }

        let actual_state = schema.actual_state();
        let (anchoring_node_id, public_key) = actual_state
            .actual_configuration()
            .find_bitcoin_key(&author)
            .ok_or_else(|| SignatureError::MissingPublicKey {
                service_key: author,
            })?;
        let redeem_script = actual_state.actual_configuration().redeem_script();
        let redeem_script_content = redeem_script.content();

        let input_signer = InputSigner::new(redeem_script.clone());
        // Checks signature content.
        let input_signature_ref = arg.input_signature.as_ref();
        let input_idx = arg.input as usize;
        let input_tx = match expected_inputs.get(input_idx) {
            Some(input_tx) => input_tx,
            _ => return Err(SignatureError::NoSuchInput { idx: input_idx }.into()),
        };

        let verification_result = input_signer.verify_input(
            TxInRef::new(tx.as_ref(), arg.input as usize),
            input_tx.as_ref(),
            &public_key.0,
            input_signature_ref,
        );

        if verification_result.is_err() {
            return Err(SignatureError::VerificationFailed.into());
        }

        let input_id = arg.input_id();
        let mut input_signatures = schema.input_signatures(&input_id, &redeem_script);
        if input_signatures.len() != redeem_script_content.quorum {
            // Adds signature to schema.
            input_signatures.insert(anchoring_node_id, arg.input_signature.clone().into());
            schema
                .transaction_signatures()
                .put(&input_id, input_signatures);
            // Tries to finalize transaction.
            let mut tx: btc::Transaction = tx.clone();
            for index in 0..expected_inputs.len() {
                let input_id = TxInputId::new(arg.transaction.id(), index as u32);
                let input_signatures = schema.input_signatures(&input_id, &redeem_script);

                if input_signatures.len() != redeem_script_content.quorum {
                    return Ok(());
                }

                input_signer.spend_input(
                    &mut tx.0.input[index],
                    input_signatures
                        .into_iter()
                        .map(|bytes| InputSignature::from_bytes(bytes).unwrap()),
                );
            }

            let payload = tx.anchoring_metadata().unwrap().1;

            info!("====== ANCHORING ======");
            info!("txid: {}", tx.id().to_hex());
            info!("height: {}", payload.block_height);
            info!("hash: {}", payload.block_hash.to_hex());
            info!("balance: {}", tx.0.output[0].value);
            trace!("Anchoring txhex: {}", tx.to_string());

            // Adds finalized transaction to the tail of anchoring transactions.
            schema.anchoring_transactions_chain().push(tx);
            if let Some(unspent_funding_tx) = schema.unspent_funding_transaction() {
                schema
                    .spent_funding_transactions()
                    .put(&unspent_funding_tx.id(), unspent_funding_tx);
            }
        }
        Ok(())
    }
}
