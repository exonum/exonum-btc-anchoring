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

use exonum::blockchain::{Transaction, TransactionSet};
use exonum::crypto::{Hash, PublicKey};
use exonum::encoding::Error as EncodingError;
use exonum::helpers::ValidatorId;
use exonum::messages::RawTransaction;

use details::btc::transactions::{AnchoringTx, BitcoinTx};
use service::ANCHORING_SERVICE_ID;

pub const ANCHORING_MESSAGE_SIGNATURE: u16 = 0;
pub const ANCHORING_MESSAGE_LATEST: u16 = 1;

transactions! {
    Messages {
        const SERVICE_ID = ANCHORING_SERVICE_ID;

        /// Exonum message with the signature for the new anchoring transaction.
        struct MsgAnchoringSignature {
            /// Public key of validator.
            from: &PublicKey,
            /// Public key index in anchoring public keys list.
            validator: ValidatorId,
            /// Transaction content.
            tx: AnchoringTx,
            /// Signed input.
            input: u32,
            /// Signature for the corresponding `input`.
            signature: &[u8],
        }
        /// Exonum message with the updated validator's lect.
        struct MsgAnchoringUpdateLatest {
            /// Public key of validator.
            from: &PublicKey,
            /// Public key index in anchoring public keys list.
            validator: ValidatorId,
            /// Lect content.
            tx: BitcoinTx,
            /// Current lects count in the `lects` table for the current validator.
            lect_count: u64,
        }
    }
}

encoding_struct! {
    /// Last expected correct transaction content.
    struct LectContent {
        /// Hash of exonum transaction that contains this lect.
        msg_hash: &Hash,
        /// Bitcoin transaction content.
        tx: BitcoinTx,
    }
}

/// Constructs anchoring transaction from the given raw message.
pub(crate) fn tx_from_raw(raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
    Messages::tx_from_raw(raw).map(Into::into)
}
