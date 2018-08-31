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

use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

use bitcoin::blockdata::script::Script;
use bitcoin::blockdata::transaction::{OutPoint, TxIn, TxOut};
use bitcoin::network::serialize::{deserialize, serialize, serialize_hex, BitcoinHash};
use bitcoin::util::privkey::Privkey;
use bitcoinrpc;
use btc_transaction_utils::{p2wsh, InputSignature, InputSignatureRef, TxInRef};
use secp256k1::key::{PublicKey, SecretKey};

use exonum::crypto::{hash, Hash};
use exonum::encoding::serialize::{FromHex, FromHexError};
use exonum::helpers::Height;
use exonum::storage::StorageValue;

use details::btc;
use details::btc::payload::{Payload, PayloadBuilder};
use details::btc::{HexValueEx, RedeemScript, TxId};
use details::error::Error as InternalError;
use details::rpc::{Error as RpcError, RpcClient};

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
    payload: Option<(Height, Hash)>,
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
            Err(FromHexError::InvalidStringLength)
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
    pub fn find_out(&self, addr: &btc::Address) -> Option<u32> {
        let script_pubkey = addr.0.script_pubkey();
        self.0
            .output
            .iter()
            .position(|output| output.script_pubkey == script_pubkey)
            .map(|x| x as u32)
    }

    pub fn has_unspent_info(
        &self,
        client: &RpcClient,
        addr: &btc::Address,
    ) -> Result<Option<bitcoinrpc::UnspentTransactionInfo>, RpcError> {
        let txid = self.id().to_string();
        let txs = client.listunspent(0, 9_999_999, &[addr.to_string()])?;
        Ok(txs.into_iter().find(|txinfo| txinfo.txid == txid))
    }
}

impl AnchoringTx {
    pub fn amount(&self) -> u64 {
        self.0.output[ANCHORING_TX_FUNDS_OUTPUT as usize].value
    }

    pub fn script_pubkey(&self) -> &Script {
        &self.0.output[ANCHORING_TX_FUNDS_OUTPUT as usize].script_pubkey
    }

    pub fn inputs(&self) -> ::std::ops::Range<u32> {
        0..self.0.input.len() as u32
    }

    pub fn payload(&self) -> Payload {
        find_payload(&self.0).expect("Unable to find payload")
    }

    pub fn prev_hash(&self) -> TxId {
        TxId::from(self.0.input[0].previous_output.txid)
    }

    pub fn sign_input(
        &self,
        redeem_script: &btc::RedeemScript,
        input: u32,
        prev_tx: &RawBitcoinTx,
        priv_key: &Privkey,
    ) -> btc::Signature {
        sign_tx_input(
            self,
            input as usize,
            redeem_script,
            prev_tx,
            priv_key.secret_key(),
        )
    }

    pub fn verify_input(
        &self,
        redeem_script: &RedeemScript,
        input: u32,
        prev_tx: &RawBitcoinTx,
        pub_key: &PublicKey,
        signature: &[u8],
    ) -> bool {
        verify_tx_input(
            self,
            input as usize,
            redeem_script,
            prev_tx,
            pub_key,
            signature,
        )
    }

    pub fn finalize(
        self,
        redeem_script: &btc::RedeemScript,
        signatures: HashMap<u32, Vec<btc::Signature>>,
    ) -> AnchoringTx {
        finalize_anchoring_transaction(self, redeem_script, signatures)
    }
}

impl fmt::Debug for AnchoringTx {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let payload = self.payload();
        f.debug_struct(stringify!(AnchoringTx))
            .field("txid", &self.id())
            .field("txhex", &self.to_hex())
            .field("content", &self.0)
            .field("payload", &payload)
            .finish()
    }
}

impl fmt::Debug for FundingTx {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(stringify!(FundingTx))
            .field("txid", &self.id())
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
            // Finds output with funds and p2wsh script_pubkey
            for out in &tx.output {
                if out.value > 0 && out.script_pubkey.is_v0_p2wsh() {
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

    pub fn payload(mut self, height: Height, hash: Hash) -> TransactionBuilder {
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

        let tx = create_anchoring_transaction(
            &addr,
            height,
            block_hash,
            self.inputs.iter(),
            total_funds,
            self.prev_tx_chain,
        );
        Ok(tx)
    }
}

fn create_anchoring_transaction<'a, I>(
    addr: &btc::Address,
    block_height: Height,
    block_hash: Hash,
    inputs: I,
    out_funds: u64,
    prev_chain_txid: Option<TxId>,
) -> AnchoringTx
where
    I: Iterator<Item = &'a (RawBitcoinTx, u32)>,
{
    let inputs = inputs
        .map(|&(ref unspent_tx, utxo_vout)| TxIn {
            previous_output: OutPoint {
                txid: unspent_tx.txid(),
                vout: utxo_vout,
            },
            script_sig: Script::new(),
            sequence: 0xFFFF_FFFF,
            witness: Vec::default(),
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
    };
    AnchoringTx::from(tx)
}

pub fn sign_tx_input(
    tx: &RawBitcoinTx,
    input: usize,
    subscript: &RedeemScript,
    prev_tx: &RawBitcoinTx,
    sec_key: &SecretKey,
) -> Vec<u8> {
    let mut signer = p2wsh::InputSigner::new(subscript.clone());
    signer
        .sign_input(TxInRef::new(tx, input), prev_tx, sec_key)
        .unwrap()
        .into()
}

pub fn verify_tx_input(
    tx: &RawBitcoinTx,
    input: usize,
    subscript: &RedeemScript,
    prev_tx: &RawBitcoinTx,
    pub_key: &PublicKey,
    signature: &[u8],
) -> bool {
    let signer = p2wsh::InputSigner::new(subscript.clone());
    InputSignatureRef::from_bytes(signer.secp256k1_context(), signature)
        .and_then(|signature| {
            signer.verify_input(TxInRef::new(tx, input), prev_tx, pub_key, signature)
        })
        .is_ok()
}

fn finalize_anchoring_transaction(
    mut anchoring_tx: AnchoringTx,
    redeem_script: &btc::RedeemScript,
    signatures: HashMap<u32, Vec<btc::Signature>>,
) -> AnchoringTx {
    let signer = p2wsh::InputSigner::new(redeem_script.clone());
    for (out, signatures) in signatures {
        let signatures = signatures
            .into_iter()
            .map(|bytes| InputSignature::from_bytes(signer.secp256k1_context(), bytes).unwrap());
        signer.spend_input(&mut anchoring_tx.0.input[out as usize], signatures);
    }
    anchoring_tx
}

fn find_payload(tx: &RawBitcoinTx) -> Option<Payload> {
    tx.output
        .get(ANCHORING_TX_DATA_OUTPUT as usize)
        .and_then(|output| Payload::from_script(&output.script_pubkey))
}
