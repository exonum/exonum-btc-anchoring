use std::fmt;
use std::collections::HashMap;

use byteorder::{ByteOrder, LittleEndian};

use bitcoin::blockdata::script::Instruction;
use bitcoin::blockdata::opcodes::All;
use bitcoin::util::hash::Hash160;
use bitcoin::network::serialize::{BitcoinHash, serialize_hex, deserialize, serialize};
use bitcoinrpc;

// FIXME do not use Hash from crypto, use Sha256Hash explicit
use exonum::crypto::{hash, Hash, FromHexError, ToHex, FromHex};
use exonum::node::Height;
use exonum::storage::StorageValue;

use super::{AnchoringRpc, RpcClient, HexValue, BitcoinSignature, Result};

pub type BitcoinTx = ::bitcoin::blockdata::transaction::Transaction;

#[derive(Clone, PartialEq)]
pub struct AnchoringTx(pub BitcoinTx);

#[derive(Clone, PartialEq)]
pub struct FundingTx(pub BitcoinTx);

pub enum TxKind {
    Anchoring(AnchoringTx),
    FundingTx(FundingTx),
    Other(BitcoinTx),
}

impl HexValue for BitcoinTx {
    fn to_hex(&self) -> String {
        serialize_hex(self).unwrap()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError> {
        let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
        if let Ok(tx) = deserialize(bytes.as_ref()) {
            Ok(tx)
        } else {
            Err(FromHexError::InvalidHexLength)
        }
    }
}

impl AnchoringTx {
    pub fn txid(&self) -> Hash {
        let hash = self.0.bitcoin_hash();
        let bytes = {
            let mut bytes = [0; 32];
            bytes.copy_from_slice(hash[..].as_ref());
            bytes.reverse(); // FIXME what about big endianless architectures?
            bytes
        };
        Hash::new(bytes)
    }

    pub fn funds(&self, out: u64) -> u64 {
        self.0.output[out as usize].value
    }

    pub fn payload(&self) -> (Height, Hash) {
        find_payload(&self.0).expect("Unable to find payload")
    }

    pub fn out(&self, multisig: &bitcoinrpc::MultiSig) -> u64 {
        let redeem_script = Vec::<u8>::from_hex(multisig.redeem_script.clone()).unwrap();
        let redeem_script_hash = Hash160::from_data(&redeem_script);

        self.0
            .output
            .iter()
            .position(|output| if let Some(Instruction::PushBytes(bytes)) =
                output.script_pubkey.into_iter().nth(1) {
                Hash160::from(bytes) == redeem_script_hash
            } else {
                false
            })
            .unwrap() as u64
    }

