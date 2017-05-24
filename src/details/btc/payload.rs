
use byteorder::{ByteOrder, LittleEndian};

use bitcoin::blockdata::script::{Builder, Instruction, Script};
use bitcoin::blockdata::opcodes::All;

use exonum::crypto::Hash;

use details::btc;

const PAYLOAD_PREFIX: &'static [u8] = b"EXONUM";
const PAYLOAD_HEADER_LEN: usize = 8;
const PAYLOAD_V1: u8 = 1;
const PAYLOAD_V1_KIND_REGULAR: u8 = 0;
const PAYLOAD_V1_KIND_RECOVER: u8 = 1;

/// Anchoring transaction payload
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Payload {
    /// Anchored block height
    pub block_height: u64,
    /// Anchored block hash
    pub block_hash: Hash,
    /// `Txid` of previous transactions chain if it have been lost.
    pub prev_tx_chain: Option<btc::TxId>,
}

enum PayloadV1 {
    Regular(u64, Hash),
    Recover(u64, Hash, btc::TxId),
}

#[derive(Default)]
pub struct PayloadV1Builder {
    block_hash: Option<Hash>,
    block_height: Option<u64>,
    prev_tx_chain: Option<btc::TxId>,
}

pub type PayloadBuilder = PayloadV1Builder;

#[cfg_attr(feature = "cargo-clippy", allow(len_without_is_empty))]
impl PayloadV1 {
    fn read(bytes: &[u8]) -> Option<PayloadV1> {
        let kind = bytes[0];
        let data = &bytes[1..];
        match kind {
            PAYLOAD_V1_KIND_REGULAR => {
                if data.len() != 40 {
                    return None;
                }

                let block_height = LittleEndian::read_u64(&data[0..8]);
                let block_hash = Hash::from_slice(&data[8..40]).unwrap();
                Some(PayloadV1::Regular(block_height, block_hash))
            }
            PAYLOAD_V1_KIND_RECOVER => {
                if data.len() != 72 {
                    return None;
                }

                let block_height = LittleEndian::read_u64(&data[0..8]);
                let block_hash = Hash::from_slice(&data[8..40]).unwrap();
                let txid = btc::TxId::from_slice(&data[40..72]).unwrap();
                Some(PayloadV1::Recover(block_height, block_hash, txid))
            }
            _ => None,
        }
    }

    fn write(&self, buf: &mut [u8]) {
        let kind = self.kind();
        buf[0] = kind as u8;

        let mut buf = &mut buf[1..];
        debug_assert_eq!(buf.len(), self.len());
        // Serialize data
        match *self {
            PayloadV1::Regular(height, hash) => {
                LittleEndian::write_u64(&mut buf[0..8], height);
                buf[8..40].copy_from_slice(hash.as_ref());
            }
            PayloadV1::Recover(height, hash, txid) => {
                LittleEndian::write_u64(&mut buf[0..8], height);
                buf[8..40].copy_from_slice(hash.as_ref());
                buf[40..72].copy_from_slice(txid.as_ref());
            }
        };
    }

    fn len(&self) -> usize {
        match *self {
            PayloadV1::Regular(..) => 40,
            PayloadV1::Recover(..) => 72,
        }
    }

    fn kind(&self) -> u8 {
        match *self {
            PayloadV1::Regular(..) => PAYLOAD_V1_KIND_REGULAR,
            PayloadV1::Recover(..) => PAYLOAD_V1_KIND_RECOVER,
        }
    }

    fn into_script(self) -> Script {
        let len = self.len() + PAYLOAD_HEADER_LEN;
        let mut buf = vec![0; len];
        // Serialize header
        buf[0..6].copy_from_slice(PAYLOAD_PREFIX);
        buf[6] = PAYLOAD_V1;
        self.write(&mut buf[7..]);
        // Build script
        Builder::new()
            .push_opcode(All::OP_RETURN)
            .push_slice(buf.as_ref())
            .into_script()
    }
}

impl PayloadV1Builder {
    pub fn new() -> PayloadV1Builder {
        PayloadV1Builder {
            block_hash: None,
            block_height: None,
            prev_tx_chain: None,
        }
    }

