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

use std::io;

pub use details::error::Error as InternalError;
pub use handler::error::Error as HandlerError;
use bitcoinrpc::Error as RpcError;

/// Anchoring btc service Error type.
#[derive(Debug, Error)]
pub enum Error {
    /// Internal error
    Internal(InternalError),
    /// Handler error.
    Handler(HandlerError),
}

impl From<RpcError> for Error {
    fn from(err: RpcError) -> Error {
        Error::Internal(InternalError::Rpc(err))
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Internal(InternalError::Io(err))
    }
}
