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

//! Module of the rust-protobuf generated files.

// For rust-protobuf generated files.
#![allow(bare_trait_objects)]
#![allow(renamed_and_removed_lints)]

pub use self::btc_anchoring::TxSignature;

use bitcoin;
use btc_transaction_utils;
use secp256k1::Secp256k1;

use exonum::encoding::protobuf::ProtobufConvert;

use btc;

include!(concat!(env!("OUT_DIR"), "/protobuf_mod.rs"));

impl ProtobufConvert for btc::Transaction {
    type ProtoStruct = btc_anchoring::BtcTransaction;

    fn to_pb(&self) -> Self::ProtoStruct {
        let bytes = bitcoin::consensus::serialize(&self.0);
        let mut proto_struct = Self::ProtoStruct::default();
        proto_struct.set_data(bytes);
        proto_struct
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
        let bytes = pb.get_data();
        Ok(btc::Transaction(
            bitcoin::consensus::deserialize(bytes).map_err(drop)?,
        ))
    }
}

impl ProtobufConvert for btc::InputSignature {
    type ProtoStruct = btc_anchoring::InputSignature;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut proto_struct = Self::ProtoStruct::default();
        proto_struct.set_data(self.0.as_ref().to_vec());
        proto_struct
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, ()> {
        let bytes = pb.get_data().to_vec();
        let context = Secp256k1::without_caps();
        Ok(btc::InputSignature(
            btc_transaction_utils::InputSignature::from_bytes(&context, bytes).map_err(drop)?,
        ))
    }
}
