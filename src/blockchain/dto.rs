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

use details::btc::transactions::{AnchoringTx, BitcoinTx};
use service::ANCHORING_SERVICE_ID;

pub const ANCHORING_MESSAGE_SIGNATURE: u16 = 0;
pub const ANCHORING_MESSAGE_LATEST: u16 = 1;

message! {
    /// Exonum message with the signature for the given input of the anchoring transaction.
    struct MsgAnchoringSignature {
        const TYPE = ANCHORING_SERVICE_ID;
        const ID = ANCHORING_MESSAGE_SIGNATURE;
        const SIZE = 54;

        /// Public key of validator.
        field from:           &PublicKey   [00 => 32]
        /// Public key index in anchoring public keys list.
        field validator:      u16          [32 => 34]
        /// Transaction content.
        field tx:             AnchoringTx  [34 => 42]
        /// Signed input.
        field input:          u32          [42 => 46]
        /// Signature.
        field signature:      &[u8]        [46 => 54]
    }
}

message! {
    /// Exonum message with the updated validator's lect.
    struct MsgAnchoringUpdateLatest {
        const TYPE = ANCHORING_SERVICE_ID;
        const ID = ANCHORING_MESSAGE_LATEST;
        const SIZE = 50;

        /// Public key of validator.
        field from:           &PublicKey   [00 => 32]
        /// Public key index in anchoring public keys list.
        field validator:      u16          [32 => 34]
        /// Lect content.
        field tx:             BitcoinTx    [34 => 42]
        /// Current lects count in the `lects` table for the current validator.
        field lect_count:     u64          [42 => 50]
    }
}

encoding_struct! {
    /// Lect content
    struct LectContent {
        const SIZE = 40;

        /// Hash of `exonum` transaction that contains this lect.
        field msg_hash:       &Hash       [00 => 32]
        /// Bitcoin transaction content.
        field tx:             BitcoinTx   [32 => 40]
    }
}
