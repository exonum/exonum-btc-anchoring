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
    /// Received lect from the non-validator node
    #[display(fmt = "Received message from the non-validator node")]
    MsgFromNonValidator = 0,
    /// Received lect with the incorrect payload
    #[display(fmt = "Received message with the incorrect payload")]
    MsgWithIncorrectPayload = 1,
    /// Received message with the incorrect output address
    #[display(fmt = "Received message with the incorrect output address")]
    MsgWithIncorrectAddress = 2,
    /// Received lect with prev_lect without +2/3 confirmations
    #[display(fmt = "Received lect with prev_lect without +2/3 confirmations")]
    LectWithoutQuorum = 3,
    /// Received lect with incorrect funding_tx
    #[display(fmt = "Received lect with incorrect funding_tx")]
    LectWithIncorrectFunding = 4,
    /// Received lect with incorrect content
    #[display(fmt = "Received lect with incorrect content")]
    LectWithIncorrectContent = 5,
    /// Received lect with wrong count
    #[display(fmt = "Received lect with wrong count")]
    LectWithWrongCount = 6,
    /// Received message with the incorrect signature
    #[display(fmt = "Received message with the incorrect signature")]
    SignatureIncorrect = 7,
    /// Received another signature for given tx propose
    #[display(fmt = "Received another signature for given tx propose")]
    SignatureDifferent = 8,
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
