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

#![deny(missing_debug_implementations)]

#[macro_use]
extern crate derive_more;
#[macro_use]
extern crate display_derive;
#[macro_use]
extern crate exonum;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

#[cfg(test)]
#[macro_use]
extern crate proptest;

#[cfg(test)]
#[macro_use]
extern crate matches;

extern crate bitcoin;
extern crate btc_transaction_utils;
extern crate byteorder;
extern crate exonum_bitcoinrpc as bitcoin_rpc;
extern crate rand;
extern crate secp256k1;
extern crate serde;
extern crate serde_str;
extern crate toml;

extern crate exonum_testkit;

pub use factory::BtcAnchoringFactory as ServiceFactory;
pub use service::{BtcAnchoringService, BTC_ANCHORING_SERVICE_ID, BTC_ANCHORING_SERVICE_NAME};

pub mod blockchain;
pub mod btc;
pub mod config;
pub mod factory;
pub mod rpc;
pub mod service;

pub mod test_data;

mod handler;

pub(crate) trait ResultEx {
    fn log_error(self);
    fn log_warn(self);
}

impl<T: ::std::fmt::Display> ResultEx for Result<(), T> {
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
