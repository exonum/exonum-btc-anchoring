use std::fmt;

use exonum::crypto::{PublicKey, Hash};
use exonum::messages::{RawTransaction, Message, FromRaw, Error as MessageError};

use details::transactions::{AnchoringTx, BitcoinTx};

#[doc(hidden)]
pub const ANCHORING_SERVICE_ID: u16 = 3;
const ANCHORING_MESSAGE_SIGNATURE: u16 = 0;
const ANCHORING_MESSAGE_LATEST: u16 = 1;

message! {
    MsgAnchoringSignature {
        const TYPE = ANCHORING_SERVICE_ID;
        const ID = ANCHORING_MESSAGE_SIGNATURE;
        const SIZE = 56;

        from:           &PublicKey   [00 => 32]
        validator:      u32          [32 => 36]
        tx:             AnchoringTx  [36 => 44]
        input:          u32          [44 => 48]
        signature:      &[u8]        [48 => 56]
    }
}

message! {
    MsgAnchoringUpdateLatest {
        const TYPE = ANCHORING_SERVICE_ID;
        const ID = ANCHORING_MESSAGE_LATEST;
        const SIZE = 52;

        from:           &PublicKey   [00 => 32]
        validator:      u32          [32 => 36]
        tx:             BitcoinTx    [36 => 44]
        lect_count:     u64          [44 => 52]
    }
}

#[doc(hidden)]
#[derive(Clone)]
pub enum AnchoringMessage {
    Signature(MsgAnchoringSignature),
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
    fn from_raw(raw: RawTransaction) -> ::std::result::Result<AnchoringMessage, MessageError> {
        match raw.message_type() {
            ANCHORING_MESSAGE_SIGNATURE => {
                Ok(AnchoringMessage::Signature(MsgAnchoringSignature::from_raw(raw)?))
            }
            ANCHORING_MESSAGE_LATEST => {
                Ok(AnchoringMessage::UpdateLatest(MsgAnchoringUpdateLatest::from_raw(raw)?))
            }
            _ => Err(MessageError::IncorrectMessageType { message_type: raw.message_type() }),
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
