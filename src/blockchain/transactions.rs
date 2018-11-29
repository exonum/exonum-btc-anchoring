// Copyright 2018 The Exonum Team
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

use exonum::{
    blockchain::{ExecutionResult, Transaction, TransactionContext},
    helpers::ValidatorId,
};

use btc_transaction_utils::{p2wsh::InputSigner, InputSignature, TxInRef};
use secp256k1::Secp256k1;

use super::data_layout::TxInputId;
use super::errors::SignatureError;
use super::BtcAnchoringSchema;
use btc;
use proto;

/// Exonum message with the signature for the new anchoring transaction.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "proto::TxSignature")]
pub struct TxSignature {
    /// Public key index in the anchoring public keys list.
    pub validator: ValidatorId,
    /// Signed Bitcoin anchoring transaction.
    pub transaction: btc::Transaction,
    /// Signed input.
    pub input: u32,
    /// Signature content.
    pub input_signature: btc::InputSignature,
}

/// Exonum BTC anchoring transactions.
#[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
pub enum Transactions {
    /// Exonum message with the signature for the new anchoring transaction.
    Signature(TxSignature),
}

impl TxSignature {
    /// Returns identifier of the signed transaction input.
    pub fn input_id(&self) -> TxInputId {
        TxInputId {
            txid: self.transaction.id(),
            input: self.input,
        }
    }
}

impl Transaction for TxSignature {
    fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
        // TODO Checks that transaction author is validator
        let tx = &self.transaction;
        let mut schema = BtcAnchoringSchema::new(context.fork());
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
            }.into());
        }

        let redeem_script = schema.actual_state().actual_configuration().redeem_script();
        let redeem_script_content = redeem_script.content();
        let public_key = match redeem_script_content
            .public_keys
            .get(self.validator.0 as usize)
        {
            Some(pk) => pk,
            _ => {
                return Err(SignatureError::MissingPublicKey {
                    validator_id: self.validator,
                }.into());
            }
        };

        let input_signer = InputSigner::new(redeem_script.clone());
        let context = Secp256k1::without_caps();

        // Checks signature content.
        let input_signature_ref = self.input_signature.as_ref();
        let input_idx = self.input as usize;
        let input_tx = match expected_inputs.get(input_idx) {
            Some(input_tx) => input_tx,
            _ => return Err(SignatureError::NoSuchInput { idx: input_idx }.into()),
        };

        let verification_result = input_signer.verify_input(
            TxInRef::new(tx.as_ref(), self.input as usize),
            input_tx.as_ref(),
            &public_key,
            input_signature_ref,
        );

        if verification_result.is_err() {
            return Err(SignatureError::VerificationFailed.into());
        }

        // Adds signature to schema.
        let input_id = self.input_id();
        let mut input_signatures = schema.input_signatures(&input_id, &redeem_script);
        if input_signatures.len() != redeem_script_content.quorum {
            input_signatures.insert(self.validator, self.input_signature.clone().into());
            schema
                .transaction_signatures_mut()
                .put(&input_id, input_signatures);
        }
        // Tries to finalize transaction.
        let mut tx: btc::Transaction = tx.clone();
        for index in 0..expected_inputs.len() {
            let input_id = TxInputId::new(self.transaction.id(), index as u32);
            let input_signatures = schema.input_signatures(&input_id, &redeem_script);

            if input_signatures.len() != redeem_script_content.quorum {
                return Ok(());
            }

            input_signer.spend_input(
                &mut tx.0.input[index],
                input_signatures
                    .into_iter()
                    .map(|bytes| InputSignature::from_bytes(&context, bytes).unwrap()),
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
        schema.anchoring_transactions_chain_mut().push(tx);
        if let Some(unspent_funding_tx) = schema.unspent_funding_transaction() {
            schema
                .spent_funding_transactions_mut()
                .put(&unspent_funding_tx.id(), unspent_funding_tx);
        }

        Ok(())
    }
}
