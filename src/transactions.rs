use std::fmt;
use std::collections::HashMap;
use std::ops::Deref;

use byteorder::{ByteOrder, LittleEndian};
use bitcoin::blockdata::script::Instruction;
use bitcoin::blockdata::opcodes::All;
use bitcoin::util::hash::Hash160;
use bitcoin::network::serialize::{BitcoinHash, serialize_hex, deserialize, serialize};
use bitcoin::blockdata::transaction::{TxIn, TxOut};
use bitcoin::blockdata::script::{Script, Builder};
use bitcoin::util::base58::ToBase58;
use bitcoin::util::address::{Address, Privkey, Type};
use bitcoin::network::constants::Network;
use bitcoin::blockdata::transaction::SigHashType;
use secp256k1::key::{PublicKey, SecretKey};
use secp256k1::{Secp256k1, Message, Signature};
use bitcoinrpc;

use exonum::crypto::{hash, Hash, FromHexError, HexValue};
use exonum::node::Height;
use exonum::storage::StorageValue;

use client::{AnchoringRpc, RpcClient};
use btc;
use btc::{TxId, RedeemScript, HexValueEx};
use error::{RpcError, Error as ServiceError};

pub type RawBitcoinTx = ::bitcoin::blockdata::transaction::Transaction;

const ANCHORING_TX_FUNDS_OUTPUT: u32 = 0;
const ANCHORING_TX_DATA_OUTPUT: u32 = 1;
const ANCHORING_TX_PREV_CHAIN_OUTPUT: u32 = 2;

/// Anchoring transaction struct is strict:
/// - Zero input is previous anchoring tx or initial funding tx
/// - Zero output is next anchoring tx
/// - First output is anchored metadata
/// - Second output is optional and contains previous tx chain's tail
#[derive(Clone, PartialEq)]
pub struct AnchoringTx(pub RawBitcoinTx);
/// Funding transaction always has an output to `p2sh` address
#[derive(Clone, PartialEq)]
pub struct FundingTx(pub RawBitcoinTx);
/// Other unspecified Bitcoin transaction
#[derive(Debug, Clone, PartialEq)]
pub struct BitcoinTx(pub RawBitcoinTx);

#[derive(Debug, Clone, PartialEq)]
pub enum TxKind {
    Anchoring(AnchoringTx),
    FundingTx(FundingTx),
    Other(BitcoinTx),
}

pub struct TransactionBuilder {
    inputs: Vec<(RawBitcoinTx, u32)>,
    output: Option<btc::Address>,
    fee: Option<u64>,
    payload: Option<(u64, Hash)>,
    prev_tx_chain: Option<TxId>,
}

impl HexValueEx for RawBitcoinTx {
    fn to_hex(&self) -> String {
        serialize_hex(self).unwrap()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError> {
        let bytes = Vec::<u8>::from_hex(v.as_ref())?;
        if let Ok(tx) = deserialize(bytes.as_ref()) {
            Ok(tx)
        } else {
            Err(FromHexError::InvalidHexLength)
        }
    }
}

implement_tx_wrapper! {AnchoringTx}
implement_tx_wrapper! {FundingTx}
implement_tx_wrapper! {BitcoinTx}

implement_tx_from_raw! {AnchoringTx}
implement_tx_from_raw! {FundingTx}

impl FundingTx {
    pub fn create(client: &AnchoringRpc,
                  address: &btc::Address,
                  total_funds: u64)
                  -> Result<FundingTx, RpcError> {
        let tx = client.send_to_address(address, total_funds)?;
        Ok(FundingTx::from(tx))
    }

    pub fn find_out(&self, addr: &btc::Address) -> Option<u32> {
        let redeem_script_hash = addr.hash;
        self.0
            .output
            .iter()
            .position(|output| if let Some(Instruction::PushBytes(bytes)) =
                output.script_pubkey.into_iter().nth(1) {
                          Hash160::from(bytes) == redeem_script_hash
                      } else {
                          false
                      })
            .map(|x| x as u32)
    }

    pub fn is_unspent(&self,
                      client: &RpcClient,
                      addr: &btc::Address)
                      -> Result<Option<bitcoinrpc::UnspentTransactionInfo>, RpcError> {
        let txid = self.txid();
        let txs = client.listunspent(0, 9999999, [addr.to_base58check().as_ref()])?;
        Ok(txs.into_iter().find(|txinfo| txinfo.txid == txid))
    }
}

impl AnchoringTx {
    pub fn amount(&self) -> u64 {
        self.0.output[ANCHORING_TX_FUNDS_OUTPUT as usize].value
    }

