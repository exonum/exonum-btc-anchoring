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

//! # Introduction
//!
//! Private blockchain infrastructure necessitates additional measures for
//! accountability of the blockchain validators.
//! In public proof of work blockchains (e.g., Bitcoin), accountability is purely economic and is
//! based on game theory and equivocation or retroactive modifications being economically costly.
//! Not so in private blockchains, where these two behaviors
//! are a real threat per any realistic threat model that assumes
//! that the blockchain is of use not only to the system validators,
//! but also to third parties.
//!
//! This crate implements a protocol for blockchain anchoring onto the Bitcoin blockchain
//! that utilizes the native Bitcoin capabilities of creating multisig([p2sh][1]) transactions.
//! This transactions contains metadata from Exonum blockchain (block's hash on corresponding
//! height) and forms a chain.
//!
//! You can read the details in [specification][2].
//!
//! [1]: https://bitcoin.org/en/glossary/p2sh-multisig
//! [2]: https://github.com/exonum/exonum-doc/blob/master/src/advanced/bitcoin-anchoring.md
//!
//! # Examples
//!
//! Create application with anchoring service
//!
//! ```rust,no_run
//! use exonum::helpers::fabric::NodeBuilder;
//!
//! fn main() {
//!     exonum::crypto::init();
//!     exonum::helpers::init_logger().unwrap();
//!
//!     let node = NodeBuilder::new()
//!         .with_service(Box::new(exonum_btc_anchoring::ServiceFactory));
//!     node.run();
//! }
//! ```
//!

#![warn(
    missing_docs,
    missing_debug_implementations,
    unsafe_code,
    bare_trait_objects
)]

pub use crate::factory::BtcAnchoringFactory as ServiceFactory;
pub use crate::service::{
    BtcAnchoringService, BTC_ANCHORING_SERVICE_ID, BTC_ANCHORING_SERVICE_NAME,
};

pub mod api;
pub mod blockchain;
pub mod btc;
pub mod config;
pub mod rpc;
pub mod test_helpers;

pub(crate) mod factory;
pub(crate) mod service;

use log::{error, warn};

mod handler;
mod proto;

pub(crate) trait ResultEx {
    fn log_error(self);
    fn log_warn(self);
}

impl<T: std::fmt::Display> ResultEx for Result<(), T> {
    fn log_error(self) {
        if let Err(e) = self {
            error!("{}", e);
        }
    }

    fn log_warn(self) {
        if let Err(e) = self {
            warn!("{}", e);
        }
    }
}
