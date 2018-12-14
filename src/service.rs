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

use exonum::api::ServiceApiBuilder;
use exonum::blockchain::{
    Schema as CoreSchema, Service, ServiceContext, Transaction, TransactionSet,
};
use exonum::crypto::Hash;
use exonum::messages::RawTransaction;
use exonum::storage::{Fork, Snapshot};

use failure;
use serde_json;

use std::sync::{Arc, RwLock};

use std::collections::HashMap;

use api;
use blockchain::{BtcAnchoringSchema, Transactions};
use btc::{Address, Privkey};
use config::GlobalConfig;
use handler::{SyncWithBtcRelayTask, UpdateAnchoringChainTask};
use rpc::BtcRelay;
use ResultEx;

/// Anchoring service id.
pub const BTC_ANCHORING_SERVICE_ID: u16 = 3;
/// Anchoring service name.
pub const BTC_ANCHORING_SERVICE_NAME: &str = "btc_anchoring";
/// Set of bitcoin private keys for corresponding anchoring addresses.
pub(crate) type KeyPool = Arc<RwLock<HashMap<Address, Privkey>>>;

/// Btc anchoring service implementation for the Exonum blockchain.
pub struct BtcAnchoringService {
    global_config: GlobalConfig,
    private_keys: KeyPool,
    btc_relay: Option<Box<dyn BtcRelay>>,
}

impl ::std::fmt::Debug for BtcAnchoringService {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.debug_struct("BtcAnchoringService").finish()
    }
}

impl BtcAnchoringService {
    /// Creates a new btc anchoring service instance.
    pub fn new(
        global_config: GlobalConfig,
        private_keys: KeyPool,
        btc_relay: Option<Box<dyn BtcRelay>>,
    ) -> Self {
        Self {
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

    fn state_hash(&self, snapshot: &dyn Snapshot) -> Vec<Hash> {
        BtcAnchoringSchema::new(snapshot).state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<dyn Transaction>, failure::Error> {
        let tx = Transactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    fn initialize(&self, _fork: &mut Fork) -> serde_json::Value {
        json!(self.global_config)
    }

    fn before_commit(&self, fork: &mut Fork) {
        // Writes a hash of the latest block to the proof list index.
        let block_header_hash = CoreSchema::new(&fork)
            .block_hashes_by_height()
            .last()
            .expect("An attempt to invoke execute during the genesis block initialization.");

        let mut schema = BtcAnchoringSchema::new(fork);
        schema.anchored_blocks_mut().push(block_header_hash);
    }

    fn after_commit(&self, context: &ServiceContext) {
        let keys = &self.private_keys.read().unwrap();
        let task = UpdateAnchoringChainTask::new(context, keys);
        task.run().log_error();
        // TODO make this task async via tokio core or something else.
        if let Some(ref relay) = self.btc_relay.as_ref() {
            let task = SyncWithBtcRelayTask::new(context, relay.as_ref());
            task.run().log_error();
        }
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::wire(builder);
    }
}
