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

use exonum::{blockchain::Schema, helpers::Height, storage::Fork};

use blockchain::consensus_storage::AnchoringConfig;
use blockchain::schema::AnchoringSchema;
use details::btc::transactions::{AnchoringTx, BitcoinTx, TxKind};
use details::rpc::BitcoinRelay;
use error::Error as ServiceError;

/// Anchoring observer configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnchoringObserverConfig {
    /// An interval of anchoring chain checks (in blocks).
    pub check_interval: Height,
    /// This option determines whether to enable observer or not.
    pub enabled: bool,
}

impl Default for AnchoringObserverConfig {
    fn default() -> AnchoringObserverConfig {
        AnchoringObserverConfig {
            check_interval: Height(1_000),
            enabled: false,
        }
    }
}

/// Anchoring chain observer. Periodically checks the state of the anchor chain and keeps
/// the verified transactions in database.
#[derive(Debug)]
pub struct AnchoringChainObserver<'a, 'b> {
    fork: &'a mut Fork,
    client: &'b dyn BitcoinRelay,
}

impl<'a, 'b> AnchoringChainObserver<'a, 'b> {
    pub fn new(fork: &'a mut Fork, client: &'b dyn BitcoinRelay) -> Self {
        AnchoringChainObserver { fork, client }
    }

    /// Tries to get `lect` for the current anchoring configuration and retrospectively adds
    /// all previously unknown anchoring transactions.
    pub fn check_anchoring_chain(self) -> Result<(), ServiceError> {
        if !self.is_blockchain_inited() {
            return Ok(());
        }

        let cfg = AnchoringSchema::new(&self.fork).actual_anchoring_config();
        if let Some(lect) = self.find_lect(&cfg)? {
            if !self.lect_payload_is_correct(&lect) {
                error!("Received lect with incorrect payload, content={:#?}", lect);
                return Ok(());
            }
            self.update_anchoring_chain(&cfg, lect)?;
        }
        Ok(())
    }

    fn update_anchoring_chain(
        self,
        actual_cfg: &AnchoringConfig,
        mut lect: AnchoringTx,
    ) -> Result<(), ServiceError> {
        let mut anchoring_schema = AnchoringSchema::new(self.fork);

        loop {
            let payload = lect.payload();
            let height = payload.block_height.into();

            // We already committed given lect to chain and there is no need to continue
            // checking chain.
            if let Some(other_lect) = anchoring_schema.anchoring_tx_chain().get(&height) {
                if other_lect == lect {
                    return Ok(());
                }
            }

            let confirmations = self.client.get_transaction_confirmations(lect.id())?;
            if confirmations.as_ref() >= Some(&actual_cfg.utxo_confirmations) {
                trace!(
                    "Adds transaction to chain, height={}, content={:#?}",
                    payload.block_height,
                    lect
                );

                anchoring_schema
                    .anchoring_tx_chain_mut()
                    .put(&height, lect.clone());
            }

            let prev_txid = payload.prev_tx_chain.unwrap_or_else(|| lect.prev_hash());
            if let Some(prev_tx) = self.client.get_transaction(prev_txid)? {
                lect = match TxKind::from(prev_tx) {
                    TxKind::Anchoring(lect) => lect,
                    TxKind::FundingTx(_) => return Ok(()),
                    TxKind::Other(tx) => {
                        panic!("Found incorrect lect transaction, content={:#?}", tx)
                    }
                }
            } else {
                return Ok(());
            }
        }
    }

    fn find_lect(&self, actual_cfg: &AnchoringConfig) -> Result<Option<AnchoringTx>, ServiceError> {
        let actual_addr = actual_cfg.redeem_script().1;

        trace!("Tries to find lect for the addr: {}", actual_addr);

        let unspent_txs: Vec<_> = self.client.unspent_transactions(&actual_addr)?;
        for tx in unspent_txs {
            if self.transaction_is_lect(actual_cfg, &tx.body)? {
                if let TxKind::Anchoring(lect) = TxKind::from(tx.body) {
                    return Ok(Some(lect));
                }
            }
        }
        Ok(None)
    }

    fn transaction_is_lect(
        &self,
        actual_cfg: &AnchoringConfig,
        tx: &BitcoinTx,
    ) -> Result<bool, ServiceError> {
        let txid = tx.id();
        let anchoring_schema = AnchoringSchema::new(&self.fork);

        let mut lect_count = 0;
        for key in &actual_cfg.anchoring_keys {
            if anchoring_schema.find_lect_position(key, &txid).is_some() {
                lect_count += 1;
            }
        }
        Ok(lect_count >= actual_cfg.majority_count())
    }

    fn lect_payload_is_correct(&self, lect: &AnchoringTx) -> bool {
        let schema = Schema::new(&self.fork);
        let payload = lect.payload();
        let block_hash = schema.block_hash_by_height(payload.block_height);
        block_hash == Some(payload.block_hash)
    }

    fn is_blockchain_inited(&self) -> bool {
        let schema = Schema::new(&self.fork);
        let len = schema.block_hashes_by_height().len();
        len > 0
    }
}