    pub fn output_address(&self, network: Network) -> btc::Address {
        let script = &self.0.output[ANCHORING_TX_FUNDS_OUTPUT as usize].script_pubkey;
        let bytes = script
            .into_iter()
            .filter_map(|instruction| if let Instruction::PushBytes(bytes) = instruction {
                            Some(bytes)
                        } else {
                            None
                        })
            .next()
            .unwrap();

        Address {
                ty: Type::ScriptHash,
                network: network,
                hash: Hash160::from(bytes),
            }
            .into()
    }

    pub fn inputs(&self) -> ::std::ops::Range<u32> {
        0..self.0.input.len() as u32
    }

    pub fn payload(&self) -> (Height, Hash) {
        find_payload(&self.0).expect("Unable to find payload")
    }

    pub fn prev_tx_chain(&self) -> Option<TxId> {
        find_prev_txchain(self)
    }

    pub fn prev_hash(&self) -> TxId {
        TxId::from(self.0.input[0].prev_hash)
    }

    pub fn sign(&self,
                redeem_script: &btc::RedeemScript,
                input: u32,
                priv_key: &Privkey)
                -> btc::Signature {
        sign_input(self, input as usize, redeem_script, priv_key.secret_key())
    }

    pub fn verify(&self,
                  redeem_script: &RedeemScript,
                  input: u32,
                  pub_key: &PublicKey,
                  signature: &[u8])
                  -> bool {
        verify_input(self, input as usize, redeem_script, pub_key, signature)
    }

    pub fn finalize(self,
                    redeem_script: &btc::RedeemScript,
                    signatures: HashMap<u32, Vec<btc::Signature>>)
                    -> AnchoringTx {
        finalize_anchoring_transaction(self, redeem_script, signatures)
    }

    pub fn send(self,
                client: &AnchoringRpc,
                redeem_script: &btc::RedeemScript,
                signatures: HashMap<u32, Vec<btc::Signature>>)
                -> Result<AnchoringTx, RpcError> {
        let tx = self.finalize(redeem_script, signatures);
        client.send_transaction(tx.clone().into())?;
        Ok(tx)
    }
}

impl fmt::Debug for AnchoringTx {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = self.payload();
        f.debug_struct(stringify!(AnchoringTx))
            .field("txid", &self.txid())
            .field("txhex", &self.to_hex())
            .field("content", &self.0)
            .field("height", &payload.0)
            .field("hash", &payload.1.to_hex())
            .finish()
    }
}

impl fmt::Debug for FundingTx {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(stringify!(AnchoringTx))
            .field("txid", &self.txid())
            .field("txhex", &self.to_hex())
            .field("content", &self.0)
            .finish()
    }
}

impl From<RawBitcoinTx> for TxKind {
    fn from(tx: RawBitcoinTx) -> TxKind {
        if find_payload(&tx).is_some() {
            TxKind::Anchoring(AnchoringTx::from(tx))
        } else {
            // Find output with funds and p2sh script_pubkey
            for out in &tx.output {
                if out.value > 0 && out.script_pubkey.is_p2sh() {
                    return TxKind::FundingTx(FundingTx::from(tx.clone()));
                }
            }
            TxKind::Other(BitcoinTx::from(tx))
        }
    }
}

impl From<BitcoinTx> for TxKind {
    fn from(tx: BitcoinTx) -> TxKind {
        TxKind::from(tx.0)
    }
}

impl TransactionBuilder {
    pub fn with_prev_tx(prev_tx: &RawBitcoinTx, out: u32) -> TransactionBuilder {
        TransactionBuilder {
            inputs: vec![(prev_tx.clone(), out)],
            output: None,
            payload: None,
            fee: None,
            prev_tx_chain: None,
        }
    }

    pub fn fee(mut self, fee: u64) -> TransactionBuilder {
        self.fee = Some(fee);
        self
    }

    pub fn add_funds(mut self, tx: &RawBitcoinTx, out: u32) -> TransactionBuilder {
        self.inputs.push((tx.clone(), out));
        self
    }

    pub fn payload(mut self, height: u64, hash: Hash) -> TransactionBuilder {
        self.payload = Some((height, hash));
        self
    }

