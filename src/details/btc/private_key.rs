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

use bitcoin::network::constants::Network;
use secp256k1::key;

use super::types::{PrivateKey, RawPrivkey};

impl PrivateKey {
    pub fn from_key(network: Network, sk: key::SecretKey, compressed: bool) -> PrivateKey {
        RawPrivkey::from_secret_key(sk, compressed, network).into()
    }
}
