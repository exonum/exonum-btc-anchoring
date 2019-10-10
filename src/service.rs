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
    crypto::Hash,
    helpers::ValidateInput,
    merkledb::{BinaryValue, Fork},
    runtime::{
        api::ServiceApiBuilder,
        rust::{
            interfaces::{verify_caller_is_supervisor, Configure},
            Service, TransactionContext,
        },
        DispatcherError, ExecutionError, InstanceDescriptor,
    },
};
use exonum_derive::ServiceFactory;
use exonum_merkledb::Snapshot;

use crate::{
    api,
    blockchain::{BtcAnchoringSchema, Transactions},
    config::Config,
    proto,
};

/// Bitcoin anchoring service implementation for the Exonum blockchain.
#[derive(ServiceFactory, Debug)]
#[exonum(
    proto_sources = "proto",
    implements("Transactions", "Configure<Params = Config>")
)]
pub struct BtcAnchoringService;

impl Service for BtcAnchoringService {
    fn initialize(
        &self,
        instance: InstanceDescriptor,
        fork: &Fork,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // TODO Use a special type for constructor. [ECR-3222]
        let config = Config::from_bytes(params.into())
            .and_then(ValidateInput::into_validated)
            .map_err(DispatcherError::malformed_arguments)?;

        BtcAnchoringSchema::new(instance.name, fork).set_actual_config(config);
        Ok(())
    }

    fn state_hash(&self, instance: InstanceDescriptor, snapshot: &dyn Snapshot) -> Vec<Hash> {
        BtcAnchoringSchema::new(instance.name, snapshot).state_hash()
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::wire(builder);
    }
}

impl Configure for BtcAnchoringService {
    type Params = Config;

    fn verify_config(
        &self,
        context: TransactionContext,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .verify_caller(verify_caller_is_supervisor)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        params
            .validate()
            .map_err(DispatcherError::malformed_arguments)
    }

    fn apply_config(
        &self,
        context: TransactionContext,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        let (_, fork) = context
            .verify_caller(verify_caller_is_supervisor)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        let schema = BtcAnchoringSchema::new(context.instance.name, fork);
        if schema.actual_config().anchoring_address() == params.anchoring_address() {
            // There are no changes in the anchoring address, so we just apply the config
            // immediately.
            schema.set_actual_config(params);
        } else {
            // Set the config as the next one, which will become an actual after the transition
            // of the anchoring chain to the following address.
            schema.following_config_entry().set(params);
        }
        Ok(())
    }
}
