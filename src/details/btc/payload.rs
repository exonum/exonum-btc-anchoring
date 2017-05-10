use byteorder::{ByteOrder, LittleEndian};

use bitcoin::blockdata::script::{Script, Instruction, Builder};
use bitcoin::blockdata::opcodes::All;

use exonum::crypto::Hash;

use details::btc;

const PAYLOAD_VERSION: u8 = 1;
const PAYLOAD_HEADER_LEN: usize = 2;
const PAYLOAD_LEN_REGULAR: usize = 42;
const PAYLOAD_LEN_RECOVER: usize = 74;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum PayloadKind {
    Regular = 0,
    Recover = 1,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Payload {
    pub version: u8,
    pub block_height: u64,
    pub block_hash: Hash,
    pub prev_tx_chain: Option<btc::TxId>,
}

pub struct PayloadBuilder {
    block_hash: Option<Hash>,
    block_height: Option<u64>,
    prev_tx_chain: Option<btc::TxId>,
}

impl PayloadKind {
    pub fn len(&self) -> usize {
        match *self {
            PayloadKind::Regular => PAYLOAD_LEN_REGULAR,
            PayloadKind::Recover => PAYLOAD_LEN_RECOVER,
        }
    }
}

impl Payload {
    pub fn from_script(script: &Script) -> Option<Payload> {
        let mut instructions = script.into_iter();
        instructions
            .next()
            .and_then(|instr| if instr == Instruction::Op(All::OP_RETURN) {
                          instructions.next()
                      } else {
                          None
                      })
            .and_then(|instr| {
                if let Instruction::PushBytes(bytes) = instr {
                    if bytes.len() < PAYLOAD_HEADER_LEN {
                        return None;
                    }
                    // Parse metadata
                    let version = bytes[0];
                    if version != PAYLOAD_VERSION {
                        return None;
                    }
                    let kind = match bytes[1] {
                        0 => PayloadKind::Regular,
                        1 => PayloadKind::Recover,
                        _ => return None,
                    };
                    // Check body len
                    if bytes.len() != kind.len() {
                        return None;
                    }
                    // Get payload data
                    let block_height = LittleEndian::read_u64(&bytes[2..10]);
                    let block_hash = Hash::from_slice(&bytes[10..42]).unwrap();
                    let prev_tx_chain = if kind == PayloadKind::Recover {
                        let txid = btc::TxId::from_slice(&bytes[42..74]).unwrap();
                        Some(txid)
                    } else {
                        None
                    };

                    let payload = Payload {
                        version: PAYLOAD_VERSION,
                        block_hash: block_hash,
                        block_height: block_height,
                        prev_tx_chain: prev_tx_chain,
                    };
                    Some(payload)
                } else {
                    None
                }
            })
    }

    pub fn into_script(self) -> Script {
        let version = PAYLOAD_VERSION;
        let kind = self.kind();
        // Serialize data
        let mut buf = vec![0; kind.len()];
        buf[0] = version;
        buf[1] = kind as u8;
        LittleEndian::write_u64(&mut buf[2..10], self.block_height);
        buf[10..42].copy_from_slice(self.block_hash.as_ref());
        if let Some(prev_tx) = self.prev_tx_chain {
            buf[42..74].copy_from_slice(prev_tx.as_ref());
        }
        // Build script
        Builder::new()
            .push_opcode(All::OP_RETURN)
            .push_slice(buf.as_ref())
            .into_script()
    }

    pub fn kind(&self) -> PayloadKind {
        if self.prev_tx_chain.is_some() {
            PayloadKind::Recover
        } else {
            PayloadKind::Regular
        }
    }
}

impl PayloadBuilder {
    pub fn new() -> PayloadBuilder {
        PayloadBuilder {
            block_hash: None,
            block_height: None,
            prev_tx_chain: None,
        }
    }

    pub fn block_height(mut self, height: u64) -> PayloadBuilder {
        self.block_height = Some(height);
        self
    }

    pub fn block_hash(mut self, hash: Hash) -> PayloadBuilder {
        self.block_hash = Some(hash);
        self
    }

    pub fn prev_tx_chain(mut self, txid: btc::TxId) -> PayloadBuilder {
        self.prev_tx_chain = Some(txid);
        self
    }

    pub fn into_payload(self) -> Payload {
        Payload {
            version: PAYLOAD_VERSION,
            block_height: self.block_height.expect("Block height is not set"),
            block_hash: self.block_hash.expect("Block hash is not set"),
            prev_tx_chain: self.prev_tx_chain,
        }
    }

    pub fn into_script(self) -> Script {
        self.into_payload().into_script()
    }
}


#[cfg(test)]
mod tests {
    use bitcoin::blockdata::script::Script;

    use exonum::crypto::hash;

    use details::btc;
    use details::btc::HexValueEx;

    use super::{Payload, PayloadBuilder, PayloadKind};

    #[test]
    fn test_payload_regular_serialize() {
        let block_hash = hash(&[]);
        let payload_script = PayloadBuilder::new()
            .block_hash(block_hash)
            .block_height(1234)
            .into_script();

        assert_eq!(payload_script.to_hex(),
                   "6a2a0100d204000000000000e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991\
                    b7852b855");
    }

    #[test]
    fn test_payload_regular_deserialize() {
        let payload_script = Script::from_hex("6a2a0100d204000000000000e3b0c44298fc1c149afbf4c8996f\
                                               b92427ae41e4649b934ca495991b7852b855")
                .unwrap();

        let block_hash = hash(&[]);
        let payload = Payload::from_script(&payload_script).unwrap();
        assert_eq!(payload.block_hash, block_hash);
        assert_eq!(payload.block_height, 1234);
        assert_eq!(payload.prev_tx_chain, None);
        assert_eq!(payload.kind(), PayloadKind::Regular);
    }

    #[test]
    fn test_payload_recover_serizalize() {
        let block_hash = hash(&[]);
        let prev_txid = btc::TxId::from_slice(block_hash.as_ref()).unwrap();
        let payload_script = PayloadBuilder::new()
            .block_hash(block_hash)
            .block_height(1234)
            .prev_tx_chain(prev_txid)
            .into_script();

        assert_eq!(payload_script.to_hex(),
                   "6a4a0101d204000000000000e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991\
                   b7852b855e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn test_payload_recover_deserialize() {
        let payload_script = Script::from_hex("6a4a0101d204000000000000e3b0c44298fc1c149afbf4c8996f\
                                               b92427ae41e4649b934ca495991b7852b855e3b0c44298fc1c14\
                                               9afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                .unwrap();

        let block_hash = hash(&[]);
        let prev_txid = btc::TxId::from_slice(block_hash.as_ref()).unwrap();
        let payload = Payload::from_script(&payload_script).unwrap();
        assert_eq!(payload.block_hash, block_hash);
        assert_eq!(payload.block_height, 1234);
        assert_eq!(payload.prev_tx_chain, Some(prev_txid));
        assert_eq!(payload.kind(), PayloadKind::Recover);
    }

    #[test]
    fn test_payload_incorrect_deserialize() {
        // Payload from old anchoring transaction
        let payload_script = Script::from_hex("6a2a0128f0b31a00000000008fb4879f1b7f332be1aee197f99f\
                                               7333c915570c6ad5c6eed641f33fe0199129")
                .unwrap();
        assert_eq!(Payload::from_script(&payload_script), None);
    }

    #[test]
    fn test_payload_non_op_return() {
        // Payload from old anchoring transaction
        let script_pubkey = Script::from_hex("a91472b7506704dc074fa46359251052e781d96f939a87")
            .unwrap();
        assert_eq!(Payload::from_script(&script_pubkey), None);
    }
}
