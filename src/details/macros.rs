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
    ($from:ident, $to:ident) => (
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
    )
}

macro_rules! implement_base58_wrapper {
    ($from:ident, $to:ident) => (
        impl ToBase58 for $to {
            fn base58_layout(&self) -> Vec<u8> {
                self.0.base58_layout()
            }
        }

        impl FromBase58 for $to {
            fn from_base58_layout(data: Vec<u8>) -> Result<$to, FromBase58Error> {
                $from::from_base58_layout(data).map($to)
            }
        }

        impl ::std::str::FromStr for $to {
            type Err = ::bitcoin::util::base58::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                $to::from_base58check(s)
            }
        }

        impl ::std::string::ToString for $to {
            fn to_string(&self) -> String {
                self.to_base58check()
            }
        }

        impl fmt::Debug for $to {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "\"{}({})\"", stringify!($to), self.to_base58check())
            }
        }
    )
}

macro_rules! implement_serde_hex {
($name:ident) => (
    impl ::std::str::FromStr for $name {
        type Err = ::exonum::encoding::serialize::FromHexError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            $name::from_hex(s)
        }
    }

    impl ::std::string::ToString for $name {
        fn to_string(&self) -> String {
            self.to_hex()
        }
    }

    impl ::serde::Serialize for $name {
        fn serialize<S>(&self, ser: S) -> ::std::result::Result<S::Ok, S::Error>
            where S: ::serde::Serializer
        {
            ser.serialize_str(&self.to_hex())
        }
    }

    impl<'de> ::serde::Deserialize<'de> for $name {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: ::serde::Deserializer<'de>
        {
            struct HexVisitor;

            impl<'v> ::serde::de::Visitor<'v> for HexVisitor {
                type Value = $name;

                fn expecting(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                    write!(fmt, "Expected hex represented string")
                }

                fn visit_str<E>(self, hex: &str) -> Result<$name, E>
                    where E: ::serde::de::Error
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
)
}

macro_rules! implement_serde_base58check {
($name:ident) => (
    impl ::serde::Serialize for $name {
        fn serialize<S>(&self, ser: S) -> ::std::result::Result<S::Ok, S::Error>
            where S: ::serde::Serializer
        {
            ser.serialize_str(&self.to_base58check())
        }
    }

    impl<'de> ::serde::Deserialize<'de> for $name {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: ::serde::Deserializer<'de>
        {
            struct Base58Visitor;

            impl<'v> ::serde::de::Visitor<'v> for Base58Visitor {
                type Value = $name;

                fn expecting(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                    write!(fmt, "Expected base58 represented string")
                }

                fn visit_str<E>(self, hex: &str) -> Result<$name, E>
                    where E: ::serde::de::Error
                {
                    match $name::from_base58check(hex) {
                        Ok(value) => Ok(value),
                        Err(_) => Err(::serde::de::Error::custom("Wrong base58")),
                    }
                }
            }

            deserializer.deserialize_str(Base58Visitor)
        }
    }
)
}

macro_rules! implement_tx_wrapper {
($name:ident) => (
    implement_wrapper! {RawBitcoinTx, $name}

    impl $name {
        pub fn id(&self) -> TxId {
            TxId::from(self.0.bitcoin_hash())
        }

        pub fn nid(&self) -> TxId {
            TxId::from(self.0.ntxid())
        }

        pub fn txid(&self) -> String {
            self.0.bitcoin_hash().be_hex_string()
        }

        pub fn ntxid(&self) -> String {
            self.0.ntxid().be_hex_string()
        }
    }

    impl HexValue for $name  {
        fn to_hex(&self) -> String {
            serialize_hex(&self.0).unwrap()
        }
        fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError> {
            let bytes = Vec::<u8>::from_hex(v.as_ref())?;
            if let Ok(tx) = deserialize::<RawBitcoinTx>(bytes.as_ref()) {
                Ok($name::from(tx))
            } else {
                Err(FromHexError::InvalidHexLength)
            }
        }
    }

    impl StorageValue for $name {
        fn into_bytes(self) -> Vec<u8> {
            let mut v = Vec::new();
            v.extend(serialize(&self.0).unwrap());
            v
        }

        fn from_bytes(v: ::std::borrow::Cow<[u8]>) -> Self {
            let tx = deserialize::<RawBitcoinTx>(v.as_ref()).unwrap();
            $name::from(tx)
        }

        fn hash(&self) -> Hash {
            let mut v = Vec::new();
            v.extend(serialize(&self.0).unwrap());
            hash(&v)
        }
    }

    impl<'a> ::exonum::encoding::Field<'a> for $name {
        fn field_size() -> ::exonum::encoding::Offset {
            8
        }

        unsafe fn read(buffer: &'a [u8],
                       from: ::exonum::encoding::Offset,
                       to: ::exonum::encoding::Offset)
            -> $name {
            let data = <&[u8] as ::exonum::encoding::Field>::read(buffer, from, to);
            <$name as StorageValue>::from_bytes(data.into())
        }

        fn write(&self,
                 buffer: &mut Vec<u8>,
                 from: ::exonum::encoding::Offset,
                 to: ::exonum::encoding::Offset) {
            <&[u8] as ::exonum::encoding::Field>::write(&self.clone().into_bytes().as_slice(),
                                                             buffer,
                                                             from,
                                                             to);
        }

        fn check(buffer: &[u8],
                 from: ::exonum::encoding::CheckedOffset,
                 to: ::exonum::encoding::CheckedOffset,
                 latest_segment: ::exonum::encoding::CheckedOffset )
            -> ::exonum::encoding::Result {
            use ::exonum::encoding::Field;
            let latest_segment = <Vec<u8> as Field>::check(buffer, from, to, latest_segment)?;
            let buf: Vec<u8> = unsafe {
                ::exonum::encoding::Field::read(buffer,
                                                     from.unchecked_offset(),
                                                     to.unchecked_offset())
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
)
}

macro_rules! implement_tx_from_raw {
($name:ident) => (
    impl From<BitcoinTx> for $name {
        fn from(tx: BitcoinTx) -> $name {
            $name(tx.0)
        }
    }

    impl Into<BitcoinTx> for $name {
        fn into(self) -> BitcoinTx {
            BitcoinTx(self.0)
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
)
}