    pub fn send_to(mut self, addr: btc::Address) -> TransactionBuilder {
        self.output = Some(addr);
        self
    }

    pub fn prev_tx_chain(mut self, txid: Option<TxId>) -> TransactionBuilder {
        self.prev_tx_chain = txid;
        self
    }

    pub fn into_transaction(mut self) -> Result<AnchoringTx, ServiceError> {
        let available_funds: u64 = self.inputs
            .iter()
            .map(|&(ref tx, out)| tx.output[out as usize].value)
            .sum();

        let addr = self.output.take().expect("Output address is not set");
        let fee = self.fee.expect("Fee is not set");
        let (height, block_hash) = self.payload.take().expect("Payload is not set");
        if available_funds < fee {
            return Err(ServiceError::InsufficientFunds);
        }
        let total_funds = available_funds - fee;

        let tx = create_anchoring_transaction(addr,
                                              height,
                                              block_hash,
                                              self.inputs.iter(),
                                              total_funds,
                                              self.prev_tx_chain);
        Ok(tx)
    }
}

fn create_anchoring_transaction<'a, I>(addr: btc::Address,
                                       block_height: Height,
                                       block_hash: Hash,
                                       inputs: I,
                                       out_funds: u64,
                                       prev_chain_txid: Option<TxId>)
                                       -> AnchoringTx
    where I: Iterator<Item = &'a (RawBitcoinTx, u32)>
{
    let inputs = inputs
        .map(|&(ref unspent_tx, utxo_vout)| {
                 TxIn {
                     prev_hash: unspent_tx.bitcoin_hash(),
                     prev_index: utxo_vout,
                     script_sig: Script::new(),
                     sequence: 0xFFFFFFFF,
                 }
             })
        .collect::<Vec<_>>();

    let metadata_script = {
        let data = {
            let mut data = [0u8; 42];
            data[0] = 1; // version
            data[1] = 40; // data len
            LittleEndian::write_u64(&mut data[2..10], block_height);
            data[10..42].copy_from_slice(block_hash.as_ref());
            data
        };
        Builder::new()
            .push_opcode(All::OP_RETURN)
            .push_slice(data.as_ref())
            .into_script()
    };
    let mut outputs = vec![TxOut {
                               value: out_funds,
                               script_pubkey: addr.script_pubkey(),
                           },
                           TxOut {
                               value: 0,
                               script_pubkey: metadata_script,
                           }];

    if let Some(prev_chain_txid) = prev_chain_txid {
        let txout = TxOut {
            value: 0,
            script_pubkey: {
                let data = {
                    let mut data = [0u8; 34];
                    data[0] = 1; // version
                    data[1] = 32; // data len
                    data[2..34].copy_from_slice(prev_chain_txid.as_ref());
                    data
                };
                Builder::new()
                    .push_opcode(All::OP_RETURN)
                    .push_slice(data.as_ref())
                    .into_script()
            },
        };
        outputs.push(txout);
    }

    let tx = RawBitcoinTx {
        version: 1,
        lock_time: 0,
        input: inputs,
        output: outputs,
        witness: vec![],
    };
    AnchoringTx::from(tx)
}

pub fn sign_input(tx: &RawBitcoinTx,
                  input: usize,
                  subscript: &Script,
                  sec_key: &SecretKey)
                  -> Vec<u8> {
    let sighash = tx.signature_hash(input, subscript, SigHashType::All.as_u32());
    // Make signature
    let context = Secp256k1::new();
    let msg = Message::from_slice(&sighash[..]).unwrap();
    let sign = context.sign(&msg, sec_key).unwrap();
    // Serialize signature
    let mut sign_data = sign.serialize_der(&context);
    sign_data.push(SigHashType::All.as_u32() as u8);
    sign_data
}

pub fn verify_input(tx: &RawBitcoinTx,
                    input: usize,
                    subscript: &Script,
                    pub_key: &PublicKey,
                    signature: &[u8])
                    -> bool {
    // Do not verify signatures other than SigHashType::All
    if Some(&(SigHashType::All.as_u32() as u8)) != signature.last() {
        return false;
    }

    let sighash = tx.signature_hash(input, subscript, SigHashType::All.as_u32());
    let msg = Message::from_slice(&sighash[..]).unwrap();

    let context = Secp256k1::new();
    if let Ok(sign) = Signature::from_der_lax(&context, signature) {
        context.verify(&msg, &sign, pub_key).is_ok()
    } else {
        false
    }
}

