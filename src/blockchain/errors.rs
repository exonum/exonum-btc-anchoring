// Copyright 2019 The Exonum Team
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

//! Error types of the BTC anchoring service.

use exonum::{blockchain::ExecutionError, crypto::Hash, helpers::ValidatorId};

use failure_derive::Fail;

use crate::btc;

/// Possible errors during execution of the `Signature` transaction.
#[derive(Debug, Fail)]
pub enum SignatureError {
    /// Received signature is for the incorrect anchoring transaction.
    #[fail(
        display = "Received signature is for the incorrect anchoring transaction. Expected: {}. Received: {}.",
        expected_id, received_id
    )]
    Unexpected {
        /// Expected identifier of the anchoring transaction.
        expected_id: Hash,
        /// Actually received identifier of the anchoring transaction.
        received_id: Hash,
    },
    /// Received signature for anchoring transaction while the node is in transition state.
    #[fail(
        display = "Received signature for anchoring transaction while the node is in transition state."
    )]
    InTransition,
    /// Public key of validator with the given identifier is missing.
    #[fail(display = "Public key of validator {} is missing.", _0)]
    MissingPublicKey {
        /// Validator identifier.
        validator_id: ValidatorId,
    },
    /// Input with the given index does not exist.
    #[fail(display = "Input with index {} does not exist.", _0)]
    NoSuchInput {
        /// Input index.
        idx: usize,
    },
    /// Signature verification failed.
    #[fail(display = "Signature verification failed.")]
    VerificationFailed,
    /// An error in transaction builder occurred.
    #[fail(display = "{}", _0)]
    TxBuilderError(btc::BuilderError),
    /// An unknown error occurred.
    #[fail(display = "Unknown error")]
    UnknownError,
}

/// Error codes for the BTC anchoring transactions.
#[derive(Debug)]
pub enum ErrorCode {
    /// [description](enum.SignatureError.html#variant.Unexpected)
    Unexpected = 1,
    /// [description](enum.SignatureError.html#variant.InTransition)
    InTransition = 2,
    /// [description](enum.SignatureError.html#variant.MissingPublicKey)
    MissingPublicKey = 3,
    /// [description](enum.SignatureError.html#variant.NoSuchInput)
    NoSuchInput = 4,
    /// [description](enum.SignatureError.html#variant.VerificationFailed)
    VerificationFailed = 5,
    /// [description](enum.SignatureError.html#variant.TxBuilderError)
    TxBuilderError = 6,
    /// [description](enum.SignatureError.html#variant.UnknownError)
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
    fn from(value: SignatureError) -> Self {
        let description = format!("{}", value);
        Self::with_description(value.code() as u8, description)
    }
}
