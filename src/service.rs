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

use exonum::blockchain::{Service, ServiceContext, Transaction, TransactionSet};
use exonum::crypto::Hash;
use exonum::encoding::Error as EncodingError;
use exonum::messages::RawMessage;
use exonum::storage::{Fork, Snapshot};

use serde_json;

use std::collections::HashMap;

use ResultEx;
use blockchain::{BtcAnchoringSchema, Transactions};
use btc::{Address, Privkey};
use config::GlobalConfig;
use handler::{SyncWithBtcRelayTask, UpdateAnchoringChainTask};
use rpc::BtcRelay;

// TODO support recovery mode if after transition transaction with following output address doesn't exist.

/// Anchoring service id.
pub const BTC_ANCHORING_SERVICE_ID: u16 = 3;
/// Anchoring service name.
pub const BTC_ANCHORING_SERVICE_NAME: &str = "btc_anchoring";

pub struct BtcAnchoringService {
    global_config: GlobalConfig,
    private_keys: HashMap<Address, Privkey>,
    btc_relay: Option<Box<BtcRelay>>,
}

impl ::std::fmt::Debug for BtcAnchoringService {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.debug_struct("BtcAnchoringService").finish()
    }
}

impl BtcAnchoringService {
    pub fn new(
        global_config: GlobalConfig,
        private_keys: HashMap<Address, Privkey>,
        btc_relay: Option<Box<BtcRelay>>,
    ) -> BtcAnchoringService {
        BtcAnchoringService {
            global_config,
            private_keys,
            btc_relay,
        }
    }
}

impl Service for BtcAnchoringService {
    fn service_id(&self) -> u16 {
        BTC_ANCHORING_SERVICE_ID
    }

    fn service_name(&self) -> &'static str {
        BTC_ANCHORING_SERVICE_NAME
    }

    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
        BtcAnchoringSchema::new(snapshot).state_hash()
    }

    fn tx_from_raw(&self, raw: RawMessage) -> Result<Box<Transaction>, EncodingError> {
        let tx = Transactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    fn initialize(&self, _fork: &mut Fork) -> serde_json::Value {
        json!(self.global_config)
    }

    fn after_commit(&self, context: &ServiceContext) {
        let task = UpdateAnchoringChainTask::new(context, &self.private_keys);
        task.run().log_error();

        // TODO make this task async via tokio core or something else.
        if let Some(ref relay) = self.btc_relay.as_ref() {
            let task = SyncWithBtcRelayTask::new(context, relay.as_ref());
            task.run().log_error();
        }
    }
}
