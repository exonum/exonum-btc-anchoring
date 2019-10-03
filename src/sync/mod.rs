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

// TODO Rewrite with the async/await syntax when it is ready. [ECR-3222]

pub use self::bitcoin_relay::BitcoinRelay;

use btc_transaction_utils::{p2wsh, TxInRef};
use exonum::crypto::Hash;
use futures::future::Future;

use std::{collections::HashMap, fmt::Display, sync::Arc};

use crate::{
    api::{AnchoringProposalState, PrivateApi},
    blockchain::SignInput,
    btc,
    config::Config,
};

mod bitcoin_relay;

macro_rules! some_or_return {
    ($value_expr:expr) => {
        if let Some(value) = $value_expr {
            value
        } else {
            return Ok(());
        }
    };
}

/// Anchoring transaction with its index in the anchoring chain.
pub type TransactionWithIndex = (btc::Transaction, u64);

type KeyPool = Arc<HashMap<btc::PublicKey, btc::PrivateKey>>;

/// Errors that occur when updating the anchoring chain.
#[derive(Debug)]
pub enum AnchoringChainUpdateError<C: Display> {
    /// Error occurred in the private API client.
    Client(C),
    /// Insufficient funds to create an anchoring transaction proposal.
    InsufficientFunds {
        /// Total transaction fee.
        total_fee: u64,
        /// Available balance.
        balance: u64,
    },
    /// Internal error.
    Internal(failure::Error),
}

/// Signs the inputs of the anchoring transaction proposal by the corresponding
/// Bitcoin private keys.
#[derive(Debug)]
pub struct AnchoringChainUpdateTask<T>
where
    T: PrivateApi + 'static,
{
    key_pool: KeyPool,
    api_client: T,
}

