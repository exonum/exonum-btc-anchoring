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
use bitcoin::util::base58::FromBase58;
use bitcoin::util::address::{Address, Privkey};
use bitcoin::network::constants::Network;
use secp256k1::Secp256k1;
use secp256k1::key::PublicKey;
use bitcoinrpc;

// FIXME do not use Hash from crypto, use Sha256Hash explicit
use exonum::crypto::{hash, Hash, FromHexError, ToHex, FromHex};
use exonum::node::Height;
use exonum::storage::StorageValue;

use {BITCOIN_NETWORK, AnchoringRpc, RpcClient, HexValue, BitcoinSignature, Result};
use multisig::{sign_input, verify_input, RedeemScript};
use crypto::TxId;

pub type BitcoinTx = ::bitcoin::blockdata::transaction::Transaction;

const ANCHORING_TX_FUNDS_OUTPUT: u32 = 0;
const ANCHORING_TX_DATA_OUTPUT: u32 = 1;
// Структура у анкорящей транзакции строгая:
// - нулевой вход это прошлая анкорящая транзакция или фундирующая, если транзакция исходная
// - нулевой выход это всегда следующая анкорящая транзакция
// - первый выход это метаданные
// Итого транзакции у которых нулевой вход нам не известен, а выходов не два или они содержат другую информацию,
// считаются априори не валидными.
#[derive(Clone, PartialEq)]
pub struct AnchoringTx(pub BitcoinTx);
// Структура валидной фундирующей транзакции тоже строгая:
// Входов и выходов может быть несколько, но главное правило, чтобы нулевой вход переводил деньги на мультисиг кошелек
#[derive(Clone, PartialEq)]
pub struct FundingTx(pub BitcoinTx);

pub enum TxKind {
    Anchoring(AnchoringTx),
    FundingTx(FundingTx),
    Other(BitcoinTx),
}

pub struct TransactionBuilder {
    inputs: Vec<(BitcoinTx, u32)>,
    output: Option<(String, u64)>,
    payload: Option<(u64, Hash)>,
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

macro_rules! implement_tx {
($name:ident) => (
    impl $name {
        pub fn id(&self) -> TxId {
            TxId::from(self.0.bitcoin_hash())
        }

        pub fn txid(&self) -> String {
            self.0.bitcoin_hash().be_hex_string()
        }
    }

    impl From<BitcoinTx> for $name {
        fn from(tx: BitcoinTx) -> $name {
            $name(tx)
        }
    }

    impl HexValue for $name  {
        fn to_hex(&self) -> String {
            serialize_hex(&self.0).unwrap()
        }
        fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError> {
            let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
            if let Ok(tx) = deserialize::<BitcoinTx>(bytes.as_ref()) {
                Ok($name::from(tx))
            } else {
                Err(FromHexError::InvalidHexLength)
            }
        }
    }

    impl StorageValue for $name {
        fn serialize(self) -> Vec<u8> {
            let mut v = vec![];
            v.extend(serialize(&self.0).unwrap());
            v
        }

        fn deserialize(v: Vec<u8>) -> Self {
            let tx = deserialize::<BitcoinTx>(v.as_ref()).unwrap();
            $name::from(tx)
        }

        fn hash(&self) -> Hash {
            let mut v = vec![];
            v.extend(serialize(&self.0).unwrap());
            hash(&v)
        }
    }

    impl<'a> ::exonum::messages::Field<'a> for $name {
        fn field_size() -> usize {
            8
        }

        fn read(buffer: &'a [u8], from: usize, to: usize) -> $name {
            let data = <&[u8] as ::exonum::messages::Field>::read(buffer, from, to);
            <$name as StorageValue>::deserialize(data.to_vec())
        }

        fn write(&self, buffer: &'a mut Vec<u8>, from: usize, to: usize) {
            <&[u8] as ::exonum::messages::Field>::write(&self.clone().serialize().as_slice(), buffer, from, to);
        }
    }

    impl Deref for $name {
        type Target = BitcoinTx;

        fn deref(&self) -> &BitcoinTx {
            &self.0
        }
    }

    impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.debug_struct(stringify!($name))
                .field("txid", &self.txid().to_hex())
                .field("txhex", &self.to_hex())
                .field("content", &self.0)
                .finish()
        }
    }
)
}

