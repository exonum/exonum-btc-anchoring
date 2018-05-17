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

use bitcoin::util::privkey;
use secp256k1;

pub use self::payload::{Payload, PayloadBuilder};
pub use self::transaction::{AnchoringTransactionBuilder, Transaction};

#[macro_use]
mod macros;

pub mod payload;
pub mod rpc;
pub mod transaction;

/// A Bitcoin ECDSA private key.
#[derive(Clone, From, Into, PartialEq)]
pub struct Privkey(pub privkey::Privkey);

/// A Secp256k1 public key, used for verification of signatures.
#[derive(Debug, Clone, From, Into, PartialEq)]
pub struct PublicKey(pub secp256k1::PublicKey);

impl ToString for Privkey {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl ::std::str::FromStr for Privkey {
    type Err = <privkey::Privkey as ::std::str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        privkey::Privkey::from_str(s).map(From::from)
    }
}

impl ::std::fmt::Debug for Privkey {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.debug_struct("Privkey").finish()
    }
}

impl ::exonum::encoding::serialize::FromHex for PublicKey {
    type Error = ::failure::Error;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        let bytes = ::exonum::encoding::serialize::decode_hex(hex)?;
        let context = secp256k1::Secp256k1::without_caps();
        let inner = secp256k1::PublicKey::from_slice(&context, &bytes)?;
        Ok(PublicKey(inner))
    }
}

impl ::exonum::encoding::serialize::ToHex for PublicKey {
    fn write_hex<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        let bytes = self.0.serialize();
        bytes.as_ref().write_hex(w)
    }

    fn write_hex_upper<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
        let bytes = self.0.serialize();
        bytes.as_ref().write_hex_upper(w)
    }
}

impl_serde_str! { Privkey }

impl_string_conversions_for_hex! { PublicKey }
impl_serde_str! { PublicKey }
