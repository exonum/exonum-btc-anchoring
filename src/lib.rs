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

extern crate bitcoin;
extern crate btc_transaction_utils;
extern crate byteorder;
extern crate secp256k1;
extern crate serde;
extern crate serde_str;

pub use service::{ANCHORING_SERVICE_ID, ANCHORING_SERVICE_NAME};

pub mod blockchain;
pub mod btc;
pub mod config;
pub mod service;
