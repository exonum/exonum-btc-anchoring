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

use secp256k1::{key, Secp256k1, Signing};

use exonum::storage::StorageKey;

use super::types::{PublicKey, RawPublicKey};

const PUBLIC_KEY_SIZE: usize = 33;

impl PublicKey {
    pub fn from_secret_key<C: Signing>(secp: &Secp256k1<C>, sk: &key::SecretKey) -> PublicKey {
        let raw = RawPublicKey::from_secret_key(secp, sk);
        PublicKey::from(raw)
    }

    pub fn to_bytes(&self) -> [u8; PUBLIC_KEY_SIZE] {
        self.serialize()
    }
}

impl StorageKey for PublicKey {
    fn size(&self) -> usize {
        PUBLIC_KEY_SIZE
    }

    fn write(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(&self.to_bytes())
    }

    fn read(buffer: &[u8]) -> Self {
        let ctx = Secp256k1::without_caps();
        let raw = RawPublicKey::from_slice(&ctx, buffer).unwrap();
        PublicKey(raw)
    }
}
