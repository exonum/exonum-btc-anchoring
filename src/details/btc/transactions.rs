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

use std::fmt;
use std::collections::HashMap;
use std::ops::Deref;

use bitcoin::blockdata::script::Instruction;
use bitcoin::blockdata::opcodes::All;
use bitcoin::util::hash::Hash160;
use bitcoin::network::serialize::{BitcoinHash, deserialize, serialize, serialize_hex};
use bitcoin::blockdata::transaction::{TxIn, TxOut};
use bitcoin::blockdata::script::{Builder, Script};
use bitcoin::util::base58::ToBase58;
use bitcoin::util::address::{Address, Privkey, Type};
use bitcoin::network::constants::Network;
use bitcoin::blockdata::transaction::SigHashType;
use secp256k1::key::{PublicKey, SecretKey};
use secp256k1::{Message, Secp256k1, Signature};
use bitcoinrpc;

use exonum::crypto::{FromHexError, Hash, HexValue, hash};
use exonum::node::Height;
use exonum::storage::StorageValue;

use details::rpc::{AnchoringRpc, Error as RpcError, RpcClient};
use details::btc;
use details::btc::{HexValueEx, RedeemScript, TxId};
use details::error::Error as InternalError;
use details::btc::payload::{Payload, PayloadBuilder};

pub type RawBitcoinTx = ::bitcoin::blockdata::transaction::Transaction;

const ANCHORING_TX_FUNDS_OUTPUT: u32 = 0;
const ANCHORING_TX_DATA_OUTPUT: u32 = 1;

/// Anchoring transaction struct is strict:
/// - Zero input is previous anchoring tx or initial funding tx
/// - Zero output is next anchoring tx
/// - First output is anchored metadata
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AnchoringTx(pub RawBitcoinTx);
/// Funding transaction always has an output to `p2sh` address
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FundingTx(pub RawBitcoinTx);
/// Other unspecified Bitcoin transaction
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BitcoinTx(pub RawBitcoinTx);

#[derive(Debug, Clone, PartialEq)]
pub enum TxKind {
    Anchoring(AnchoringTx),
    FundingTx(FundingTx),
    Other(BitcoinTx),
}

pub trait TxFromRaw: Sized {
    fn from_raw(raw: RawBitcoinTx) -> Option<Self>;
}

#[derive(Debug)]
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

implement_serde_hex! {AnchoringTx}
implement_serde_hex! {FundingTx}
implement_serde_hex! {BitcoinTx}

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

    pub fn has_unspent_info(&self,
                            client: &RpcClient,
                            addr: &btc::Address)
                            -> Result<Option<bitcoinrpc::UnspentTransactionInfo>, RpcError> {
        let txid = self.txid();
        let txs = client
            .listunspent(0, 9999999, [addr.to_base58check().as_ref()])?;
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
        }.into()
    }

    pub fn inputs(&self) -> ::std::ops::Range<u32> {
        0..self.0.input.len() as u32
    }

    pub fn payload(&self) -> Payload {
        find_payload(&self.0).expect("Unable to find payload")
    }

    pub fn prev_hash(&self) -> TxId {
        TxId::from(self.0.input[0].prev_hash)
    }

    pub fn sign_input(&self,
                      redeem_script: &btc::RedeemScript,
                      input: u32,
                      priv_key: &Privkey)
                      -> btc::Signature {
        let mut sign_data =
            sign_tx_input(self, input as usize, redeem_script, priv_key.secret_key());
        /// Adds btc related sighash type byte
        sign_data.push(SigHashType::All.as_u32() as u8);
        sign_data
    }

    pub fn verify_input(&self,
                        redeem_script: &RedeemScript,
                        input: u32,
                        pub_key: &PublicKey,
                        signature: &[u8])
                        -> bool {
        /// Cuts off btc related sighash type byte
        let signature = &signature[0..signature.len() - 1];
        verify_tx_input(self, input as usize, redeem_script, pub_key, signature)
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
            .field("payload", &payload)
            .finish()
    }
}

impl fmt::Debug for FundingTx {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(stringify!(FundingTx))
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

impl TxFromRaw for BitcoinTx {
    fn from_raw(raw: RawBitcoinTx) -> Option<BitcoinTx> {
        Some(BitcoinTx(raw))
    }
}

impl TxFromRaw for AnchoringTx {
    fn from_raw(raw: RawBitcoinTx) -> Option<AnchoringTx> {
        if let TxKind::Anchoring(tx) = TxKind::from(raw) {
            Some(tx)
        } else {
            None
        }
    }
}

impl TxFromRaw for FundingTx {
    fn from_raw(raw: RawBitcoinTx) -> Option<FundingTx> {
        if let TxKind::FundingTx(tx) = TxKind::from(raw) {
            Some(tx)
        } else {
            None
        }
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

    pub fn into_transaction(mut self) -> Result<AnchoringTx, InternalError> {
        let available_funds: u64 = self.inputs
            .iter()
            .map(|&(ref tx, out)| tx.output[out as usize].value)
            .sum();

        let addr = self.output.take().expect("Output address is not set");
        let fee = self.fee.expect("Fee is not set");
        let (height, block_hash) = self.payload.take().expect("Payload is not set");
        if available_funds < fee {
            return Err(InternalError::InsufficientFunds);
        }
        let total_funds = available_funds - fee;

        let tx = create_anchoring_transaction(&addr,
                                              height,
                                              block_hash,
                                              self.inputs.iter(),
                                              total_funds,
                                              self.prev_tx_chain);
        Ok(tx)
    }
}

fn create_anchoring_transaction<'a, I>(addr: &btc::Address,
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

    let metadata_script = PayloadBuilder::new()
        .block_hash(block_hash)
        .block_height(block_height)
        .prev_tx_chain(prev_chain_txid)
        .into_script();
    let outputs = vec![
        TxOut {
            value: out_funds,
            script_pubkey: addr.script_pubkey(),
        },
        TxOut {
            value: 0,
            script_pubkey: metadata_script,
        },
    ];

    let tx = RawBitcoinTx {
        version: 1,
        lock_time: 0,
        input: inputs,
        output: outputs,
        witness: vec![],
    };
    AnchoringTx::from(tx)
}

pub fn sign_tx_input(tx: &RawBitcoinTx,
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
    sign.serialize_der(&context)
}

pub fn verify_tx_input(tx: &RawBitcoinTx,
                       input: usize,
                       subscript: &Script,
                       pub_key: &PublicKey,
                       signature: &[u8])
                       -> bool {
    let sighash = tx.signature_hash(input, subscript, SigHashType::All.as_u32());
    let msg = Message::from_slice(&sighash[..]).unwrap();

    let context = Secp256k1::new();
    if let Ok(sign) = Signature::from_der(&context, signature) {
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


fn find_payload(tx: &RawBitcoinTx) -> Option<Payload> {
    tx.output
        .get(ANCHORING_TX_DATA_OUTPUT as usize)
        .and_then(|output| Payload::from_script(&output.script_pubkey))
}
