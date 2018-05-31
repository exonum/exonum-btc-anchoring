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

macro_rules! implement_wrapper {
    ($from:ident, $to:ident) => {
        impl Deref for $to {
            type Target = $from;

            fn deref(&self) -> &$from {
                &self.0
            }
        }

        impl From<$from> for $to {
            fn from(p: $from) -> $to {
                $to(p)
            }
        }

        impl From<$to> for $from {
            fn from(p: $to) -> $from {
                p.0
            }
        }

        impl PartialEq<$from> for $to {
            fn eq(&self, other: &$from) -> bool {
                self.0.eq(other)
            }
        }
    };
}

macro_rules! implement_str_conversion {
    ($from:ident, $to:ident) => {
        impl ::std::str::FromStr for $to {
            type Err = <$from as ::std::str::FromStr>::Err;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok($to::from($from::from_str(s)?))
            }
        }

        impl From<&'static str> for $to {
            fn from(s: &'static str) -> $to {
                use std::str::FromStr;
                $to::from_str(s).unwrap()
            }
        }

        impl ::std::fmt::Display for $to {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                f.write_str(&self.0.to_string())
            }
        }

        // FIXME: Known issue in clippy lints.
        // https://rust-lang-nursery.github.io/rust-clippy/master/index.html#write_literal
        #[cfg_attr(feature = "cargo-clippy", allow(write_literal))]

        impl fmt::Debug for $to {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "\"{}({})\"", stringify!($to), self.to_string())
            }
        }
    };
}

macro_rules! implement_serde_hex {
    ($name:ident) => {
        impl ::std::str::FromStr for $name {
            type Err = $crate::exonum::encoding::serialize::FromHexError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                use $crate::exonum::encoding::serialize::FromHex;
                $name::from_hex(s)
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                use $crate::exonum::encoding::serialize::ToHex;
                self.write_hex(f)
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, ser: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                ser.serialize_str(&self.to_string())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                struct HexVisitor;

                impl<'v> ::serde::de::Visitor<'v> for HexVisitor {
                    type Value = $name;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                        write!(fmt, "Expected hex represented string")
                    }

                    fn visit_str<E>(self, hex: &str) -> Result<$name, E>
                    where
                        E: ::serde::de::Error,
                    {
                        match $name::from_hex(hex) {
                            Ok(value) => Ok(value),
                            Err(_) => Err(::serde::de::Error::custom("Wrong hex")),
                        }
                    }
                }

                deserializer.deserialize_str(HexVisitor)
            }
        }
    };
}

macro_rules! implement_serde_string {
    ($name:ident) => {
        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, ser: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                ser.serialize_str(&self.to_string())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                struct FromStrVisitor;

                impl<'v> ::serde::de::Visitor<'v> for FromStrVisitor {
                    type Value = $name;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                        write!(fmt, "Unable to decode string")
                    }

                    fn visit_str<E>(self, s: &str) -> Result<$name, E>
                    where
                        E: ::serde::de::Error,
                    {
                        use std::str::FromStr;
                        $name::from_str(s).map_err(|_| ::serde::de::Error::custom("Wrong string"))
                    }
                }

                deserializer.deserialize_str(FromStrVisitor)
            }
        }
    };
}