fn finalize_anchoring_transaction(mut anchoring_tx: AnchoringTx,
                                  redeem_script: &btc::RedeemScript,
                                  signatures: HashMap<u32, Vec<btc::Signature>>)
                                  -> AnchoringTx {
    let redeem_script_bytes = redeem_script.0.clone().into_vec();
    // build scriptSig
    for (out, signatures) in signatures {
        anchoring_tx.0.input[out as usize].script_sig = {
            let mut builder = Builder::new();
            builder = builder.push_opcode(All::OP_PUSHBYTES_0);
            for sign in &signatures {
                builder = builder.push_slice(sign.as_ref());
            }
            builder
                .push_slice(redeem_script_bytes.as_ref())
                .into_script()
        };
    }
    anchoring_tx
}

fn find_payload(tx: &RawBitcoinTx) -> Option<(Height, Hash)> {
    tx.output
        .get(ANCHORING_TX_DATA_OUTPUT as usize)
        .and_then(|output| {
            output
                .script_pubkey
                .into_iter()
                .filter_map(|instruction| if let Instruction::PushBytes(bytes) = instruction {
                                Some(bytes)
                            } else {
                                None
                            })
                .next()
        })
        .and_then(|bytes| if bytes.len() == 42 && bytes[0] == 1 {
                      // TODO check len
                      let height = LittleEndian::read_u64(&bytes[2..10]);
                      let block_hash = Hash::from_slice(&bytes[10..42]).unwrap();
                      Some((height, block_hash))
                  } else {
                      None
                  })
}

fn find_prev_txchain(tx: &RawBitcoinTx) -> Option<TxId> {
    tx.output
        .get(ANCHORING_TX_PREV_CHAIN_OUTPUT as usize)
        .and_then(|output| {
            output
                .script_pubkey
                .into_iter()
                .filter_map(|instruction| if let Instruction::PushBytes(bytes) = instruction {
                                Some(bytes)
                            } else {
                                None
                            })
                .next()
        })
        .and_then(|bytes| if bytes.len() == 34 && bytes[0] == 1 {
                      // TODO check len
                      let prev_tx_id = TxId::from_slice(&bytes[2..34]).unwrap();
                      Some(prev_tx_id)
                  } else {
                      None
                  })
}

#[cfg(test)]
mod tests {
    extern crate blockchain_explorer;

    use std::collections::HashMap;

    use bitcoin::network::constants::Network;
    use bitcoin::util::base58::{FromBase58, ToBase58};
    use bitcoin::util::address::Privkey;
    use secp256k1::key::PublicKey as RawPublicKey;
    use secp256k1::Secp256k1;

    use exonum::crypto::{Hash, HexValue};

    use transactions::{BitcoinTx, AnchoringTx, FundingTx, TransactionBuilder, TxKind,
                       verify_input, sign_input};
    use btc::{RedeemScript, PublicKey, HexValueEx};
    use btc;

    #[test]
    fn test_redeem_script_from_pubkeys() {
        let redeem_script_hex = "5321027db7837e51888e94c094703030d162c682c8dba312210f44ff440fbd5e5c24732102bdd272891c9e4dfc3962b1fdffd5a59732019816f9db4833634dbdaf01a401a52103280883dc31ccaee34218819aaa245480c35a33acd91283586ff6d1284ed681e52103e2bc790a6e32bf5a766919ff55b1f9e9914e13aed84f502c0e4171976e19deb054ae";
        let keys = ["027db7837e51888e94c094703030d162c682c8dba312210f44ff440fbd5e5c2473",
                    "02bdd272891c9e4dfc3962b1fdffd5a59732019816f9db4833634dbdaf01a401a5",
                    "03280883dc31ccaee34218819aaa245480c35a33acd91283586ff6d1284ed681e5",
                    "03e2bc790a6e32bf5a766919ff55b1f9e9914e13aed84f502c0e4171976e19deb0"]
                .into_iter()
                .map(|x| PublicKey::from_hex(x).unwrap())
                .collect::<Vec<_>>();

        let redeem_script = RedeemScript::from_pubkeys(&keys, 3);
        assert_eq!(redeem_script.to_hex(), redeem_script_hex);
        assert_eq!(redeem_script.to_address(Network::Testnet),
                   "2N1mHzwKTmjnC7JjqeGFBRKYE4WDTjTfop1");
        assert_eq!(RedeemScript::from_hex(redeem_script_hex).unwrap(),
                   redeem_script);

        let compressed_redeem_script = redeem_script.compressed(Network::Testnet);
        assert_eq!(compressed_redeem_script.to_hex(),
                   "5321027db7837e51888e94c094703030d162c682c8dba312210f44ff440fbd5e5c24732102bdd272891c9e4dfc3962b1fdffd5a59732019816f9db4833634dbdaf01a401a52103280883dc31ccaee34218819aaa245480c35a33acd91283586ff6d1284ed681e52103e2bc790a6e32bf5a766919ff55b1f9e9914e13aed84f502c0e4171976e19deb054ae");
        assert_eq!(compressed_redeem_script.compressed(Network::Testnet),
                   compressed_redeem_script);
    }

