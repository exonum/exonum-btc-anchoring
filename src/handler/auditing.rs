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

use exonum::blockchain::ServiceContext;

use error::Error as ServiceError;
use details::btc::transactions::{AnchoringTx, FundingTx};
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::schema::AnchoringSchema;

use super::{AnchoringHandler, LectKind};
use super::error::Error as HandlerError;

#[doc(hidden)]
impl AnchoringHandler {
    pub fn handle_auditing_state(
        &mut self,
        cfg: &AnchoringConfig,
        state: &ServiceContext,
    ) -> Result<(), ServiceError> {
        trace!("Auditing state");
        if state.height().0 % self.node.check_lect_frequency == 0 {
            let r = match self.collect_lects(state)? {
                LectKind::Funding(tx) => self.check_funding_lect(tx, state),
                LectKind::Anchoring(tx) => self.check_anchoring_lect(&tx),
                LectKind::None => {
                    let e = HandlerError::LectNotFound {
                        height: cfg.latest_anchoring_height(state.height()),
                    };
                    Err(e.into())
                }
            };
            return r;
        }
        Ok(())
    }

    fn check_funding_lect(
        &self,
        tx: FundingTx,
        context: &ServiceContext,
    ) -> Result<(), ServiceError> {
        let cfg = AnchoringSchema::new(context.snapshot()).genesis_anchoring_config();
        let (_, addr) = cfg.redeem_script();
        if &tx != cfg.funding_tx() {
            let e = HandlerError::IncorrectLect {
                reason: "Initial funding_tx from cfg is different than in lect".to_string(),
                tx: tx.into(),
            };
            return Err(e.into());
        }
        if tx.find_out(&addr).is_none() {
            let e = HandlerError::IncorrectLect {
                reason: format!(
                    "Initial funding_tx has no outputs with address={}",
                    addr.to_string()
                ),
                tx: tx.into(),
            };
            return Err(e.into());
        }

        // Checks with access to the `bitcoind`
        if let Some(ref client) = self.client {
            if client.get_transaction(tx.id())?.is_none() {
                let e = HandlerError::IncorrectLect {
                    reason: "Initial funding_tx not found in the bitcoin blockchain".to_string(),
                    tx: tx.into(),
                };
                return Err(e.into());
            }
        }

        info!("CHECKED_INITIAL_LECT ====== txid={}", tx.id());
        Ok(())
    }

    fn check_anchoring_lect(&self, tx: &AnchoringTx) -> Result<(), ServiceError> {
        // Checks with access to the `bitcoind`
        if let Some(ref client) = self.client {
            if client.get_transaction(tx.id())?.is_none() {
                let e = HandlerError::LectNotFound {
                    height: tx.payload().block_height,
                };
                return Err(e.into());
            }
        }

        info!("CHECKED_LECT ====== txid={}", tx.id());
        Ok(())
    }
}
