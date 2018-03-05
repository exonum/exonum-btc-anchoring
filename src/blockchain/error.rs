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

use exonum::blockchain::ExecutionError;

/// Error codes for the anchoring service.
#[derive(Debug, Fail, Display, Clone, Copy)]
#[repr(u8)]
pub enum Error {
    /// Received lect from the non validator node
    #[display(fmt = "Received message from the non validator node")]
    MsgFromNonValidator = 0,
    /// Received lect with the incorrect payload
    #[display(fmt = "Received message with the incorrect payload")]
    MsgWithIncorrectPayload,
    /// Received msg with incorrect output address
    #[display(fmt = "Received message with the incorrect output address")]
    MsgWithIncorrectAddress,
    /// Received lect with prev_lect without 2/3+ confirmations
    #[display(fmt = "Received lect with prev_lect without 2/3+ confirmations")]
    LectWithoutQuorum,
    /// Received lect with incorrect funding_tx
    #[display(fmt = "Received lect with incorrect funding_tx")]
    LectWithIncorrectFunding,
    /// Received lect with incorrect content
    #[display(fmt = "Received lect with incorrect content")]
    LectWithIncorrectContent,
    /// Received lect with wrong count
    #[display(fmt = "Received lect with wrong count")]
    LectWithWrongCount,
    /// Received msg with incorrect output address
    #[display(fmt = "Received message with the incorrect signature")]
    SignatureIncorrect,
    /// Received another signature for given tx propose
    #[display(fmt = "Received another signature for given tx propose")]
    SignatureDifferent,
}

impl Error {
    /// Converts error to the raw code
    pub fn as_code(self) -> u8 {
        self as u8
    }
}

impl From<Error> for ExecutionError {
    fn from(value: Error) -> ExecutionError {
        ExecutionError::new(value as u8)
    }
}

impl From<Error> for u8 {
    fn from(value: Error) -> u8 {
        value as u8
    }
}