    pub fn block_height(mut self, height: u64) -> PayloadV1Builder {
        self.block_height = Some(height);
        self
    }

    pub fn block_hash(mut self, hash: Hash) -> PayloadV1Builder {
        self.block_hash = Some(hash);
        self
    }

    pub fn prev_tx_chain(mut self, txid: Option<btc::TxId>) -> PayloadV1Builder {
        self.prev_tx_chain = txid;
        self
    }

    pub fn into_script(self) -> Script {
        let block_height = self.block_height.expect("Block height is not set");
        let block_hash = self.block_hash.expect("Block hash is not set");

        let payload = match self.prev_tx_chain {
            Some(txid) => PayloadV1::Recover(block_height, block_hash, txid),
            None => PayloadV1::Regular(block_height, block_hash),
        };
        payload.into_script()
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
                    if &bytes[0..6] != PAYLOAD_PREFIX {
                        return None;
                    }
                    // Parse metadata
                    let version = bytes[6];
                    match version {
                        PAYLOAD_V1 => PayloadV1::read(&bytes[7..]).map(Payload::from),
                        _ => None,
                    }
                } else {
                    None
                }
            })
    }
}

impl From<PayloadV1> for Payload {
    fn from(v1: PayloadV1) -> Payload {
        match v1 {
            PayloadV1::Regular(height, hash) => {
                Payload {
                    block_height: height,
                    block_hash: hash,
                    prev_tx_chain: None,
                }
            }
            PayloadV1::Recover(height, hash, txid) => {
                Payload {
                    block_height: height,
                    block_hash: hash,
                    prev_tx_chain: Some(txid),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::blockdata::script::Script;

    use exonum::crypto::hash;

    use details::btc;
    use details::btc::HexValueEx;

    use super::{Payload, PayloadBuilder};

    #[test]
    fn test_payload_regular_serialize() {
        let block_hash = hash(&[]);
        let payload_script = PayloadBuilder::new()
            .block_hash(block_hash)
            .block_height(1234)
            .into_script();

        assert_eq!(payload_script.to_hex(),
                   "6a3045584f4e554d0100d204000000000000e3b0c44298fc1c149afbf4c8996fb92427ae41e4649\
                   b934ca495991b7852b855");
    }

    #[test]
    fn test_payload_regular_deserialize() {
        let payload_script = Script::from_hex("6a3045584f4e554d0100d204000000000000e3b0c44298fc1c14\
                                               9afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                .unwrap();

        let block_hash = hash(&[]);
        let payload = Payload::from_script(&payload_script).unwrap();
        assert_eq!(payload.block_hash, block_hash);
        assert_eq!(payload.block_height, 1234);
        assert_eq!(payload.prev_tx_chain, None);
    }

    #[test]
    fn test_payload_recover_serizalize() {
        let block_hash = hash(&[]);
        let prev_txid = btc::TxId::from_slice(block_hash.as_ref()).unwrap();
        let payload_script = PayloadBuilder::new()
            .block_hash(block_hash)
            .block_height(1234)
            .prev_tx_chain(Some(prev_txid))
            .into_script();

        assert_eq!(payload_script.to_hex(),
                   "6a4c5045584f4e554d0101d204000000000000e3b0c44298fc1c149afbf4c8996fb92427ae41e46\
                   49b934ca495991b7852b855e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7\
                   852b855");
    }

    #[test]
    fn test_payload_recover_deserialize() {
        let payload_script = Script::from_hex("6a4c5045584f4e554d0101d204000000000000e3b0c44298fc1c\
                                               149afbf4c8996fb92427ae41e4649b934ca495991b7852b855e3\
                                               b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca49599\
                                               1b7852b855")
                .unwrap();

        let block_hash = hash(&[]);
        let prev_txid = btc::TxId::from_slice(block_hash.as_ref()).unwrap();
        let payload = Payload::from_script(&payload_script).unwrap();
        assert_eq!(payload.block_hash, block_hash);
        assert_eq!(payload.block_height, 1234);
        assert_eq!(payload.prev_tx_chain, Some(prev_txid));
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
