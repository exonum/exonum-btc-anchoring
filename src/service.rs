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

use exonum::{
    blockchain::Schema as CoreSchema,
    crypto::Hash,
    runtime::{
        api::ServiceApiBuilder,
        rust::{AfterCommitContext, BeforeCommitContext, Service},
        InstanceDescriptor,
    },
};
use exonum_derive::ServiceFactory;
use exonum_merkledb::Snapshot;

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    api,
    blockchain::{BtcAnchoringSchema, Transactions},
    btc::{Address, PrivateKey},
    handler::{SyncWithBtcRelayTask, UpdateAnchoringChainTask},
    proto,
    rpc::BtcRelay,
    ResultEx,
};

/// Set of bitcoin private keys for corresponding anchoring addresses.
pub(crate) type KeyPool = Arc<RwLock<HashMap<Address, PrivateKey>>>;

/// Btc anchoring service implementation for the Exonum blockchain.
#[derive(ServiceFactory)]
#[exonum(
    proto_sources = "proto",
    service_constructor = "Self::create_instance",
    implements("Transactions")
)]
pub struct BtcAnchoringService {
    private_keys: KeyPool,
    btc_relay: Option<Arc<dyn BtcRelay>>,
}

impl std::fmt::Debug for BtcAnchoringService {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.debug_struct("BtcAnchoringService").finish()
    }
}

impl BtcAnchoringService {
    /// Creates a new btc anchoring service instance.
    pub fn new(private_keys: KeyPool, btc_relay: Option<Arc<dyn BtcRelay>>) -> Self {
        Self {
            private_keys,
            btc_relay,
        }
    }

    fn create_instance(&self) -> Box<dyn Service> {
        let instance = Self {
            private_keys: self.private_keys.clone(),
            btc_relay: self.btc_relay.clone(),
        };
        Box::new(instance)
    }
}

impl Service for BtcAnchoringService {
    fn state_hash(&self, instance: InstanceDescriptor, snapshot: &dyn Snapshot) -> Vec<Hash> {
        BtcAnchoringSchema::new(instance.name, snapshot).state_hash()
    }

    fn before_commit(&self, context: BeforeCommitContext) {
        // Writes a hash of the latest block to the proof list index.
        let block_header_hash = CoreSchema::new(context.fork)
            .block_hashes_by_height()
            .last()
            .expect("An attempt to invoke execute during the genesis block initialization.");

        let schema = BtcAnchoringSchema::new(context.instance.name, context.fork);
        schema.anchored_blocks().push(block_header_hash);
    }

    fn after_commit(&self, context: AfterCommitContext) {
        let keys = &self.private_keys.read().unwrap();
        let task = UpdateAnchoringChainTask::new(&context, keys);
        task.run().log_error();
        // TODO make this task async via tokio core or something else.
        if let Some(ref relay) = self.btc_relay.as_ref() {
            let task = SyncWithBtcRelayTask::new(&context, relay.as_ref());
            task.run().log_error();
        }
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::wire(builder);
    }
}
