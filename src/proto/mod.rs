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

use bitcoin;
use btc_transaction_utils;
use exonum::{
    crypto::{Hash, PublicKey},
    impl_serde_hex_for_binary_value,
    merkledb::{BinaryValue, ObjectHash},
    proto::{schema::helpers, ProtobufConvert},
};
use exonum_derive::ProtobufConvert;
use failure;
use serde_derive::{Deserialize, Serialize};

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

/// Public keys of an anchoring node.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "self::service::AnchoringKeys")]
pub struct AnchoringKeys {
    /// Service key is used to authorize transactions.
    pub service_key: PublicKey,
    /// The Bitcoin public key is used to calculate the corresponding redeem script.
    pub bitcoin_key: btc::PublicKey,
}

/// Exonum message with a signature for one of the inputs of a new anchoring transaction.
#[derive(Debug, Clone, PartialEq, ProtobufConvert)]
#[exonum(pb = "self::service::SignInput")]
pub struct SignInput {
    /// Signed Bitcoin anchoring transaction.
    pub transaction: btc::Transaction,
    /// Signed input.
    pub input: u32,
    /// Signature content.
    pub input_signature: btc::InputSignature,
}

/// Consensus parameters in the BTC anchoring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// Type of the used BTC network.
    pub network: bitcoin::Network,
    /// Bitcoin public keys of nodes from from which the current anchoring redeem script can be calculated.
    pub anchoring_keys: Vec<AnchoringKeys>,
    /// Interval in blocks between anchored blocks.
    pub anchoring_interval: u64,
    /// Fee per byte in satoshis.
    pub transaction_fee: u64,
    // TODO remove funding transaction from the config. [ECR-3603]
    /// Funding transaction.
    pub funding_transaction: Option<btc::Transaction>,
}

impl ProtobufConvert for Config {
    type ProtoStruct = self::service::Config;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut proto_struct = Self::ProtoStruct::default();

        proto_struct.set_network(self.network.magic());
        proto_struct.set_anchoring_keys(self.anchoring_keys.to_pb().into());
        proto_struct.set_anchoring_interval(self.anchoring_interval.to_pb());
        proto_struct.set_transaction_fee(self.transaction_fee.to_pb());
        if let Some(tx) = self.funding_transaction.as_ref() {
            proto_struct.set_funding_transaction(tx.to_pb())
        }

        proto_struct
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let network = bitcoin::Network::from_magic(pb.get_network())
            .ok_or_else(|| failure::format_err!("Unknown Bitcoin network"))?;
        let funding_transaction = {
            let tx = pb.get_funding_transaction().to_owned();
            if tx.get_data().is_empty() {
                None
            } else {
                Some(btc::Transaction::from_pb(tx)?)
            }
        };

        Ok(Self {
            network,
            funding_transaction,
            anchoring_keys: ProtobufConvert::from_pb(pb.get_anchoring_keys().to_owned())?,
            anchoring_interval: ProtobufConvert::from_pb(pb.get_anchoring_interval())?,
            transaction_fee: ProtobufConvert::from_pb(pb.get_transaction_fee())?,
        })
    }
}

impl BinaryValue for Config {
    fn to_bytes(&self) -> Vec<u8> {
        use protobuf::Message;
        self.to_pb()
            .write_to_bytes()
            .expect("Error while serializing value")
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Result<Self, failure::Error> {
        use protobuf::Message;
        let mut pb = <Self as ProtobufConvert>::ProtoStruct::new();
        pb.merge_from_bytes(bytes.as_ref())?;
        Self::from_pb(pb)
    }
}

impl ObjectHash for Config {
    fn object_hash(&self) -> Hash {
        self.to_bytes().object_hash()
    }
}

impl_serde_hex_for_binary_value! { SignInput }
