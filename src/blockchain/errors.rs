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

use exonum::runtime::ExecutionError;
use exonum_derive::IntoExecutionError;

use crate::btc;

/// Possible errors during execution of the `sign_input` method.
#[derive(Debug, IntoExecutionError)]
pub enum Error {
    /// Transaction author is not authorized to sign anchoring transactions.
    UnauthorizedAnchoringKey = 0,
    /// Transaction input with the specified index is absent in the anchoring proposal.
    NoSuchInput = 1,
    /// The transaction input signature is invalid.
    InputVerificationFailed = 2,
    /// An error occurred while creating of the anchoring transaction proposal.
    AnchoringBuilderError = 3,
    /// Unexpected anchoring proposal transaction ID.
    UnexpectedProposalTxId = 4,
    /// Funding transaction has been already used.
    AlreadyUsedFundingTx = 5,
    /// Funding transaction is unsuitable.
    UnsuitableFundingTx = 6,
}

impl Error {
    /// Creates an error instance from the anchoring transaction builder error.
    pub fn anchoring_builder_error(error: btc::BuilderError) -> ExecutionError {
        (Error::AnchoringBuilderError, error).into()
    }
}