    #[test]
    fn test_sign_raw_transaction() {
        let unsigned_tx = BitcoinTx::from_hex("01000000015d1b8ba33a162d8f6e7c5707fbb557e726c32f30f77f2ba348a48c3c5d71ee0b0000000000ffffffff02b80b00000000000017a914889fc9c82819c7a728974ffa78cc884e3e9e68838700000000000000002c6a2a6a28020000000000000062467691cf583d4fa78b18fafaf9801f505e0ef03baf0603fd4b0cd004cd1e7500000000").unwrap();

        let priv_key = Privkey::from_base58check("cVC9eJN5peJemWn1byyWcWDevg6xLNXtACjHJWmrR5ynsCu8mkQE")
            .unwrap();
        let pub_key = {
            let context = Secp256k1::new();
            RawPublicKey::from_secret_key(&context, priv_key.secret_key()).unwrap()
        };

        let redeem_script = RedeemScript::from_hex("5321027db7837e51888e94c094703030d162c682c8dba312210f44ff440fbd5e5c24732102bdd272891c9e4dfc3962b1fdffd5a59732019816f9db4833634dbdaf01a401a52103280883dc31ccaee34218819aaa245480c35a33acd91283586ff6d1284ed681e52103e2bc790a6e32bf5a766919ff55b1f9e9914e13aed84f502c0e4171976e19deb054ae").unwrap();
        let actual_signature = sign_input(&unsigned_tx, 0, &redeem_script, priv_key.secret_key());

        assert_eq!(actual_signature.to_hex(),
                   "304502210092f1fd6367677ef63dfddfb69cb3644ab10a7c497e5cd391e1d36284dca6a570022021dc2132349afafb9273600698d806f6d5f55756fcc058fba4e49c066116124e01");
        assert!(verify_input(&unsigned_tx,
                             0,
                             &redeem_script,
                             &pub_key,
                             actual_signature.as_ref()));
    }

    #[test]
    fn test_redeem_script_pubkey() {
        let redeem_script = RedeemScript::from_hex("55210351d8beec8ef4faef9a299640f2f2c8427b4c5ec655da3bdf9c78bb02debce7052103c39016fa9182f84d367d382b561a3db2154041926e4e461607a903ce2b78dbf72103cba17beba839abbc377f8ff8a908199d544ef821509a45ec3b5684e733e4d73b2102014c953a69d452a8c385d1c68e985d697d04f79bf0ddb11e2852e40b9bb880a4210389cbc7829f40deff4acef55babf7dc486a805ad0f4533e665dee4dd6d38157a32103c60e0aeb3d87b05f49341aa88a347237ab2cff3e91a78d23880080d05f8f08e756ae").unwrap();

        assert_eq!(redeem_script.script_pubkey(Network::Testnet).to_hex(),
                   "a914544fa2db1f36b091bbee603c0bc7675fe34655ff87");
    }

