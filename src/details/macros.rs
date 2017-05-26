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

        impl fmt::Debug for $to {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "\"{}({})\"", stringify!($to), self.to_base58check())
            }
        }
    )
}

macro_rules! implement_serde_hex {
($name:ident) => (
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

        pub fn confirmations(&self, client: &AnchoringRpc)
             -> ::std::result::Result<Option<u64>, ::bitcoinrpc::Error> {
            let confirmations = client.get_transaction_info(&self.txid())?
                .and_then(|info| info.confirmations);
            Ok(confirmations)
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
        fn serialize(self) -> Vec<u8> {
            let mut v = vec![];
            v.extend(serialize(&self.0).unwrap());
            v
        }

        fn deserialize(v: Vec<u8>) -> Self {
            let tx = deserialize::<RawBitcoinTx>(v.as_ref()).unwrap();
            $name::from(tx)
        }

        fn hash(&self) -> Hash {
            let mut v = vec![];
            v.extend(serialize(&self.0).unwrap());
            hash(&v)
        }
    }

    impl<'a> ::exonum::messages::Field<'a> for $name {
        fn field_size() -> usize {
            8
        }

        fn read(buffer: &'a [u8], from: usize, to: usize) -> $name {
            let data = <&[u8] as ::exonum::messages::Field>::read(buffer, from, to);
            <$name as StorageValue>::deserialize(data.to_vec())
        }

        fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
            <&[u8] as ::exonum::messages::Field>::write(&self.clone().serialize().as_slice(),
            buffer, from, to);
        }

        fn check(buffer: &'a [u8], from: usize, to: usize) ->
            Result<(), ::exonum::messages::Error> {
            let buf: Vec<u8> = ::exonum::messages::Field::read(buffer, from, to);
            let raw_tx = deserialize::<RawBitcoinTx>(buf.as_ref())
                .map_err(|_| ::exonum::messages::Error::IncorrectMessageType { message_type: 1 })?;
            if <$name as TxFromRaw>::from_raw(raw_tx).is_some() {
                Ok(())
            } else {
                Err(::exonum::messages::Error::IncorrectMessageType { message_type: 2 })
            }
        }
    }

    impl<'a> ::exonum::serialize::json::ExonumJsonSerialize for $name {
        fn serialize<S: ::serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
            ser.serialize_str(&self.to_hex())
        }
    }

    impl ::exonum::serialize::json::ExonumJsonDeserializeField for $name {
        fn deserialize_field<B>(value: &::serde_json::Value,
                                buffer: &mut B,
                                from: usize,
                                to: usize)
                                -> Result<(), Box<::std::error::Error>>
            where B: ::exonum::serialize::json::WriteBufferWrapper
        {
            let tx = ::serde_json::from_value(value.clone())?;
            buffer.write(from, to, <$name as StorageValue>::serialize(tx));
            Ok(())
        }
    }
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
