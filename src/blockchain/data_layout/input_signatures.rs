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

use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use exonum::{
    crypto::{self, Hash},
    helpers::ValidatorId,
};
use exonum_merkledb::{BinaryValue, ObjectHash};
use serde_derive::{Deserialize, Serialize};

use std::{
    borrow::Cow,
    io::{Cursor, Write},
    iter::{FilterMap, IntoIterator},
    vec::IntoIter,
};

/// A set of signatures for a transaction input ordered by the validators identifiers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputSignatures {
    content: Vec<Option<Vec<u8>>>,
}

impl InputSignatures {
    /// Creates an empty signatures set for the given validators count.
    pub fn new(validators_count: usize) -> Self {
        let content = vec![None; validators_count as usize];
        Self { content }
    }

    /// Inserts a signature from the validator with the given identifier.
    pub fn insert(&mut self, id: ValidatorId, signature: Vec<u8>) {
        let index = id.0 as usize;
        self.content[index] = Some(signature);
    }

    /// Checks the existence of a signature from the validator with the given identifier.
    pub fn contains(&self, id: ValidatorId) -> bool {
        let index = id.0 as usize;
        self.content[index].is_some()
    }

    /// Returns the total count of signatures.
    pub fn len(&self) -> usize {
        self.content.iter().filter(|x| x.is_some()).count()
    }

    /// Checks that signatures set is not empty.
    pub fn is_empty(&self) -> bool {
        self.content.iter().any(|x| x.is_some())
    }
}

type OpSig = Option<Vec<u8>>;
impl IntoIterator for InputSignatures {
    type Item = Vec<u8>;
    type IntoIter = FilterMap<IntoIter<OpSig>, fn(_: OpSig) -> OpSig>;

    fn into_iter(self) -> Self::IntoIter {
        self.content.into_iter().filter_map(|x| x)
    }
}

impl BinaryValue for InputSignatures {
    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        for signature in &self.content {
            let bytes = signature
                .as_ref()
                .map_or_else(|| [].as_ref(), |x| &x.as_slice());
            buf.write_u64::<LittleEndian>(bytes.len() as u64).unwrap();
            buf.write_all(bytes).unwrap();
        }
        buf.into_inner()
    }

    fn from_bytes(value: Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut signatures = Vec::new();
        let mut reader = value.as_ref();
        while !reader.is_empty() {
            let bytes_len = LittleEndian::read_u64(reader) as usize;
            reader = &reader[8..];
            let signature = if bytes_len == 0 {
                None
            } else {
                let buf = Some(reader[0..bytes_len].to_vec());
                reader = &reader[bytes_len..];
                buf
            };
            signatures.push(signature);
        }

        Ok(Self {
            content: signatures,
        })
    }
}

impl ObjectHash for InputSignatures {
    fn object_hash(&self) -> Hash {
        crypto::hash(&self.to_bytes())
    }
}

#[test]
fn test_input_signatures_storage_value() {
    let mut signatures = InputSignatures::new(4);
    let data = vec![
        b"abacaba1224634abcfdfdfca353".to_vec(),
        b"abacaba1224634abcfdfdfca353ee2224774".to_vec(),
    ];
    signatures.insert(ValidatorId(3), data[1].clone());
    signatures.insert(ValidatorId(1), data[0].clone());
    assert_eq!(signatures.len(), 2);

    let bytes = signatures.clone().into_bytes();
    let signatures2 = InputSignatures::from_bytes(bytes.into()).unwrap();
    assert_eq!(signatures, signatures2);
    assert_eq!(signatures2.into_iter().collect::<Vec<_>>(), data);
}