    pub fn get_info(&self, client: &RpcClient) -> Result<Option<bitcoinrpc::RawTransactionInfo>> {
        let r = client.getrawtransaction(&self.txid().to_hex());
        match r {
            Ok(tx) => Ok(Some(tx)),
            Err(bitcoinrpc::Error::NoInformation(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn sign(&self,
                client: &RpcClient,
                multisig: &bitcoinrpc::MultiSig,
                input: u64,
                priv_key: &str)
                -> Result<BitcoinSignature> {
        client.sign_anchoring_transaction(&self.0, &multisig.redeem_script, input, priv_key)
    }

    pub fn proposal(self,
                    client: &RpcClient,
                    from: &bitcoinrpc::MultiSig,
                    to: &bitcoinrpc::MultiSig,
                    fee: u64,
                    funding_txs: &[FundingTx],
                    block_height: Height,
                    block_hash: Hash)
                    -> Result<AnchoringTx> {
        let inputs = {
            let utxo_vout = self.out(from);
            let utxo_tx = self.0;

            let mut inputs = vec![(utxo_tx, utxo_vout)];
            for funding_tx in funding_txs {
                inputs.push((funding_tx.0.clone(), funding_tx.find_out(&from).unwrap()));
            }
            inputs
        };

        let mut out_funds = 0;
        for &(ref utxo_tx, utxo_vout) in &inputs {
            out_funds += utxo_tx.output[utxo_vout as usize].value;
        }
        out_funds -= fee;

        let tx = client.create_anchoring_transaction(&to.address,
                                          block_height,
                                          block_hash,
                                          inputs.iter(),
                                          out_funds)?;
        Ok(tx)
    }

    pub fn finalize(self,
                    client: &RpcClient,
                    mutlisig: &bitcoinrpc::MultiSig,
                    signatures: HashMap<u64, Vec<BitcoinSignature>>)
                    -> Result<AnchoringTx> {
        let tx = client.finalize_anchoring_transaction(self, &mutlisig.redeem_script, signatures)?;
        Ok(tx)
    }

    pub fn send(self,
                client: &RpcClient,
                mutlisig: &bitcoinrpc::MultiSig,
                signatures: HashMap<u64, Vec<BitcoinSignature>>)
                -> Result<AnchoringTx> {
        let tx = self.finalize(client, mutlisig, signatures)?;
        client.send_transaction(tx.0.clone())?;
        Ok(tx)
    }
}

// TODO replace by macros

impl From<BitcoinTx> for AnchoringTx {
    fn from(tx: BitcoinTx) -> AnchoringTx {
        AnchoringTx(tx)
    }
}

impl HexValue for AnchoringTx {
    fn to_hex(&self) -> String {
        serialize_hex(&self.0).unwrap()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError> {
        let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
        if let Ok(tx) = deserialize::<BitcoinTx>(bytes.as_ref()) {
            Ok(AnchoringTx::from(tx))
        } else {
            Err(FromHexError::InvalidHexLength)
        }
    }
}

impl StorageValue for AnchoringTx {
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![];
        v.extend(serialize(&self.0).unwrap());
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        let tx = deserialize::<BitcoinTx>(v.as_ref()).unwrap();
        AnchoringTx::from(tx)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![];
        v.extend(serialize(&self.0).unwrap());
        hash(&v)
    }
}

impl fmt::Debug for AnchoringTx {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AnchoringTx")
            .field("txid", &self.txid().to_hex())
            .field("txhex", &self.to_hex())
            .field("content", &self.0)
            .finish()
    }
}

impl From<BitcoinTx> for FundingTx {
    fn from(tx: BitcoinTx) -> FundingTx {
        FundingTx(tx)
    }
}

impl FundingTx {
    pub fn create(client: &RpcClient,
                  multisig: &bitcoinrpc::MultiSig,
                  total_funds: u64)
                  -> Result<FundingTx> {
        let tx = client.send_to_address(&multisig.address, total_funds)?;
        Ok(FundingTx(tx))
    }

    // TODO Use BitcoinTxId trait
    pub fn txid(&self) -> Hash {
        let hash = self.0.bitcoin_hash();
        let bytes = unsafe {
            let ptr = hash.as_ptr();
            let slice = ::std::slice::from_raw_parts(ptr, 32);
            let mut bytes = [0; 32];
            bytes.copy_from_slice(slice);
            bytes.reverse(); // FIXME what about big endianless architectures?
            bytes
        };
        Hash::new(bytes)
    }

    pub fn find_out(&self, multisig: &bitcoinrpc::MultiSig) -> Option<u64> {
        let redeem_script = Vec::<u8>::from_hex(multisig.redeem_script.clone()).unwrap();
        let redeem_script_hash = Hash160::from_data(&redeem_script);

        self.0
            .output
            .iter()
            .position(|output| if let Some(Instruction::PushBytes(bytes)) =
                output.script_pubkey.into_iter().nth(1) {
                Hash160::from(bytes) == redeem_script_hash
            } else {
                false
            })
            .map(|x| x as u64)
    }

    pub fn is_unspent(&self,
                      client: &RpcClient,
                      multisig: &bitcoinrpc::MultiSig)
                      -> Result<Option<bitcoinrpc::UnspentTransactionInfo>> {
        let txid = self.txid().to_hex();
        let txs = client.listunspent(0, 999999, [multisig.address.as_str()])?;
        Ok(txs.into_iter()
            .find(|txinfo| txinfo.txid == txid))
    }

    pub fn make_anchoring_tx(self,
                             client: &RpcClient,
                             multisig: &bitcoinrpc::MultiSig,
                             fee: u64,
                             block_height: Height,
                             block_hash: Hash)
                             -> Result<AnchoringTx> {
        let utxo_vout = self.find_out(multisig).unwrap();
        let utxo_tx = self.0;

        let out_funds = utxo_tx.output[utxo_vout as usize].value - fee;
        let tx = client.create_anchoring_transaction(&multisig.address,
                                          block_height,
                                          block_hash,
                                          [(utxo_tx, utxo_vout)].iter(),
                                          out_funds)?;
        Ok(tx)
    }
}

impl From<BitcoinTx> for TxKind {
    fn from(tx: BitcoinTx) -> TxKind {
        if find_payload(&tx).is_some() {
            TxKind::Anchoring(AnchoringTx::from(tx))
        } else {
            if tx.output.len() == 1 {
                TxKind::FundingTx(FundingTx::from(tx))
            } else {
                TxKind::Other(tx)
            }
        }
    }
}

impl TxKind {
    pub fn from_txid(client: &RpcClient, txid: Hash) -> Result<TxKind> {
        let tx = client.get_transaction(txid.to_hex().as_ref())?;
        Ok(TxKind::from(tx))
    }
}

impl StorageValue for FundingTx {
    fn serialize(self) -> Vec<u8> {
        let mut v = vec![];
        v.extend(serialize(&self.0).unwrap());
        v
    }

    fn deserialize(v: Vec<u8>) -> Self {
        let tx = deserialize::<BitcoinTx>(v.as_ref()).unwrap();
        FundingTx::from(tx)
    }

    fn hash(&self) -> Hash {
        let mut v = vec![];
        v.extend(serialize(&self.0).unwrap());
        hash(&v)
    }
}

impl HexValue for FundingTx {
    fn to_hex(&self) -> String {
        serialize_hex(&self.0).unwrap()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError> {
        let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
        if let Ok(tx) = deserialize::<BitcoinTx>(bytes.as_ref()) {
            Ok(FundingTx::from(tx))
        } else {
            Err(FromHexError::InvalidHexLength)
        }
    }
}

impl fmt::Debug for FundingTx {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FundingTx")
            .field("txid", &self.txid().to_hex())
            .field("content", &self.0)
            .finish()
    }
}

fn find_payload(tx: &BitcoinTx) -> Option<(Height, Hash)> {
    tx.output
        .iter()
        .find(|output| {
            output.script_pubkey.into_iter().next() == Some(Instruction::Op(All::OP_RETURN))
        })
        .and_then(|output| {
            output.script_pubkey
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
