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
    helpers::ValidateInput,
    merkledb::BinaryValue,
    runtime::{CommonError, ExecutionContext, ExecutionError},
};
use exonum_derive::{ServiceDispatcher, ServiceFactory};
use exonum_rust_runtime::{api::ServiceApiBuilder, Service};
use exonum_supervisor::Configure;

use crate::{
    api,
    blockchain::{BtcAnchoringInterface, Schema},
    config::Config,
    proto,
};

/// Bitcoin anchoring service implementation for the Exonum blockchain.
#[derive(ServiceFactory, ServiceDispatcher, Debug, Clone, Copy)]
#[service_dispatcher(implements("BtcAnchoringInterface", raw = "Configure<Params = Config>"))]
#[service_factory(proto_sources = "proto")]
pub struct BtcAnchoringService;

impl Service for BtcAnchoringService {
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // TODO Use a special type for constructor. [ECR-3222]
        let config = Config::from_bytes(params.into())
            .and_then(ValidateInput::into_validated)
            .map_err(CommonError::malformed_arguments)?;

        Schema::new(context.service_data())
            .actual_config
            .set(config);
        Ok(())
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::wire(builder);
    }
}

impl Configure for BtcAnchoringService {
    type Params = Config;

    fn verify_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .caller()
            .as_supervisor()
            .ok_or(CommonError::UnauthorizedCaller)?;

        params.validate().map_err(CommonError::malformed_arguments)
    }

    fn apply_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        context
            .caller()
            .as_supervisor()
            .ok_or(CommonError::UnauthorizedCaller)?;

        let mut schema = Schema::new(context.service_data());
        if schema.actual_config().anchoring_address() == params.anchoring_address() {
            // There are no changes in the anchoring address, so we just apply the config
            // immediately.
            schema.actual_config.set(params);
        } else {
            // Set the config as the next one, which will become an actual after the transition
            // of the anchoring chain to the following address.
            schema.following_config.set(params);
        }
        Ok(())
    }
}
