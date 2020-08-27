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

pub use binary_map::BinaryMap;

use anyhow::anyhow;
use exonum::{
    crypto::{proto::*, Hash, PublicKey},
    merkledb::{
        impl_object_hash_for_binary_value, impl_serde_hex_for_binary_value, BinaryKey, BinaryValue,
        ObjectHash,
    },
};
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use protobuf::Message;
use serde_derive::{Deserialize, Serialize};

use std::borrow::Cow;

use crate::btc;

mod binary_map;

include!(concat!(env!("OUT_DIR"), "/protobuf_mod.rs"));

impl ProtobufConvert for btc::PublicKey {
    type ProtoStruct = btc_types::PublicKey;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut proto_struct = Self::ProtoStruct::default();
        self.0.write_into(&mut proto_struct.data);
        proto_struct
    }

    fn from_pb(pb: Self::ProtoStruct) -> anyhow::Result<Self> {
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

    fn from_pb(pb: Self::ProtoStruct) -> anyhow::Result<Self> {
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

    fn from_pb(pb: Self::ProtoStruct) -> anyhow::Result<Self> {
        let bytes = pb.get_data().to_vec();
        Ok(Self(btc_transaction_utils::InputSignature::from_bytes(
            bytes,
        )?))
    }
}

impl ProtobufConvert for btc::Sha256d {
    type ProtoStruct = btc_types::Sha256d;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut proto_struct = Self::ProtoStruct::default();
        proto_struct.data.extend(&self.0[..]);
        proto_struct
    }

    fn from_pb(pb: Self::ProtoStruct) -> anyhow::Result<Self> {
        use bitcoin_hashes::{sha256d, Hash};
        sha256d::Hash::from_slice(pb.get_data())
            .map(Self::from)
            .map_err(From::from)
    }
}

/// Public keys of an anchoring node.
#[derive(
    Serialize, Deserialize, Debug, Clone, PartialEq, ProtobufConvert, BinaryValue, ObjectHash,
)]
#[protobuf_convert(source = "self::service::AnchoringKeys")]
pub struct AnchoringKeys {
    /// Service key is used to authorize transactions.
    pub service_key: PublicKey,
    /// The Bitcoin public key is used to calculate the corresponding redeem script.
    pub bitcoin_key: btc::PublicKey,
}

/// Exonum message with a signature for one of the inputs of a new anchoring transaction.
#[derive(Debug, Clone, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "self::service::SignInput")]
pub struct SignInput {
    /// Proposal transaction id.
    pub txid: Sha256d,
    /// Signed input.
    pub input: u32,
    /// Signature content.
    pub input_signature: btc::InputSignature,
}

/// Exonum message with the unspent funding transaction.
#[derive(Debug, Clone, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "self::service::AddFunds")]
pub struct AddFunds {
    /// Transaction content.
    pub transaction: btc::Transaction,
}

/// Consensus parameters in the BTC anchoring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, BinaryValue, ObjectHash)]
pub struct Config {
    /// Type of the used BTC network.
    pub network: bitcoin::Network,
    /// Bitcoin public keys of nodes from from which the current anchoring redeem script can be calculated.
    pub anchoring_keys: Vec<AnchoringKeys>,
    /// Interval in blocks between anchored blocks.
    pub anchoring_interval: u64,
    /// Fee per byte in satoshis.
    pub transaction_fee: u64,
}

impl ProtobufConvert for Config {
    type ProtoStruct = self::service::Config;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut proto_struct = Self::ProtoStruct::default();

        proto_struct.set_network(self.network.magic());
        proto_struct.set_anchoring_keys(self.anchoring_keys.to_pb().into());
        proto_struct.set_anchoring_interval(self.anchoring_interval.to_pb());
        proto_struct.set_transaction_fee(self.transaction_fee.to_pb());
        proto_struct
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> anyhow::Result<Self> {
        let network = bitcoin::Network::from_magic(pb.get_network())
            .ok_or_else(|| anyhow!("Unknown Bitcoin network"))?;

        Ok(Self {
            network,
            anchoring_keys: ProtobufConvert::from_pb(pb.take_anchoring_keys().into_vec())?,
            anchoring_interval: ProtobufConvert::from_pb(pb.get_anchoring_interval())?,
            transaction_fee: ProtobufConvert::from_pb(pb.get_transaction_fee())?,
        })
    }
}

impl_serde_hex_for_binary_value! { SignInput }

impl BinaryValue for btc::Sha256d {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_pb()
            .write_to_bytes()
            .expect("Error while serializing value")
    }

    fn from_bytes(bytes: Cow<[u8]>) -> anyhow::Result<Self> {
        let mut pb = btc_types::Sha256d::new();
        pb.merge_from_bytes(bytes.as_ref())?;
        Self::from_pb(pb)
    }
}

impl BinaryKey for btc::Sha256d {
    fn size(&self) -> usize {
        Self::LEN
    }

    fn write(&self, buffer: &mut [u8]) -> usize {
        buffer.copy_from_slice(&self.0[..]);
        self.size()
    }

    fn read(buffer: &[u8]) -> Self::Owned {
        Self::from_slice(buffer).unwrap()
    }
}

// TODO Fix kind of input for these macro [ECR-3222]
use btc::Sha256d;
impl_object_hash_for_binary_value! { Sha256d }
