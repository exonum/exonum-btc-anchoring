use std::fmt;

use exonum::crypto::{Hash, PublicKey};
use exonum::messages::{FromRaw, Message, RawTransaction};
use exonum::encoding::Error as StreamStructError;

use details::btc::transactions::{AnchoringTx, BitcoinTx};
use service::ANCHORING_SERVICE_ID;

const ANCHORING_MESSAGE_SIGNATURE: u16 = 0;
const ANCHORING_MESSAGE_LATEST: u16 = 1;

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
    /// Exonum message with the updated validator's `lect`.
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

        /// Hash of `exonum` transaction that contains this `lect`.
        field msg_hash:       &Hash       [00 => 32]
        /// Bitcoin transaction content.
        field tx:             BitcoinTx   [32 => 40]
    }
}

/// Generalized type of any anchoring messages.
#[derive(Clone, Serialize)]
pub enum AnchoringMessage {
    /// Signature message
    Signature(MsgAnchoringSignature),
    /// Lect message.
    UpdateLatest(MsgAnchoringUpdateLatest),
}

impl Into<AnchoringMessage> for MsgAnchoringSignature {
    fn into(self) -> AnchoringMessage {
        AnchoringMessage::Signature(self)
    }
}

impl Into<AnchoringMessage> for MsgAnchoringUpdateLatest {
    fn into(self) -> AnchoringMessage {
        AnchoringMessage::UpdateLatest(self)
    }
}

impl AnchoringMessage {
    pub fn from(&self) -> &PublicKey {
        match *self {
            AnchoringMessage::UpdateLatest(ref msg) => msg.from(),
            AnchoringMessage::Signature(ref msg) => msg.from(),
        }
    }
}

impl Message for AnchoringMessage {
    fn raw(&self) -> &RawTransaction {
        match *self {
            AnchoringMessage::UpdateLatest(ref msg) => msg.raw(),
            AnchoringMessage::Signature(ref msg) => msg.raw(),
        }
    }

    fn verify_signature(&self, public_key: &PublicKey) -> bool {
        match *self {
            AnchoringMessage::UpdateLatest(ref msg) => msg.verify_signature(public_key),
            AnchoringMessage::Signature(ref msg) => msg.verify_signature(public_key),
        }
    }

    fn hash(&self) -> Hash {
        match *self {
            AnchoringMessage::UpdateLatest(ref msg) => Message::hash(msg),
            AnchoringMessage::Signature(ref msg) => Message::hash(msg),
        }
    }
}

impl FromRaw for AnchoringMessage {
    fn from_raw(raw: RawTransaction) -> ::std::result::Result<AnchoringMessage, StreamStructError> {
        match raw.message_type() {
            ANCHORING_MESSAGE_SIGNATURE => {
                Ok(AnchoringMessage::Signature(MsgAnchoringSignature::from_raw(raw)?))
            }
            ANCHORING_MESSAGE_LATEST => {
                Ok(AnchoringMessage::UpdateLatest(MsgAnchoringUpdateLatest::from_raw(raw)?))
            }
            _ => Err("Expected different message type".into()),
        }
    }
}

impl fmt::Debug for AnchoringMessage {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            AnchoringMessage::UpdateLatest(ref msg) => write!(fmt, "{:?}", msg),
            AnchoringMessage::Signature(ref msg) => write!(fmt, "{:?}", msg),
        }
    }
}