macro_rules! implement_tx_wrapper {
    ($name:ident) => {
        implement_wrapper! {RawBitcoinTx, $name}

        impl $name {
            pub fn id(&self) -> TxId {
                let hash = self.0.txid();
                TxId::from(hash)
            }

            pub fn wid(&self) -> TxId {
                TxId::from(self.0.bitcoin_hash())
            }

            pub fn nid(&self) -> TxId {
                TxId::from(self.0.ntxid())
            }

            pub fn txid(&self) -> String {
                self.id().to_string()
            }

            pub fn ntxid(&self) -> String {
                self.nid().to_string()
            }

            pub fn wtxid(&self) -> String {
                self.wid().to_string()
            }

            pub fn to_hex(&self) -> String {
                use $crate::exonum::encoding::serialize::ToHex;
                let mut out = String::new();
                self.write_hex(&mut out).unwrap();
                out
            }

            pub fn has_witness(&self) -> bool {
                self.0.input.iter().any(|input| input.witness.is_empty())
            }
        }

        impl $crate::exonum::encoding::serialize::ToHex for $name {
            fn write_hex<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
                let string = $crate::bitcoin::network::serialize::serialize_hex(&self.0).unwrap();
                w.write_str(&string)
            }

            fn write_hex_upper<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
                let string = $crate::bitcoin::network::serialize::serialize_hex(&self.0).unwrap();
                w.write_str(&string)
            }
        }

        impl $crate::exonum::encoding::serialize::FromHex for $name {
            type Error = $crate::exonum::encoding::serialize::FromHexError;

            fn from_hex<T: AsRef<[u8]>>(v: T) -> Result<Self, Self::Error> {
                let bytes = Vec::<u8>::from_hex(v)?;
                if let Ok(tx) = deserialize::<RawBitcoinTx>(bytes.as_ref()) {
                    Ok($name::from(tx))
                } else {
                    Err($crate::exonum::encoding::serialize::FromHexError::InvalidStringLength)
                }
            }
        }

        impl $crate::exonum::storage::StorageValue for $name {
            fn into_bytes(self) -> Vec<u8> {
                let mut v = Vec::new();
                v.extend(serialize(&self.0).unwrap());
                v
            }

            fn from_bytes(v: ::std::borrow::Cow<[u8]>) -> Self {
                let tx = deserialize::<RawBitcoinTx>(v.as_ref()).unwrap();
                $name::from(tx)
            }
        }

        impl $crate::exonum::crypto::CryptoHash for $name {
            fn hash(&self) -> $crate::exonum::crypto::Hash {
                let mut v = Vec::new();
                v.extend(serialize(&self.0).unwrap());
                hash(&v)
            }
        }

        impl<'a> ::exonum::encoding::Field<'a> for $name {
            fn field_size() -> ::exonum::encoding::Offset {
                8
            }

            unsafe fn read(
                buffer: &'a [u8],
                from: ::exonum::encoding::Offset,
                to: ::exonum::encoding::Offset,
            ) -> $name {
                let data = <&[u8] as ::exonum::encoding::Field>::read(buffer, from, to);
                <$name as StorageValue>::from_bytes(data.into())
            }

            fn write(
                &self,
                buffer: &mut Vec<u8>,
                from: ::exonum::encoding::Offset,
                to: ::exonum::encoding::Offset,
            ) {
                <&[u8] as ::exonum::encoding::Field>::write(
                    &self.clone().into_bytes().as_slice(),
                    buffer,
                    from,
                    to,
                );
            }

            fn check(
                buffer: &[u8],
                from: ::exonum::encoding::CheckedOffset,
                to: ::exonum::encoding::CheckedOffset,
                latest_segment: ::exonum::encoding::CheckedOffset,
            ) -> ::exonum::encoding::Result {
                use exonum::encoding::Field;
                let latest_segment = <Vec<u8> as Field>::check(buffer, from, to, latest_segment)?;
                let buf: Vec<u8> = unsafe {
                    ::exonum::encoding::Field::read(
                        buffer,
                        from.unchecked_offset(),
                        to.unchecked_offset(),
                    )
                };
                let raw_tx: Result<RawBitcoinTx, ::exonum::encoding::Error> =
                    deserialize::<RawBitcoinTx>(buf.as_ref())
                        .map_err(|_| "Incorrect bitcoin transaction".into());

                if <$name as TxFromRaw>::from_raw(raw_tx?).is_some() {
                    Ok(latest_segment)
                } else {
                    Err("Incorrect bitcoin transaction".into())
                }
            }
        }

        implement_exonum_serializer! { $name }
    };
}

macro_rules! implement_tx_from_raw {
    ($name:ident) => {
        impl From<BitcoinTx> for $name {
            fn from(tx: BitcoinTx) -> $name {
                $name(tx.0)
            }
        }

        impl From<$name> for BitcoinTx {
            fn from(tx: $name) -> Self {
                BitcoinTx(tx.0)
            }
        }

        impl PartialEq<BitcoinTx> for $name {
            fn eq(&self, other: &BitcoinTx) -> bool {
                self.0.eq(other)
            }
        }

        impl AsRef<RawBitcoinTx> for $name {
            fn as_ref(&self) -> &RawBitcoinTx {
                &self.0
            }
        }
    };
}