    #[test]
    fn test_anchoring_tx_sign() {
        let _ = blockchain_explorer::helpers::init_logger();

        let priv_keys = ["cVC9eJN5peJemWn1byyWcWDevg6xLNXtACjHJWmrR5ynsCu8mkQE",
                         "cMk66oMazTgquBVaBLHzDi8FMgAaRN3tSf6iZykf9bCh3D3FsLX1",
                         "cT2S5KgUQJ41G6RnakJ2XcofvoxK68L9B44hfFTnH4ddygaxi7rc",
                         "cRUKB8Nrhxwd5Rh6rcX3QK1h7FosYPw5uzEsuPpzLcDNErZCzSaj"]
                .iter()
                .map(|x| btc::PrivateKey::from_base58check(x).unwrap())
                .collect::<Vec<_>>();

        let pub_keys = ["03475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c",
                        "02a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0",
                        "0230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb49",
                        "036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e"]
                .iter()
                .map(|x| btc::PublicKey::from_hex(x).unwrap())
                .collect::<Vec<_>>();
        let redeem_script = RedeemScript::from_pubkeys(pub_keys.iter(), 3)
            .compressed(Network::Testnet);

        let prev_tx = AnchoringTx::from_hex("01000000014970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd670100483045022100e6ef3de83437c8dc33a8099394b7434dfb40c73631fc4b0378bd6fb98d8f42b002205635b265f2bfaa6efc5553a2b9e98c2eabdfad8e8de6cdb5d0d74e37f1e198520147304402203bb845566633b726e41322743677694c42b37a1a9953c5b0b44864d9b9205ca10220651b7012719871c36d0f89538304d3f358da12b02dab2b4d74f2981c8177b69601473044022052ad0d6c56aa6e971708f079073260856481aeee6a48b231bc07f43d6b02c77002203a957608e4fbb42b239dd99db4e243776cc55ed8644af21fa80fd9be77a59a60014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02b80b00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280000000000000000f1cb806d27e367f1cac835c22c8cc24c402a019e2d3ea82f7f841c308d830a9600000000").unwrap();
        let funding_tx = FundingTx::from_hex("01000000019532a4022a22226a6f694c3f21216b2c9f5c1c79007eb7d3be06bc2f1f9e52fb000000006a47304402203661efd05ca422fad958b534dbad2e1c7db42bbd1e73e9b91f43a2f7be2f92040220740cf883273978358f25ca5dd5700cce5e65f4f0a0be2e1a1e19a8f168095400012102ae1b03b0f596be41a247080437a50f4d8e825b170770dcb4e5443a2eb2ecab2afeffffff02a00f00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678716e1ff05000000001976a91402f5d7475a10a9c24cea32575bd8993d3fabbfd388ac089e1000").unwrap();

        let tx = TransactionBuilder::with_prev_tx(&prev_tx, 0)
            .add_funds(&funding_tx, 0)
            .payload(10, Hash::from_hex("164d236bbdb766e64cec57847e3a0509d4fc77fa9c17b7e61e48f7a3eaa8dbc9").unwrap())
            .fee(1000)
            .send_to(btc::Address::from_script(&redeem_script, Network::Testnet))
            .into_transaction()
            .unwrap();

        let mut signatures = HashMap::new();
        for input in tx.inputs() {
            let mut input_signs = Vec::new();
            for priv_key in &priv_keys {
                let sign = tx.sign(&redeem_script, input, priv_key);
                input_signs.push(sign);
            }
            signatures.insert(input, input_signs);
        }

        for (input, signs) in &signatures {
            for (id, signature) in signs.iter().enumerate() {
                assert!(tx.verify(&redeem_script, *input, &pub_keys[id], signature.as_ref()));
            }
        }
    }

    #[test]
    fn test_anchoring_tx_output_address() {
        let tx = AnchoringTx::from_hex("01000000014970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd670100483045022100e6ef3de83437c8dc33a8099394b7434dfb40c73631fc4b0378bd6fb98d8f42b002205635b265f2bfaa6efc5553a2b9e98c2eabdfad8e8de6cdb5d0d74e37f1e198520147304402203bb845566633b726e41322743677694c42b37a1a9953c5b0b44864d9b9205ca10220651b7012719871c36d0f89538304d3f358da12b02dab2b4d74f2981c8177b69601473044022052ad0d6c56aa6e971708f079073260856481aeee6a48b231bc07f43d6b02c77002203a957608e4fbb42b239dd99db4e243776cc55ed8644af21fa80fd9be77a59a60014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02b80b00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280000000000000000f1cb806d27e367f1cac835c22c8cc24c402a019e2d3ea82f7f841c308d830a9600000000").unwrap();

        let pub_keys = ["03475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c",
                        "02a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0",
                        "0230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb49",
                        "036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e"]
                .iter()
                .map(|x| btc::PublicKey::from_hex(x).unwrap())
                .collect::<Vec<_>>();
        let redeem_script = RedeemScript::from_pubkeys(&pub_keys, 3).compressed(Network::Testnet);

        assert_eq!(tx.output_address(Network::Testnet).to_base58check(),
                   redeem_script.to_address(Network::Testnet));
    }

