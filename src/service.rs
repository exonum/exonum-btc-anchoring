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
    merkledb::{BinaryValue, Fork},
    runtime::{
        api::ServiceApiBuilder,
        rust::{BeforeCommitContext, Configure, Service},
        DispatcherError, ExecutionError, InstanceDescriptor,
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
    btc::{PrivateKey, PublicKey},
    config::GlobalConfig,
    proto,
};

/// Set of bitcoin private keys for corresponding public keys.
pub(crate) type KeyPool = Arc<RwLock<HashMap<PublicKey, PrivateKey>>>;

/// Btc anchoring service implementation for the Exonum blockchain.
#[derive(ServiceFactory)]
#[exonum(
    proto_sources = "proto",
    implements("Transactions", "Configure<Params = GlobalConfig>")
)]
pub struct BtcAnchoringService;

impl std::fmt::Debug for BtcAnchoringService {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.debug_struct("BtcAnchoringService").finish()
    }
}

impl Service for BtcAnchoringService {
    fn initialize(
        &self,
        instance: InstanceDescriptor,
        fork: &Fork,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        let config = GlobalConfig::from_bytes(params.into())
            .map_err(DispatcherError::malformed_arguments)?;

        BtcAnchoringSchema::new(instance.name, fork)
            .actual_config_entry()
            .set(config);
        Ok(())
    }

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

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::wire(builder);
    }
}