impl<T> AnchoringChainUpdateTask<T>
where
    T: PrivateApi + 'static,
    T::Error: Display,
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
    pub fn process(&self) -> Result<(), AnchoringChainUpdateError<T::Error>> {
        log::trace!("Perform an anchoring chain update");

        match self
            .api_client
            .anchoring_proposal()
            .map_err(AnchoringChainUpdateError::Client)?
        {
            AnchoringProposalState::None => Ok(()),
            AnchoringProposalState::Available {
                transaction,
                inputs,
            } => {
                let config = self
                    .api_client
                    .config()
                    .map_err(AnchoringChainUpdateError::Client)?;
                self.handle_proposal(config, transaction, inputs)
            }
            AnchoringProposalState::InsufficientFunds { balance, total_fee } => {
                Err(AnchoringChainUpdateError::InsufficientFunds { balance, total_fee })
            }
        }
    }

    fn handle_proposal(
        &self,
        config: Config,
        proposal: btc::Transaction,
        inputs: Vec<btc::Transaction>,
    ) -> Result<(), AnchoringChainUpdateError<T::Error>> {
        log::trace!("Got an anchoring proposal: {:?}", proposal);
        // Find among the keys one from which we have a private part.
        // TODO What we have to do if we find more than one key? [ECR-3222]
        let keypair = some_or_return!(
            self.find_private_key(config.anchoring_keys.iter().map(|x| x.bitcoin_key))
        );
        // Create `SignInput` transactions
        let redeem_script = config.redeem_script();
        let block_height = match proposal.anchoring_payload() {
            Some(payload) => payload.block_height,
            None => {
                return Err(AnchoringChainUpdateError::Internal(failure::format_err!(
                    "Incorrect anchoring proposal found: {:?}",
                    proposal
                )))
            }
        };

        log::info!(
            "Found a new unfinished anchoring transaction proposal for height: {}",
            block_height
        );

        let mut signer = p2wsh::InputSigner::new(redeem_script);
        let sign_input_messages = inputs
            .iter()
            .enumerate()
            .map(|(index, proposal_input)| {
                let signature = signer.sign_input(
                    TxInRef::new(proposal.as_ref(), index),
                    inputs[index].as_ref(),
                    &(keypair.1).0.key,
                )?;

                signer.verify_input(
                    TxInRef::new(proposal.as_ref(), index),
                    proposal_input.as_ref(),
                    &(keypair.0).0,
                    &signature,
                )?;

                Ok(SignInput {
                    transaction: proposal.clone(),
                    input: index as u32,
                    input_signature: signature.into(),
                })
            })
            .collect::<Result<Vec<_>, failure::Error>>()
            .map_err(AnchoringChainUpdateError::Internal)?;
        // Send sign input transactions to the Exonum node.
        for sign_input in sign_input_messages {
            self.api_client
                .sign_input(sign_input)
                .wait()
                .map_err(AnchoringChainUpdateError::Client)?;
        }
        Ok(())
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

/// Errors that occur when updating the sync with Bitcoin task.
#[derive(Debug)]
pub enum SyncWithBitcoinError<C: Display, R: Display> {
    /// Error occurred in the private API client.
    Client(C),
    /// Error occurred in the Bitcoin relay.
    Relay(R),
    /// Internal error.
    Internal(failure::Error),
    /// Initial funding transaction is unconfirmed.
    UnconfirmedFundingTransaction(Hash),
}

/// Pushes anchoring transactions to the Bitcoin blockchain.
#[derive(Debug)]
pub struct SyncWithBitcoinTask<T, R>
where
    T: PrivateApi + 'static,
    R: BitcoinRelay + 'static,
{
    btc_relay: R,
    api_client: T,
}

impl<T, R> SyncWithBitcoinTask<T, R>
where
    T: PrivateApi + 'static,
    R: BitcoinRelay + 'static,
    T::Error: Display,
    R::Error: Display,
{
    /// Create a new sync with Bitcoin task instance.
    pub fn new(btc_relay: R, api_client: T) -> Self {
        Self {
            api_client,
            btc_relay,
        }
    }

    /// Perform a one attempt to send the first uncommitted anchoring transaction into the Bitcoin network, if any.
    /// sign an anchoring proposal, if any. Return an index of the first committed transaction.
    pub fn process(
        &self,
        latest_committed_tx_index: Option<u64>,
    ) -> Result<Option<u64>, SyncWithBitcoinError<T::Error, R::Error>> {
        log::trace!("Perform syncing with the Bitcoin network");

        let (index, tx) = {
            if let Some(index) = latest_committed_tx_index {
                // Check that the latest committed transaction was really committed into
                // the Bitcoin network.
                let tx = self.get_transaction(index)?;
                if self.transaction_is_committed(tx.id())? {
                    let chain_len = self
                        .api_client
                        .transactions_count()
                        .map_err(SyncWithBitcoinError::Client)?
                        .value;

                    if index + 1 == chain_len {
                        return Ok(Some(index));
                    }
                    let index = index + 1;
                    (index, self.get_transaction(index)?)
                } else {
                    (index, tx)
                }
            }
            // Perform to find the actual uncommitted transaction.
            else if let Some((tx, index)) = self.find_index_of_first_uncommitted_transaction()? {
                (index, tx)
            } else {
                return Ok(None);
            }
        };
        // Send an actual uncommitted transaction into the Bitcoin network.
        self.btc_relay
            .send_transaction(&tx)
            .map_err(SyncWithBitcoinError::Relay)?;

        log::info!(
            "Sent transaction to the Bitcoin network: {}",
            tx.id().to_hex()
        );

        Ok(Some(index))
    }

    /// Find the first anchoring transaction and its index, which was not committed into
    /// the Bitcoin blockchain.
    pub fn find_index_of_first_uncommitted_transaction(
        &self,
    ) -> Result<Option<TransactionWithIndex>, SyncWithBitcoinError<T::Error, R::Error>> {
        let index = {
            let count = self
                .api_client
                .transactions_count()
                .map_err(SyncWithBitcoinError::Client)?
                .value;

            if count == 0 {
                return Ok(None);
            }
            count - 1
        };
        // Check that the tail of anchoring chain is committed to the Bitcoin.
        let transaction = self.get_transaction(index)?;
        if self.transaction_is_committed(transaction.id())? {
            return Ok(None);
        }
        // Try to find the first of uncommitted transactions.
        for index in (0..=index).rev() {
            let transaction = self.get_transaction(index)?;
            log::trace!(
                "Checking for transaction with index {} and id {}",
                index,
                transaction.id().to_hex()
            );
            if self.transaction_is_committed(transaction.prev_tx_id())? {
                log::trace!("Found committed transaction");
                return Ok(Some((transaction, index)));
            } else if index == 0 {
                return Err(SyncWithBitcoinError::UnconfirmedFundingTransaction(
                    transaction.prev_tx_id(),
                ));
            }
        }
        Ok(None)
    }

    fn get_transaction(
        &self,
        index: u64,
    ) -> Result<btc::Transaction, SyncWithBitcoinError<T::Error, R::Error>> {
        self.api_client
            .transaction_with_index(index)
            .map_err(SyncWithBitcoinError::Client)?
            .ok_or_else(|| {
                SyncWithBitcoinError::Internal(failure::format_err!(
                    "Transaction with index {} is absent in the anchoring chain",
                    index
                ))
            })
    }

    fn transaction_is_committed(
        &self,
        txid: Hash,
    ) -> Result<bool, SyncWithBitcoinError<T::Error, R::Error>> {
        let info = self
            .btc_relay
            .transaction_confirmations(txid)
            .map_err(SyncWithBitcoinError::Relay)?;
        Ok(info.is_some())
    }
}
