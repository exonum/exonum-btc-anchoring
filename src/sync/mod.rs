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

//! Building blocks of the anchoring sync utility.

use btc_transaction_utils::{p2wsh, TxInRef};
use futures::{
    future::Future,
    stream::{futures_unordered, Stream},
};

use std::{collections::HashMap, sync::Arc};

use crate::{
    api::{AnchoringTransactionProposal, IntoAsyncResult, PrivateApi, PublicApi},
    blockchain::SignInput,
    btc,
    config::Config,
};

use self::bitcoin_relay::BtcRelay;

pub mod bitcoin_relay;

type KeyPool = Arc<HashMap<btc::PublicKey, btc::PrivateKey>>;

macro_rules! some_or_return {
    ($value_expr:expr) => {
        if let Some(value) = $value_expr {
            value
        } else {
            return Ok(()).into_async();
        }
    };
}

/// Signs the inputs of the anchoring transaction proposal by the corresponding
/// Bitcoin private keys.
#[derive(Debug, Clone)]
pub struct AnchoringChainUpdater<T>
where
    T: PrivateApi<Error = String> + Send + Clone + 'static,
{
    key_pool: KeyPool,
    api_client: T,
}

impl<T> AnchoringChainUpdater<T>
where
    T: PrivateApi<Error = String> + Send + Clone + 'static,
{
    /// Create a new anchoring chain updater instance.
    pub fn new(
        keys: impl IntoIterator<Item = (btc::PublicKey, btc::PrivateKey)>,
        api_client: T,
    ) -> Self {
        Self {
            key_pool: Arc::new(keys.into_iter().collect()),
            api_client,
        }
    }

    /// Perform a one attempt to sign an anchoring proposal, if any.
    pub fn process(self) -> impl Future<Item = (), Error = String> {
        log::trace!("Perform an anchoring chain update");
        self.clone()
            .api_client
            .anchoring_proposal()
            .and_then(move |proposal| {
                let proposal = some_or_return!(proposal);
                Box::new(
                    self.clone()
                        .api_client
                        .config()
                        .and_then(move |config| self.clone().handle_proposal(config, proposal)),
                )
            })
    }

    fn handle_proposal(
        self,
        config: Config,
        proposal: AnchoringTransactionProposal,
    ) -> impl Future<Item = (), Error = String> {
        log::trace!("Got an anchoring proposal: {:?}", proposal);
        // Find among the keys one from which we have a private part.
        // TODO What we have to do if we find more than one key? [ECR-3222]
        let keypair = some_or_return!(
            self.find_private_key(config.anchoring_keys.iter().map(|x| x.bitcoin_key))
        );
        // Create `SignInput` transactions
        let redeem_script = config.redeem_script();
        let block_height = match proposal.transaction.anchoring_payload() {
            Some(payload) => payload.block_height,
            None => {
                return Err(format!(
                    "Incorrect anchoring proposal found: {:?}",
                    proposal.transaction
                ))
                .into_async()
            }
        };

        log::info!(
            "Found a new unfinished anchoring transaction proposal for height: {}",
            block_height
        );

        let mut signer = p2wsh::InputSigner::new(redeem_script);
        let sign_input_messages = proposal
            .inputs
            .iter()
            .enumerate()
            .map(|(index, proposal_input)| {
                let signature = signer.sign_input(
                    TxInRef::new(proposal.transaction.as_ref(), index),
                    proposal.inputs[index].as_ref(),
                    &(keypair.1).0.key,
                )?;

                signer.verify_input(
                    TxInRef::new(proposal.transaction.as_ref(), index),
                    proposal_input.as_ref(),
                    &(keypair.0).0,
                    &signature,
                )?;

                Ok(SignInput {
                    transaction: proposal.transaction.clone(),
                    input: index as u32,
                    input_signature: signature.into(),
                })
            })
            .collect::<Result<Vec<_>, failure::Error>>();

        let sign_input_messages = match sign_input_messages {
            Ok(messages) => messages,
            Err(e) => return Err(e.to_string()).into_async(),
        };

        // Send sign input transactions to the Exonum node.
        let api_client = self.api_client.clone();
        Box::new(
            futures_unordered(
                sign_input_messages
                    .into_iter()
                    .map(move |sign_input| api_client.clone().sign_input(sign_input)),
            )
            .collect()
            .map(move |_| {
                log::info!(
                    "Successfully sent signatures for the proposal with id: {} for height: {}.",
                    proposal.transaction.id().to_hex(),
                    block_height,
                );
                log::info!("Balance: {}", proposal.transaction.0.output[0].value)
            }),
        )
    }

    fn find_private_key(
        &self,
        anchoring_keys: impl IntoIterator<Item = btc::PublicKey>,
    ) -> Option<(btc::PublicKey, btc::PrivateKey)> {
        anchoring_keys.into_iter().find_map(|public_key| {
            self.key_pool
                .get(&public_key)
                .cloned()
                .map(|private_key| (public_key, private_key))
        })
    }
}

/// Pushes anchoring transactions to the Bitcoin blockchain.
#[derive(Debug, Clone)]
pub struct SyncWithBitcoinTask<T, R>
where
    T: PublicApi<Error = String> + Send + Clone + 'static,
    R: BtcRelay,
{
    btc_relay: R,
    api_client: T,
}

impl<T, R> SyncWithBitcoinTask<T, R>
where
    T: PublicApi<Error = String> + Send + Clone + 'static,
    R: BtcRelay,
{
    /// Create a new sync with Bitcoin task instance.
    pub fn new(api_client: T, btc_relay: R) -> Self {
        Self {
            api_client,
            btc_relay,
        }
    }
}
