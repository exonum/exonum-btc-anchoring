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

extern crate exonum;
extern crate bitcoin;
extern crate byteorder;
extern crate btc_transaction_utils;

pub use service::{ANCHORING_SERVICE_NAME, ANCHORING_SERVICE_ID};

pub mod blockchain;
pub mod btc;
pub mod service;