// Copyright 2017 The Exonum Team
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

use std::fmt;
use std::error;

use exonum::helpers::Height;

use details::btc::transactions::BitcoinTx;

#[derive(Debug, PartialEq)]
pub enum Error {
    IncorrectLect { reason: String, tx: BitcoinTx },
    LectNotFound { height: Height },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IncorrectLect { ref reason, ref tx } => {
                write!(f, "Incorrect lect: {}, tx={:#?}", reason, tx)
            }
            Error::LectNotFound { height } => {
                write!(f, "Suitable lect not found for height={}", height)
            }
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::IncorrectLect { .. } => "Incorrect lect",
            Error::LectNotFound { .. } => "Suitable lect not found",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}
