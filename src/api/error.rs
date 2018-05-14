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

use std::error;
use std::fmt;

use exonum::api::ApiError;
use exonum::storage::Error as StorageError;

#[derive(Debug)]
pub enum Error {
    UnknownValidatorId(u32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::UnknownValidatorId(id) => write!(f, "Unknown validator id={}", id),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::UnknownValidatorId(_) => "UnknownValidatorId",
        }
    }
}

impl Into<ApiError> for Error {
    fn into(self) -> ApiError {
        match self {
            Error::UnknownValidatorId(id) => {
                ApiError::Storage(StorageError::new(format!("Unknown validator id={}", id)))
            }
        }
    }
}
