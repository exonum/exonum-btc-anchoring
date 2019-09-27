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
use exonum::crypto::Hash;
use futures::{
    future::Future,
    stream::{futures_unordered, Stream},
};

use std::{collections::HashMap, sync::Arc};

use crate::{
    api::{AnchoringTransactionProposal, AsyncResult, IntoAsyncResult, PrivateApi},
    blockchain::SignInput,
    btc,
    config::Config,
};

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

/// Client implementation for the private API of the anchoring service instance.
#[derive(Debug, Clone)]
pub struct PrivateApiClient {
    /// Complete prefix with the port and the anchoring instance name.
    prefix: String,
    /// Underlying HTTP client.
    client: reqwest::r#async::Client,
}

impl PrivateApiClient {
    /// Create a new anchoring private API relay with the specified host and name of instance.
    /// Hostname should be in form `{http|https}://{address}:{port}`.
    pub fn new(hostname: impl AsRef<str>, instance_name: impl AsRef<str>) -> Self {
        Self {
            prefix: format!(
                "{}/api/services/{}",
                hostname.as_ref(),
                instance_name.as_ref()
            ),
            client: reqwest::r#async::Client::new(),
        }
    }

    fn endpoint(&self, name: impl AsRef<str>) -> String {
        format!("{}/{}", self.prefix, name.as_ref())
    }
}

impl PrivateApi for PrivateApiClient {
    type Error = failure::Error;

    fn sign_input(&self, sign_input: SignInput) -> AsyncResult<Hash, Self::Error> {
        Box::new(
            self.client
                .post(&self.endpoint("sign-input"))
                .json(&sign_input)
                .send()
                .and_then(|mut request| request.json())
                .map_err(From::from),
        )
    }

    fn anchoring_proposal(&self) -> AsyncResult<Option<AnchoringTransactionProposal>, Self::Error> {
        Box::new(
            self.client
                .get(&self.endpoint("anchoring-proposal"))
                .send()
                .and_then(|mut request| request.json())
                .map_err(From::from),
        )
    }

    fn config(&self) -> AsyncResult<Config, Self::Error> {
        Box::new(
            self.client
                .get(&self.endpoint("config"))
                .send()
                .and_then(|mut request| request.json())
                .map_err(From::from),
        )
    }
}

/// Signs the inputs of the anchoring transaction proposal by the corresponding
/// Bitcoin private keys.
#[derive(Debug, Clone)]
pub struct AnchoringChainUpdater<T>
where
    T: PrivateApi<Error = failure::Error> + Clone + 'static,
{
    key_pool: KeyPool,
    api_relay: T,
}

impl<T> AnchoringChainUpdater<T>
where
    T: PrivateApi<Error = failure::Error> + Clone + 'static,
{
    /// Create a new anchoring chain updater instance.
    pub fn new(
        keys: impl IntoIterator<Item = (btc::PublicKey, btc::PrivateKey)>,
        api_relay: T,
    ) -> Self {
        Self {
            key_pool: Arc::new(keys.into_iter().collect()),
            api_relay,
        }
    }

    /// Perform a one attempt to sign an anchoring proposal, if any.
    pub fn process(self) -> impl Future<Item = ()> {
        self.clone()
            .api_relay
            .anchoring_proposal()
            .and_then(move |proposal| {
                let proposal = some_or_return!(proposal);
                Box::new(
                    self.clone()
                        .api_relay
                        .config()
                        .and_then(move |config| self.clone().handle_proposal(config, proposal)),
                )
            })
    }

    fn handle_proposal(
        self,
        config: Config,
        proposal: AnchoringTransactionProposal,
    ) -> impl Future<Item = (), Error = failure::Error> {
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
                return Err(failure::format_err!(
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
            Err(e) => return Err(e).into_async(),
        };

        let api_relay = self.api_relay.clone();
        Box::new(
            futures_unordered(
                sign_input_messages
                    .into_iter()
                    .map(move |sign_input| api_relay.clone().sign_input(sign_input)),
            )
            .collect()
            .map(move |_| {
                log::info!(
                    "Successfully sent signatures for the proposal with id: {} for height: {}",
                    proposal.transaction.id(),
                    block_height,
                );
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
