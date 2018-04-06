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

use bitcoin::blockdata::opcodes::All;
use bitcoin::blockdata::script::{Builder, Script};
use bitcoin::blockdata::script::Instruction;
use bitcoin::network::constants::Network;
use bitcoin::util::address::{Address as RawAddress, Payload as AddressPayload};
use bitcoin::util::base58;
use secp256k1::key::PublicKey as RawPublicKey;
use secp256k1::Secp256k1;

use super::{Address, PublicKey, RedeemScript};

// TODO implement errors

impl RedeemScript {
    pub fn from_pubkeys<'a, I>(pubkeys: I, majority_count: u8) -> RedeemScript
    where
        I: IntoIterator<Item = &'a PublicKey>,
    {
        let mut builder = Builder::new().push_int(i64::from(majority_count));
        let mut total_count = 0;

        for pubkey in pubkeys {
            let bytes = pubkey.serialize();
            builder = builder.push_slice(bytes.as_ref());
            total_count += 1;
        }

        let script = builder
            .push_int(total_count)
            .push_opcode(All::OP_CHECKMULTISIG)
            .into_script();
        RedeemScript(script)
    }

    pub fn from_addresses<'a, I>(addrs: I, majority_count: u8) -> RedeemScript
    where
        I: Iterator<Item = &'a String>,
    {
        let mut builder = Builder::new().push_int(i64::from(majority_count));
        let mut total_count = 0;
        for addr in addrs {
            let bytes = base58::from_check(addr).unwrap();
            builder = builder.push_slice(bytes.as_slice());
            total_count += 1;
        }

        let script = builder
            .push_int(total_count)
            .push_opcode(All::OP_CHECKMULTISIG)
            .into_script();
        RedeemScript(script)
    }

    pub fn to_address(&self, network: Network) -> Address {
        RawAddress::p2sh(self, network).into()
    }

    pub fn compressed(&self, network: Network) -> RedeemScript {
        let mut builder = Builder::new();
        let context = Secp256k1::without_caps();

        for instruction in &self.0 {
            match instruction {
                Instruction::PushBytes(bytes) => {
                    if bytes.len() == 33 {
                        builder = builder.push_slice(bytes);
                    } else {
                        let pubkey = RawPublicKey::from_slice(&context, bytes).unwrap();
                        let addr = RawAddress::p2pk(&pubkey, network);
                        if let AddressPayload::PubkeyHash(ref hash) = addr.payload {
                            builder = builder.push_slice(hash[..].as_ref());
                        } else {
                            panic!("Wrong address type");
                        }
                    }
                }
                Instruction::Op(opcode) => builder = builder.push_opcode(opcode),
                Instruction::Error(_) => unimplemented!(),
            }
        }
        RedeemScript(builder.into_script())
    }

    pub fn script_pubkey(&self, network: Network) -> Script {
        self.to_address(network).script_pubkey()
    }
}
