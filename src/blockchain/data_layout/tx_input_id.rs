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

use exonum::crypto::{self, CryptoHash, Hash};
use exonum::storage::{HashedKey, StorageKey};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use std::io::{Cursor, Read, Write};

/// Unique transaction input identifier composed of a transaction identifier
/// and an input index.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct TxInputId {
    /// Transaction identifier.
    pub txid: Hash,
    /// Transaction input index.
    pub input: u32,
}

impl TxInputId {
    /// Creates a new identifier.
    pub fn new(txid: Hash, input: u32) -> TxInputId {
        TxInputId { txid, input }
    }
}

impl StorageKey for TxInputId {
    fn size(&self) -> usize {
        self.txid.size() + self.input.size()
    }

    fn read(inp: &[u8]) -> Self {
        let mut reader = Cursor::new(inp);

        let txid = {
            let mut txid = [0u8; 32];
            let _ = reader.read(&mut txid).unwrap();
            Hash::new(txid)
        };
        let input = reader.read_u32::<LittleEndian>().unwrap();
        TxInputId { txid, input }
    }

    fn write(&self, out: &mut [u8]) {
        let mut writer = Cursor::new(out);
        let _ = writer.write(self.txid.as_ref()).unwrap();
        writer.write_u32::<LittleEndian>(self.input).unwrap();
    }
}

impl CryptoHash for TxInputId {
    fn hash(&self) -> Hash {
        let mut bytes = [0u8; 36];
        self.write(&mut bytes);
        crypto::hash(bytes.as_ref())
    }
}

impl HashedKey for TxInputId {}

#[test]
fn test_tx_input_id_storage_key() {
    let txout = TxInputId {
        txid: crypto::hash(&[1, 2, 3]),
        input: 2,
    };

    let mut buf = vec![0u8; txout.size()];
    txout.write(&mut buf);

    let txout2 = TxInputId::read(&buf);
    assert_eq!(txout, txout2);

    let buf_hash = crypto::hash(&buf);
    assert_eq!(txout2.hash(), buf_hash);
}
