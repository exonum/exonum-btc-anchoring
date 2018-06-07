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

use exonum::blockchain::ServiceContext;
use exonum::helpers::ValidatorId;

use btc_transaction_utils::TxInRef;
use btc_transaction_utils::p2wsh;
use failure;

use std::cmp;
use std::collections::HashMap;

use blockchain::data_layout::TxInputId;
use blockchain::transactions::Signature;
use blockchain::{BtcAnchoringSchema, BtcAnchoringState};
use btc::{Address, Privkey};
use rpc::BtcRelay;

pub struct UpdateAnchoringChainTask<'a> {
    context: &'a ServiceContext,
    anchoring_state: BtcAnchoringState,
    private_keys: &'a HashMap<Address, Privkey>,
}

impl<'a> UpdateAnchoringChainTask<'a> {
    pub fn new(
        context: &'a ServiceContext,
        private_keys: &'a HashMap<Address, Privkey>,
    ) -> UpdateAnchoringChainTask<'a> {
        UpdateAnchoringChainTask {
            context,
            anchoring_state: BtcAnchoringSchema::new(context.snapshot()).actual_state(),
            private_keys,
        }
    }

    pub fn run(self) -> Result<(), failure::Error> {
        if let Some(validator_id) = self.context.validator_id() {
            let address = self.anchoring_state.output_address();
            let privkey = self.private_keys
                .get(&address)
                .ok_or_else(|| format_err!("Private key for the address {} is absent.", address))?;
            self.handle_as_validator(validator_id, &privkey)
        } else {
            self.handle_as_auditor()
        }
    }

    fn handle_as_validator(
        self,
        validator_id: ValidatorId,
        privkey: &Privkey,
    ) -> Result<(), failure::Error> {
        let anchoring_schema = BtcAnchoringSchema::new(self.context.snapshot());
        let latest_anchored_height = anchoring_schema.latest_anchored_height();
        let anchoring_height = self.anchoring_state
            .following_anchoring_height(latest_anchored_height);

        if self.context.height() < anchoring_height {
            return Ok(());
        }

        // Creates anchoring proposal
        let (proposal, proposal_inputs) = if let Some(proposal) =
            anchoring_schema.proposed_anchoring_transaction(&self.anchoring_state)
        {
            proposal?
        } else {
            return Ok(());
        };

        let config = self.anchoring_state.actual_configuration();
        let redeem_script = config.redeem_script();
        // Creates Signature transactions.
        let pubkey = redeem_script.content().public_keys[validator_id.0 as usize];
        let mut signer = p2wsh::InputSigner::new(redeem_script);

        for (index, proposal_input) in proposal_inputs.iter().enumerate() {
            let input_id = TxInputId::new(proposal.id(), index as u32);
            if anchoring_schema
                .transaction_signatures()
                .contains(&input_id)
            {
                continue;
            }

            let signature = signer.sign_input(
                TxInRef::new(proposal.as_ref(), index),
                proposal_inputs[index].as_ref(),
                privkey.0.secret_key(),
            )?;

            signer
                .verify_input(
                    TxInRef::new(proposal.as_ref(), index),
                    proposal_input.as_ref(),
                    &pubkey,
                    &signature,
                )
                .unwrap();

            let signature_tx = Signature::new(
                self.context.public_key(),
                validator_id,
                proposal.clone(),
                index as u32,
                signature.as_ref(),
                self.context.secret_key(),
            );

            self.context
                .transaction_sender()
                .send(Box::new(signature_tx))?;
        }

        Ok(())
    }

    fn handle_as_auditor(self) -> Result<(), failure::Error> {
        unimplemented!();
    }
}

#[derive(Debug)]
pub struct SyncWithBtcRelayTask<'a> {
    context: &'a ServiceContext,
    relay: &'a BtcRelay,
}

impl<'a> SyncWithBtcRelayTask<'a> {
    pub fn new(context: &'a ServiceContext, relay: &'a BtcRelay) -> SyncWithBtcRelayTask<'a> {
        SyncWithBtcRelayTask { context, relay }
    }

    pub fn run(self) -> Result<(), failure::Error> {
        let anchoring_schema = BtcAnchoringSchema::new(self.context.snapshot());
        let sync_interval = cmp::max(
            1,
            anchoring_schema.actual_configuration().anchoring_interval / 2,
        );

        if self.context.height().0 % sync_interval == 0 {
            if let Some(index) = self.find_index_of_first_uncommitted_transaction()? {
                let anchoring_txs = anchoring_schema.anchoring_transactions_chain();
                for tx in anchoring_txs.iter_from(index) {
                    trace!("Send anchoring transaction to btc relay: {}", tx.id());
                    self.relay.send_transaction(&tx)?;
                }
            }
        }

        Ok(())
    }

    fn find_index_of_first_uncommitted_transaction(&self) -> Result<Option<u64>, failure::Error> {
        let anchoring_schema = BtcAnchoringSchema::new(self.context.snapshot());
        let anchoring_txs = anchoring_schema.anchoring_transactions_chain();

        let anchoring_txs_len = anchoring_txs.len();
        let tx_indices = (0..anchoring_txs_len).rev();
        for index in tx_indices {
            let tx = anchoring_txs.get(index).unwrap();
            let info = self.relay.transaction_info(&tx.prev_tx_id())?;
            if info.is_some() {
                let info = self.relay.transaction_info(&tx.id())?;
                return Ok(info.map(|_| index));
            }
        }

        Ok(None)
    }
}
