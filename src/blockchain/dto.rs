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

use exonum::crypto::{Hash, PublicKey};
use exonum::helpers::ValidatorId;

use details::btc::transactions::{AnchoringTx, BitcoinTx};
use service::ANCHORING_SERVICE_ID;

pub const ANCHORING_MESSAGE_SIGNATURE: u16 = 0;
pub const ANCHORING_MESSAGE_LATEST: u16 = 1;

message! {
    /// Exonum message with the signature for the given input of the anchoring transaction.
    struct MsgAnchoringSignature {
        const TYPE = ANCHORING_SERVICE_ID;
        const ID = ANCHORING_MESSAGE_SIGNATURE;

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
}

message! {
    /// Exonum message with the updated validator's lect.
    struct MsgAnchoringUpdateLatest {
        const TYPE = ANCHORING_SERVICE_ID;
        const ID = ANCHORING_MESSAGE_LATEST;

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

encoding_struct! {
    /// Lect content
    struct LectContent {
        /// Hash of exonum transaction that contains this lect.
        msg_hash: &Hash,
        /// Bitcoin transaction content.
        tx: BitcoinTx,
    }
}