    #[test]
    fn test_anchoring_tx_prev_chain() {
        let prev_tx = AnchoringTx::from_hex("01000000014970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd670100483045022100e6ef3de83437c8dc33a8099394b7434dfb40c73631fc4b0378bd6fb98d8f42b002205635b265f2bfaa6efc5553a2b9e98c2eabdfad8e8de6cdb5d0d74e37f1e198520147304402203bb845566633b726e41322743677694c42b37a1a9953c5b0b44864d9b9205ca10220651b7012719871c36d0f89538304d3f358da12b02dab2b4d74f2981c8177b69601473044022052ad0d6c56aa6e971708f079073260856481aeee6a48b231bc07f43d6b02c77002203a957608e4fbb42b239dd99db4e243776cc55ed8644af21fa80fd9be77a59a60014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02b80b00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280000000000000000f1cb806d27e367f1cac835c22c8cc24c402a019e2d3ea82f7f841c308d830a9600000000").unwrap();
        let tx = TransactionBuilder::with_prev_tx(&prev_tx, 0)
            .fee(1000)
            .payload(0, Hash::default())
            .prev_tx_chain(Some(prev_tx.id()))
            .send_to(btc::Address::from_base58check("2N1mHzwKTmjnC7JjqeGFBRKYE4WDTjTfop1")
                         .unwrap())
            .into_transaction()
            .unwrap();

        assert_eq!(tx.prev_tx_chain(), Some(prev_tx.id()));
    }

    #[test]
    fn test_tx_kind_funding() {
        let tx = BitcoinTx::from_hex("01000000019532a4022a22226a6f694c3f21216b2c9f5c1c79007eb7d3be06bc2f1f9e52fb000000006a47304402203661efd05ca422fad958b534dbad2e1c7db42bbd1e73e9b91f43a2f7be2f92040220740cf883273978358f25ca5dd5700cce5e65f4f0a0be2e1a1e19a8f168095400012102ae1b03b0f596be41a247080437a50f4d8e825b170770dcb4e5443a2eb2ecab2afeffffff02a00f00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678716e1ff05000000001976a91402f5d7475a10a9c24cea32575bd8993d3fabbfd388ac089e1000").unwrap();
        match TxKind::from(tx) {
            TxKind::FundingTx(_) => {}
            _ => panic!("Wrong tx kind!"),
        }
    }

    #[test]
    fn test_tx_kind_anchoring() {
        let tx = BitcoinTx::from_hex("01000000014970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd670100483045022100e6ef3de83437c8dc33a8099394b7434dfb40c73631fc4b0378bd6fb98d8f42b002205635b265f2bfaa6efc5553a2b9e98c2eabdfad8e8de6cdb5d0d74e37f1e198520147304402203bb845566633b726e41322743677694c42b37a1a9953c5b0b44864d9b9205ca10220651b7012719871c36d0f89538304d3f358da12b02dab2b4d74f2981c8177b69601473044022052ad0d6c56aa6e971708f079073260856481aeee6a48b231bc07f43d6b02c77002203a957608e4fbb42b239dd99db4e243776cc55ed8644af21fa80fd9be77a59a60014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02b80b00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280000000000000000f1cb806d27e367f1cac835c22c8cc24c402a019e2d3ea82f7f841c308d830a9600000000").unwrap();
        match TxKind::from(tx) {
            TxKind::Anchoring(_) => {}
            _ => panic!("Wrong tx kind!"),
        }
    }

    #[test]
    fn test_tx_kind_other() {
        let tx = BitcoinTx::from_hex("0100000001cea827387bc0bb1b5e6afa6e6d557123e4432e47bad8c2d94214a9cd1e2e074b010000006a473044022034d463312dd75445ad078b1159a75c0b148388b36686b69da8aecca863e63dc3022071ef86a064bd15f11ec89059072bbd3e3d3bb6c5e9b10712e0e2dc6710520bb00121035e63a48d34250dbbcc58fdc0ab63b901769e71035e19e0eee1a87d433a96723afeffffff0296a6f80b000000001976a914b5d7055cfdacc803e5547b981faa693c5aaa813b88aca0860100000000001976a914f5548cb02bb197f071934a0ea3eeb5878cb59dff88ac03a21000").unwrap();
        match TxKind::from(tx) {
            TxKind::Other(_) => {}
            _ => panic!("Wrong tx kind!"),
        }
    }
}
