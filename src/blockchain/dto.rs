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
