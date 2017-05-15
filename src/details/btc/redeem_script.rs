use bitcoin::blockdata::script::{Builder, Script};
use bitcoin::blockdata::opcodes::All;
use bitcoin::blockdata::script::Instruction;
use bitcoin::util::base58::{FromBase58, ToBase58};
use bitcoin::util::address::Address;
use bitcoin::network::constants::Network;
use secp256k1::key::PublicKey as RawPublicKey;
use secp256k1::Secp256k1;

use super::{PublicKey, RedeemScript};

// TODO implement errors

impl RedeemScript {
    pub fn from_pubkeys<'a, I>(pubkeys: I, majority_count: u8) -> RedeemScript
        where I: IntoIterator<Item = &'a PublicKey>
    {
        let mut builder = Builder::new().push_int(majority_count as i64);
        let mut total_count = 0;

        let context = Secp256k1::without_caps();
        for pubkey in pubkeys {
            let bytes = pubkey.serialize_vec(&context, true);
            builder = builder.push_slice(bytes.as_slice());
            total_count += 1;
        }

        let script = builder
            .push_int(total_count)
            .push_opcode(All::OP_CHECKMULTISIG)
            .into_script();
        RedeemScript(script)
    }

    pub fn from_addresses<'a, I>(addrs: I, majority_count: u8) -> RedeemScript
        where I: Iterator<Item = &'a String>
    {
        let mut builder = Builder::new().push_int(majority_count as i64);
        let mut total_count = 0;
        for addr in addrs {
            let bytes = Vec::<u8>::from_base58check(addr).unwrap();
            builder = builder.push_slice(bytes.as_slice());
            total_count += 1;
        }

        let script = builder
            .push_int(total_count)
            .push_opcode(All::OP_CHECKMULTISIG)
            .into_script();
        RedeemScript(script)
    }

    pub fn to_address(&self, network: Network) -> String {
        let addr = Address::from_script(network, self);
        addr.to_base58check()
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
                        let addr = Address::from_key(network, &pubkey, true);
                        builder = builder.push_slice(addr.hash[..].as_ref());
                    }
                }
                Instruction::Op(opcode) => builder = builder.push_opcode(opcode),
                Instruction::Error(_) => unimplemented!(),
            }
        }
        RedeemScript(builder.into_script())
    }

    pub fn script_pubkey(&self, network: Network) -> Script {
        let addr = Address::from_script(network, self);
        addr.script_pubkey()
    }
}
