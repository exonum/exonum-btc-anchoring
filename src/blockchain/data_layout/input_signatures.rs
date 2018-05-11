// Copyright 2018 The Exonum Team
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

use exonum::crypto::{CryptoHash, Hash};
use exonum::helpers::ValidatorId;
use exonum::storage::StorageValue;

use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq)]
pub struct InputSignatures {
    content: Vec<Option<Vec<u8>>>,
}

impl InputSignatures {
    pub fn new(validators_count: u16) -> InputSignatures {
        let content = vec![None; validators_count as usize];
        InputSignatures { content }
    }

    pub fn insert(&mut self, id: ValidatorId, signature: Vec<u8>) {
        let index = id.0 as usize;
        self.content[index] = Some(signature);
    }

    pub fn into_iter(self) -> impl Iterator<Item = Vec<u8>> {
        self.content.into_iter().filter_map(|x| x)
    }
}

encoding_struct! {
    struct InputSignature {
        content: &[u8]
    }
}

encoding_struct! {
    struct InputSignaturesStored {
        content: Vec<InputSignature>
    }
}

impl From<Option<Vec<u8>>> for InputSignature {
    fn from(s: Option<Vec<u8>>) -> InputSignature {
        let bytes = s.as_ref()
            .map(|x| x.as_ref())
            .unwrap_or_else(|| [].as_ref());
        InputSignature::new(bytes)
    }
}

impl From<InputSignature> for Option<Vec<u8>> {
    fn from(s: InputSignature) -> Option<Vec<u8>> {
        if s.content().is_empty() {
            None
        } else {
            Some(s.content().to_vec())
        }
    }
}

impl From<InputSignatures> for InputSignaturesStored {
    fn from(s: InputSignatures) -> InputSignaturesStored {
        let content = s.content.into_iter().map(From::from).collect::<_>();
        InputSignaturesStored::new(content)
    }
}

impl From<InputSignaturesStored> for InputSignatures {
    fn from(s: InputSignaturesStored) -> InputSignatures {
        let content = s.content().into_iter().map(From::from).collect::<_>();
        InputSignatures { content }
    }
}

impl StorageValue for InputSignatures {
    fn into_bytes(self) -> Vec<u8> {
        let stored = InputSignaturesStored::from(self);
        stored.into_bytes()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        let stored = InputSignaturesStored::from_bytes(value);
        stored.into()
    }
}

impl CryptoHash for InputSignatures {
    fn hash(&self) -> Hash {
        InputSignaturesStored::from(self.clone()).hash()
    }
}