implement_tx! {AnchoringTx}
implement_tx! {FundingTx}

impl FundingTx {
    pub fn create(client: &RpcClient,
                  multisig: &bitcoinrpc::MultiSig,
                  total_funds: u64)
                  -> Result<FundingTx> {
        let tx = client.send_to_address(&multisig.address, total_funds)?;
        Ok(FundingTx(tx))
    }

    pub fn find_out(&self, addr: &str) -> Option<u32> {
        let redeem_script_hash = Address::from_base58check(addr).unwrap().hash;
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
                      multisig: &bitcoinrpc::MultiSig)
                      -> Result<Option<bitcoinrpc::UnspentTransactionInfo>> {
        let txid = self.txid();
        let txs = client.listunspent(0, 999999, [multisig.address.as_str()])?;
        Ok(txs.into_iter()
            .find(|txinfo| txinfo.txid == txid))
    }

    pub fn make_anchoring_tx(self,
                             multisig: &bitcoinrpc::MultiSig,
                             fee: u64,
                             block_height: Height,
                             block_hash: Hash)
                             -> Result<AnchoringTx> {
        let utxo_vout = self.find_out(&multisig.address).unwrap();
        let utxo_tx = self.0;

        let out_funds = utxo_tx.output[utxo_vout as usize].value - fee;
        let tx = create_anchoring_transaction(&multisig.address,
                                              block_height,
                                              block_hash,
                                              [(utxo_tx, utxo_vout)].iter(),
                                              out_funds);
        Ok(tx)
    }
}

impl AnchoringTx {
    pub fn amount(&self) -> u64 {
        self.0.output[ANCHORING_TX_FUNDS_OUTPUT as usize].value
    }

    pub fn output_address(&self, network: Network) -> Address {
        let ref script = self.0.output[ANCHORING_TX_FUNDS_OUTPUT as usize].script_pubkey;
        Address::from_script(network, script)
    }

    pub fn inputs(&self) -> ::std::ops::Range<u32> {
        0..self.0.input.len() as u32
    }

    pub fn payload(&self) -> (Height, Hash) {
        find_payload(&self.0).expect("Unable to find payload")
    }

    pub fn prev_hash(&self) -> TxId {
        TxId::from(self.0.input[0].prev_hash)
    }

    pub fn out(&self, multisig: &bitcoinrpc::MultiSig) -> u32 {
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
            .unwrap() as u32
    }

