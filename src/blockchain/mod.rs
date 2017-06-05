//! An anchoring blockchain implementation details.

#[doc(hidden)]
pub mod schema;
#[doc(hidden)]
pub mod dto;
#[doc(hidden)]
pub mod transactions;
#[doc(hidden)]
pub mod consensus_storage;
#[cfg(test)]
mod tests;

pub use self::dto::{ANCHORING_SERVICE_ID, LectContent, MsgAnchoringSignature,
                    MsgAnchoringUpdateLatest};

pub use self::schema::AnchoringSchema;