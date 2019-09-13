// Copyright 2019 The Exonum Team
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

//! Module of the rust-protobuf generated files.

pub use self::service::TxSignature;

use bitcoin;
use btc_transaction_utils;
use exonum::proto::ProtobufConvert;
use failure;

use crate::btc;

include!(concat!(env!("OUT_DIR"), "/protobuf_mod.rs"));

impl ProtobufConvert for btc::PublicKey {
    type ProtoStruct = btc_types::PublicKey;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut proto_struct = Self::ProtoStruct::default();
        self.0.write_into(&mut proto_struct.data);
        proto_struct
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let bytes = pb.get_data();
        Ok(Self(bitcoin::PublicKey::from_slice(bytes)?))
    }
}

impl ProtobufConvert for btc::Transaction {
    type ProtoStruct = btc_types::Transaction;

    fn to_pb(&self) -> Self::ProtoStruct {
        let bytes = bitcoin::consensus::serialize(&self.0);
        let mut proto_struct = Self::ProtoStruct::default();
        proto_struct.set_data(bytes);
        proto_struct
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let bytes = pb.get_data();
        Ok(Self(bitcoin::consensus::deserialize(bytes)?))
    }
}

impl ProtobufConvert for btc::InputSignature {
    type ProtoStruct = btc_types::InputSignature;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut proto_struct = Self::ProtoStruct::default();
        proto_struct.set_data(self.0.as_ref().to_vec());
        proto_struct
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let bytes = pb.get_data().to_vec();
        Ok(Self(btc_transaction_utils::InputSignature::from_bytes(
            bytes,
        )?))
    }
}
