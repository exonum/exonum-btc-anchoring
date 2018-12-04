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

#[macro_export]
macro_rules! impl_wrapper_for_bitcoin_type {
    ($name:ident) => {
        impl_wrapper_for_bitcoin_consensus_encoding! { $name }
        impl_string_conversions_for_hex! { $name }
        impl_serde_str! { $name }
    };
}

#[macro_export]
macro_rules! impl_wrapper_for_bitcoin_consensus_encoding {
    ($name:ident) => {
        impl ::exonum::storage::StorageValue for $name {
            fn into_bytes(self) -> Vec<u8> {
                ::bitcoin::consensus::serialize(&self.0)
            }

            fn from_bytes(value: ::std::borrow::Cow<[u8]>) -> $name {
                let inner = ::bitcoin::consensus::deserialize(value.as_ref()).unwrap();
                $name(inner)
            }
        }

        impl ::exonum::crypto::CryptoHash for $name {
            fn hash(&self) -> ::exonum::crypto::Hash {
                let bytes = ::bitcoin::consensus::serialize(&self.0);
                ::exonum::crypto::hash(&bytes)
            }
        }

        impl ::exonum::encoding::serialize::FromHex for $name {
            type Error = ::failure::Error;

            fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
                let bytes = ::exonum::encoding::serialize::decode_hex(hex)?;
                let inner = ::bitcoin::consensus::deserialize(bytes.as_ref())?;
                Ok($name(inner))
            }
        }

        impl ::exonum::encoding::serialize::ToHex for $name {
            fn write_hex<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
                let bytes = ::bitcoin::consensus::serialize(&self.0);
                bytes.write_hex(w)
            }

            fn write_hex_upper<W: ::std::fmt::Write>(&self, w: &mut W) -> ::std::fmt::Result {
                let bytes = ::bitcoin::consensus::serialize(&self.0);
                bytes.write_hex_upper(w)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_string_conversions_for_hex {
    ($name:ident) => {
        impl ::std::fmt::LowerHex for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                use exonum::encoding::serialize::ToHex;
                let mut buf = String::new();
                self.write_hex(&mut buf).unwrap();
                write!(f, "{}", buf)
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(f, "{:x}", self)
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = ::failure::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                use exonum::encoding::serialize::FromHex;
                Self::from_hex(s).map_err(From::from)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_serde_str {
    ($name:ident) => {
        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, ser: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                ::serde_str::serialize(self, ser)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                ::serde_str::deserialize(deserializer)
            }
        }
    };
}
