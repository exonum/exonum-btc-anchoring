use byteorder::{ByteOrder, LittleEndian};

use bitcoin::blockdata::script::{Script, Instruction, Builder};
use bitcoin::blockdata::opcodes::All;

use exonum::crypto::Hash;

use details::btc;

const PAYLOAD_VERSION: u8 = 1;
const PAYLOAD_LEN: u8 = 74;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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

impl Payload {
    pub fn from_script(script: Script) -> Option<Payload> {
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
                    if bytes.len() != PAYLOAD_LEN as usize {
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
        let kind = self.kind() as u8;
        // Serialize data
        let mut buf = vec![0; PAYLOAD_LEN as usize];
        buf[0] = version;
        buf[1] = kind;
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
    use exonum::crypto::hash;

    use details::btc;

    use super::{Payload, PayloadBuilder, PayloadKind};

    #[test]
    fn test_payload_regular() {
        let block_hash = hash(&[]);
        let payload_script = PayloadBuilder::new()
            .block_hash(block_hash)
            .block_height(1234)
            .into_script();

        let payload = Payload::from_script(payload_script).unwrap();
        assert_eq!(payload.block_hash, block_hash);
        assert_eq!(payload.block_height, 1234);
        assert_eq!(payload.prev_tx_chain, None);
        assert_eq!(payload.kind(), PayloadKind::Regular);
    }

    #[test]
    fn test_payload_recover() {
        let block_hash = hash(&[]);
        let prev_txid = btc::TxId::from_slice(block_hash.as_ref()).unwrap();
        let payload_script = PayloadBuilder::new()
            .block_hash(block_hash)
            .block_height(1234)
            .prev_tx_chain(prev_txid)
            .into_script();

        let payload = Payload::from_script(payload_script).unwrap();
        assert_eq!(payload.block_hash, block_hash);
        assert_eq!(payload.block_height, 1234);
        assert_eq!(payload.prev_tx_chain, Some(prev_txid));
        assert_eq!(payload.kind(), PayloadKind::Recover);
    }
}