    pub fn get_info(&self, client: &RpcClient) -> Result<Option<bitcoinrpc::RawTransactionInfo>> {
        let r = client.getrawtransaction(&self.txid());
        match r {
            Ok(tx) => Ok(Some(tx)),
            Err(bitcoinrpc::Error::NoInformation(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn sign(&self, redeem_script: &str, input: u32, priv_key: &str) -> BitcoinSignature {
        sign_anchoring_transaction(self, redeem_script, input, priv_key)
    }

    pub fn verify(&self,
                  redeem_script: &RedeemScript,
                  input: u32,
                  pub_key: &str,
                  signature: &[u8])
                  -> bool {
        verify_anchoring_transaction(self, redeem_script, input, pub_key, signature)
    }

    pub fn proposal(self,
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
                inputs.push((funding_tx.0.clone(), funding_tx.find_out(&from.address).unwrap()));
            }
            inputs
        };

        let mut out_funds = 0;
        for &(ref utxo_tx, utxo_vout) in &inputs {
            out_funds += utxo_tx.output[utxo_vout as usize].value;
        }
        out_funds -= fee;

        let tx = create_anchoring_transaction(&to.address,
                                              block_height,
                                              block_hash,
                                              inputs.iter(),
                                              out_funds);
        Ok(tx)
    }

    pub fn finalize(self,
                    redeem_script: &str,
                    signatures: HashMap<u32, Vec<BitcoinSignature>>)
                    -> Result<AnchoringTx> {
        let tx = finalize_anchoring_transaction(self, redeem_script, signatures);
        Ok(tx)
    }

    pub fn send(self,
                client: &RpcClient,
                mutlisig: &bitcoinrpc::MultiSig,
                signatures: HashMap<u32, Vec<BitcoinSignature>>)
                -> Result<AnchoringTx> {
        let tx = self.finalize(&mutlisig.redeem_script, signatures)?;
        client.send_transaction(tx.0.clone())?;
        Ok(tx)
    }
}

impl TxKind {
    pub fn from_txid(client: &RpcClient, txid: Hash) -> Result<TxKind> {
        let tx = client.get_transaction(txid.to_hex().as_ref())?;
        Ok(TxKind::from(tx))
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

impl TransactionBuilder {
    pub fn with_prev_tx(prev_tx: &BitcoinTx, out: u32) -> TransactionBuilder {
        TransactionBuilder {
            inputs: vec![(prev_tx.clone(), out)],
            output: None,
            payload: None,
        }
    }

    pub fn add_funds(mut self, tx: &BitcoinTx, out: u32) -> TransactionBuilder {
        self.inputs.push((tx.clone(), out));
        self
    }

    pub fn payload(mut self, height: u64, hash: Hash) -> TransactionBuilder {
        self.payload = Some((height, hash));
        self
    }

    pub fn send_to<S: AsRef<str>>(mut self, addr: S, out_funds: u64) -> TransactionBuilder {
        self.output = Some((addr.as_ref().to_string(), out_funds));
        self
    }

    pub fn into_transaction(mut self) -> AnchoringTx {
        let (addr, funds) = self.output.take().unwrap();
        let (height, block_hash) = self.payload.take().unwrap();
        create_anchoring_transaction(&addr, height, block_hash, self.inputs.iter(), funds)
    }
}

fn create_anchoring_transaction<'a, I>(output_addr: &str,
                                       block_height: Height,
                                       block_hash: Hash,
                                       inputs: I,
                                       out_funds: u64)
                                       -> AnchoringTx
    where I: Iterator<Item = &'a (BitcoinTx, u32)>
{
    let inputs = inputs.map(|&(ref unspent_tx, utxo_vout)| {
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
    let addr = Address::from_base58check(output_addr).unwrap();
    let outputs = vec![TxOut {
                           value: out_funds,
                           script_pubkey: addr.script_pubkey(),
                       },
                       TxOut {
                           value: 0,
                           script_pubkey: metadata_script,
                       }];

    let tx = BitcoinTx {
        version: 1,
        lock_time: 0,
        input: inputs,
        output: outputs,
        witness: vec![],
    };
    AnchoringTx::from(tx)
}

fn sign_anchoring_transaction(tx: &BitcoinTx,
                              redeem_script: &str,
                              vin: u32,
                              priv_key: &str)
                              -> BitcoinSignature {
    let priv_key = Privkey::from_base58check(priv_key).unwrap();
    let redeem_script = RedeemScript::from_hex(redeem_script).unwrap().compressed(BITCOIN_NETWORK);
    let signature = sign_input(tx, vin as usize, &redeem_script, priv_key.secret_key());
    signature
}

fn verify_anchoring_transaction(tx: &BitcoinTx,
                                redeem_script: &RedeemScript,
                                vin: u32,
                                pub_key: &str,
                                signature: &[u8])
                                -> bool {
    let pub_key = {
        let data = Vec::<u8>::from_hex(pub_key).unwrap();
        let context = Secp256k1::new();
        PublicKey::from_slice(&context, data.as_ref()).unwrap()
    };
    verify_input(tx, vin as usize, redeem_script, &pub_key, signature)
}

fn finalize_anchoring_transaction(mut anchoring_tx: AnchoringTx,
                                  redeem_script: &str,
                                  signatures: HashMap<u32, Vec<BitcoinSignature>>)
                                  -> AnchoringTx {
    // build scriptSig
    for (out, signatures) in signatures.into_iter() {
        anchoring_tx.0.input[out as usize].script_sig = {
            let redeem_script = Vec::<u8>::from_hex(&redeem_script).unwrap();

            let mut builder = Builder::new();
            builder = builder.push_opcode(All::OP_PUSHBYTES_0);
            for sign in &signatures {
                builder = builder.push_slice(sign.as_ref());
            }
            builder.push_slice(&redeem_script)
                .into_script()
        };
    }
    anchoring_tx
}

fn find_payload(tx: &BitcoinTx) -> Option<(Height, Hash)> {
    tx.output
        .get(ANCHORING_TX_DATA_OUTPUT as usize)
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

#[cfg(test)]
mod tests {
    extern crate blockchain_explorer;

    use std::collections::HashMap;

    use bitcoin::network::constants::Network;

    use exonum::crypto::{Hash, HexValue};

    use multisig::RedeemScript;
    use HexValue as HexValueEx;
    use transactions::{AnchoringTx, FundingTx, TransactionBuilder};

    #[test]
    fn test_anchoring_tx_sign() {
        let _ = blockchain_explorer::helpers::init_logger();

        let priv_keys = ["cVC9eJN5peJemWn1byyWcWDevg6xLNXtACjHJWmrR5ynsCu8mkQE",
                         "cMk66oMazTgquBVaBLHzDi8FMgAaRN3tSf6iZykf9bCh3D3FsLX1",
                         "cT2S5KgUQJ41G6RnakJ2XcofvoxK68L9B44hfFTnH4ddygaxi7rc",
                         "cRUKB8Nrhxwd5Rh6rcX3QK1h7FosYPw5uzEsuPpzLcDNErZCzSaj"];
        let pub_keys =
            ["03475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c".to_string(),
             "02a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0".to_string(),
             "0230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb49".to_string(),
             "036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e".to_string()];
        let redeem_script = RedeemScript::from_pubkeys(pub_keys.iter(), 3)
            .compressed(Network::Testnet);
        let redeem_script_hex = redeem_script.to_hex();

        let prev_tx = AnchoringTx::from_hex("01000000014970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd670100483045022100e6ef3de83437c8dc33a8099394b7434dfb40c73631fc4b0378bd6fb98d8f42b002205635b265f2bfaa6efc5553a2b9e98c2eabdfad8e8de6cdb5d0d74e37f1e198520147304402203bb845566633b726e41322743677694c42b37a1a9953c5b0b44864d9b9205ca10220651b7012719871c36d0f89538304d3f358da12b02dab2b4d74f2981c8177b69601473044022052ad0d6c56aa6e971708f079073260856481aeee6a48b231bc07f43d6b02c77002203a957608e4fbb42b239dd99db4e243776cc55ed8644af21fa80fd9be77a59a60014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02b80b00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280000000000000000f1cb806d27e367f1cac835c22c8cc24c402a019e2d3ea82f7f841c308d830a9600000000").unwrap();
        let funding_tx = FundingTx::from_hex("01000000019532a4022a22226a6f694c3f21216b2c9f5c1c79007eb7d3be06bc2f1f9e52fb000000006a47304402203661efd05ca422fad958b534dbad2e1c7db42bbd1e73e9b91f43a2f7be2f92040220740cf883273978358f25ca5dd5700cce5e65f4f0a0be2e1a1e19a8f168095400012102ae1b03b0f596be41a247080437a50f4d8e825b170770dcb4e5443a2eb2ecab2afeffffff02a00f00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678716e1ff05000000001976a91402f5d7475a10a9c24cea32575bd8993d3fabbfd388ac089e1000").unwrap();

        let tx = TransactionBuilder::with_prev_tx(&prev_tx, 0)
            .add_funds(&funding_tx, 0)
            .payload(10, Hash::from_hex("164d236bbdb766e64cec57847e3a0509d4fc77fa9c17b7e61e48f7a3eaa8dbc9").unwrap())
            .send_to(&redeem_script.to_address(Network::Testnet), 6000)
            .into_transaction();

        let mut signatures = HashMap::new();
        for input in tx.inputs() {
            let mut input_signs = Vec::new();
            for priv_key in priv_keys.iter() {
                let sign = tx.sign(&redeem_script_hex, input, priv_key);
                input_signs.push(sign);
            }
            signatures.insert(input, input_signs);
        }

        for (input, signs) in signatures.iter() {
            for (id, signature) in signs.iter().enumerate() {
                assert!(tx.verify(&redeem_script, *input, &pub_keys[id], signature.as_ref()));
            }
        }
    }
}