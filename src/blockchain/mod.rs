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

//! Blockchain implementation details for the anchoring service.

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

pub use self::schema::{AnchoringSchema, KnownSignatureId};
pub use self::dto::{LectContent, MsgAnchoringSignature, MsgAnchoringUpdateLatest};
