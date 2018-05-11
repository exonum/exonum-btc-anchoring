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
use exonum::helpers::ValidatorId;
use exonum::storage::{HashedKey, StorageKey, StorageValue};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use std::io::{Cursor, Read, Write};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct TxInputId {
    pub txid: Hash,
    pub index: u32,
}

impl TxInputId {
    pub fn new(txid: Hash, index: u32) -> TxInputId {
        TxInputId { txid, index }
    }
}

impl StorageKey for TxInputId {
    fn size(&self) -> usize {
        self.txid.size() + self.index.size()
    }

    fn read(inp: &[u8]) -> Self {
        let mut reader = Cursor::new(inp);

        let txid = {
            let mut txid = [0u8; 32];
            reader.read(&mut txid).unwrap();
            Hash::new(txid)
        };
        let index = reader.read_u32::<BigEndian>().unwrap();
        TxInputId { txid, index }
    }

    fn write(&self, out: &mut [u8]) {
        let mut writer = Cursor::new(out);
        writer.write(self.txid.as_ref()).unwrap();
        writer.write_u32::<BigEndian>(self.index).unwrap();
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
        index: 2,
    };

    let mut buf = vec![0u8; txout.size()];
    txout.write(&mut buf);

    let txout2 = TxInputId::read(&buf);
    assert_eq!(txout, txout2);

    let buf_hash = crypto::hash(&buf);
    assert_eq!(txout2.hash(), buf_hash);
}
