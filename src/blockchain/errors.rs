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

use btc;
use exonum::blockchain::ExecutionError;
use exonum::crypto::Hash;
use exonum::helpers::ValidatorId;

#[derive(Debug, Fail)]
pub enum SignatureError {
    #[fail(
        display = "Received signature for the incorrect anchoring transaction. Expected: {:?}. Received: {:?}.",
        expected_id,
        received_id
    )]
    Unexpected {
        expected_id: Hash,
        received_id: Hash,
    },
    #[fail(display = "Received signature for anchoring transaction while in transition state.")]
    InTransition,
    #[fail(display = "Public key of validator {:?} is missing.", _0)]
    MissingPublicKey { validator_id: ValidatorId },
    #[fail(display = "Input with index {} doesn't exist.", _0)]
    NoSuchInput { idx: usize },
    #[fail(display = "Signature verification failed.")]
    VerificationFailed,
    #[fail(display = "{}", _0)]
    TxBuilderError(btc::BuilderError),
    #[fail(display = "Unknown error")]
    UnknownError,
}

#[derive(Debug)]
pub enum ErrorCode {
    Unexpected = 1,
    InTransition = 2,
    MissingPublicKey = 3,
    NoSuchInput = 4,
    VerificationFailed = 5,
    TxBuilderError = 6,
    UnknownError = 255,
}

impl SignatureError {
    fn code(&self) -> ErrorCode {
        match self {
            SignatureError::Unexpected { .. } => ErrorCode::Unexpected,
            SignatureError::InTransition => ErrorCode::InTransition,
            SignatureError::MissingPublicKey { .. } => ErrorCode::MissingPublicKey,
            SignatureError::NoSuchInput { .. } => ErrorCode::NoSuchInput,
            SignatureError::VerificationFailed => ErrorCode::VerificationFailed,
            SignatureError::TxBuilderError(..) => ErrorCode::TxBuilderError,
            _ => ErrorCode::UnknownError,
        }
    }
}

impl From<SignatureError> for ExecutionError {
    fn from(value: SignatureError) -> ExecutionError {
        let description = format!("{}", value);
        ExecutionError::with_description(value.code() as u8, description)
    }
}
