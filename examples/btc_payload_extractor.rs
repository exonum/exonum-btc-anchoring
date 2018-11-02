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

extern crate exonum;
extern crate exonum_btc_anchoring;

extern crate structopt;
extern crate serde_json;
#[macro_use]
extern crate failure;

use structopt::StructOpt;

use exonum::encoding::serialize::FromHex;
use exonum_btc_anchoring::btc::Transaction;

/// BTC anchoring payload extractor
///
/// Extracts and prints JSON object with payload of the given anchoring transaction.
#[derive(StructOpt)]
struct Opts {
    /// Bitcoin transaction hex.
    hex: String,
}

fn main() -> Result<(), failure::Error> {
    let transaction = Transaction::from_hex(Opts::from_args().hex)?;
    let payload = transaction
        .anchoring_payload()
        .ok_or_else(|| format_err!("Given transaction does not contains anchoring payload"))?;
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}
