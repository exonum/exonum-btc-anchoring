extern crate jsonrpc_v1;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate log;

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::value::{Value, ToJson};

use jsonrpc_v1::client::Client as RpcClient;
pub use jsonrpc_v1::error::Error as RpcError;

#[derive(Debug)]
pub enum Error {
    NoInformation(String),
    Memory(String),
    TransactionIncorrect(String),
    TransactionRejected(String),
    InsufficientFunds,
    TransactionAlreadyInChain,
    Other(RpcError),
}

pub type Result<T> = ::std::result::Result<T, Error>;
pub type Params = Vec<Value>;

impl Error {
    pub fn incorrect_transaction<S: Into<String>>(s: S) -> Error {
        Error::TransactionIncorrect(s.into())
    }
}

impl From<RpcError> for Error {
    fn from(e: RpcError) -> Error {
        match e {
            jsonrpc_v1::Error::Rpc(value) => {
                if let Some(code) = value.find("code").and_then(Value::as_i64) {
                    let msg = value.find("message")
                        .and_then(Value::as_str)
                        .unwrap_or_else(|| "")
                        .into();

                    match code {
                        -5 => return Error::NoInformation(msg),
                        -6 => return Error::InsufficientFunds,
                        -7 => return Error::Memory(msg),
                        -25 => return Error::TransactionIncorrect(msg),
                        -26 => return Error::TransactionRejected(msg),
                        -27 => return Error::TransactionAlreadyInChain,
                        _ => {}
                    }
                }
                Error::Other(RpcError::Rpc(value))
            }
            e @ _ => Error::Other(e),
        }
    }
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            Error::NoInformation(ref msg) => write!(f, "{}", msg),
            Error::Memory(ref msg) => write!(f, "{}", msg),         
            Error::TransactionRejected(ref msg) => write!(f, "{}", msg),         
            Error::TransactionIncorrect(ref msg) => write!(f, "{}", msg),         
            Error::InsufficientFunds => write!(f, "Insufficient funds"),
            Error::TransactionAlreadyInChain => write!(f, "Transaction already in chain"),
            Error::Other(ref e) => write!(f, "JsonRpc error: {}", e),
        }
    }
}

impl ::std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::NoInformation(ref msg) => &msg,
            Error::Memory(ref msg) => &msg,
            Error::TransactionRejected(ref msg) => &msg,
            Error::TransactionIncorrect(ref msg) => &msg,
            Error::InsufficientFunds => "Insufficient funds",
            Error::TransactionAlreadyInChain => "Transaction already in chain",
            Error::Other(_) => "Rpc error",
        }
    }

    fn cause(&self) -> Option<&::std::error::Error> {
        match *self {
            Error::Other(ref e) => Some(e),
            _ => None,
        }
    }
}

pub struct Client {
    inner: RpcClient,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("BitcoinRpcClient")
            .finish()
    }
}

#[derive(Clone, Deserialize, Debug)]
pub struct Info {
    pub version: u32,
    pub protocolversion: u32,
    pub walletversion: u32,
    pub balance: f64,
    pub blocks: u64,
    pub timeoffset: u64,
    pub connections: u32,
    pub proxy: String,
    pub difficulty: f64,
    pub testnet: bool,
    pub keypoololdest: u64,
    pub keypoolsize: u64,
    pub paytxfee: f64,
    pub relayfee: f64,
    pub errors: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct AddressInfo {
    pub isvalid: bool,
    pub address: String,
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: String,
    pub ismine: bool,
    pub iswatchonly: bool,
    pub isscript: bool,
    pub pubkey: String,
    pub iscompressed: bool,
    pub account: String,
    pub hdkeypath: String,
    pub hdmasterkeyid: String,
}

#[derive(Clone, Deserialize, Debug, PartialEq)]
pub struct MultiSig {
    pub address: String,
    #[serde(rename = "redeemScript")]
    pub redeem_script: String,
}

#[derive(Clone, Deserialize, Debug, PartialEq)]
pub struct ScriptSig {
    pub asm: String,
    pub hex: String,
}

#[derive(Clone, Deserialize, Debug, PartialEq)]
pub struct ScriptPubKey {
    pub asm: String,
    pub hex: String,
    #[serde(rename = "reqSigs")]
    pub req_sigs: Option<u64>,
    #[serde(rename = "type")]
    pub key_type: String,
    pub addresses: Option<Vec<String>>,
}

// TODO use TxIn from bitcoin crate
#[derive(Clone, Deserialize, Debug)]
pub struct TxIn {
    pub txid: String,
    pub vout: u32,
    #[serde(rename = "scriptSig")]
    pub script_sig: ScriptSig,
    pub sequence: u64,
    pub txinwitness: Option<Vec<String>>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct TxOut {
    pub value: f64,
    pub n: u32,
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: ScriptPubKey,
}

#[derive(Clone, Deserialize, Debug)]
pub struct RawTransactionInfo {
    pub hex: Option<String>,
    pub txid: String,
    pub hash: String,
    pub size: u64,
    pub vsize: u64,
    pub version: u32,
    pub locktime: u32,
    pub vin: Vec<TxIn>,
    pub vout: Vec<TxOut>,
    pub confirmations: Option<u64>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct UnspentTransactionInfo {
    pub txid: String,
    pub vout: u32,
    pub confirmations: u64,
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: String,
    pub amount: f64,
    #[serde(rename = "redeemScript")]
    pub redeem_script: Option<String>,
    pub spendable: bool,
    pub solvable: bool,
}

#[derive(Clone, Serialize, Debug)]
pub struct DependentOutput {
    pub txid: String,
    pub vout: u32,
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: String,
    #[serde(rename = "redeemScript")]
    pub redeem_script: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct SignTxOutput {
    pub hex: String,
    pub complete: bool,
}

#[derive(Clone, Serialize, Debug)]
pub struct TransactionInput {
    pub txid: String,
    pub vout: u32,
    pub sequence: Option<u64>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct TransactionOutput {
    pub address: String,
    pub value: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct TransactionInfo {
    pub address: Option<String>,
    pub vout: u32,
    pub confirmations: u64,
    pub txid: String,
    pub abandoned: Option<bool>,
    pub time: u64,
}

#[derive(Debug)]
struct RpcRequest {
    method: String,
    params: Params,
    response: Result<Value>,
}

impl Client {
    pub fn new<S>(url: S, user: Option<String>, password: Option<String>) -> Client
        where S: Into<String>
    {
        Client { inner: RpcClient::new(url.into(), user.map(Into::into), password.map(Into::into)) }
    }

    pub fn url(&self) -> &str {
        self.inner.url()
    }
    pub fn password(&self) -> &Option<String> {
        self.inner.password()
    }
    pub fn username(&self) -> &Option<String> {
        self.inner.username()
    }

    fn request<T: Deserialize>(&self, method: &str, params: Params) -> Result<T> {
        let request = self.inner.build_request(method.into(), params);
        let response = self.inner.send_request(&request)?;
        trace!("{:#?}",
               RpcRequest {
                   method: request.method.clone(),
                   params: request.params.clone(),
                   response: response.clone().into_result::<Value>().map_err(Error::from),
               });
        response.into_result::<T>().map_err(Error::from)
    }
}

// public api part
impl Client {
    pub fn getinfo(&self) -> Result<Info> {
        self.request("getinfo", Vec::new())
    }
    pub fn getnewaddress(&self, account: &str) -> Result<String> {
        self.request("getnewaddress", vec![account.to_json()])
    }
    pub fn validateaddress(&self, addr: &str) -> Result<AddressInfo> {
        self.request("validateaddress", vec![addr.to_json()])
    }
    pub fn createmultisig<V: AsRef<[String]>>(&self, signs: u8, addrs: V) -> Result<MultiSig> {
        let n = signs.to_json();
        let addrs = addrs.as_ref().to_json();
        self.request("createmultisig", vec![n, addrs])
    }
    pub fn sendtoaddress(&self, addr: &str, amount: &str) -> Result<String> {
        self.request("sendtoaddress", vec![addr.to_json(), amount.to_json()])
    }
    pub fn getrawtransaction(&self, txid: &str) -> Result<String> {
        self.request("getrawtransaction", vec![txid.to_json(), 0.to_json()])
    }
    pub fn getrawtransaction_verbose(&self, txid: &str) -> Result<RawTransactionInfo> {
        self.request("getrawtransaction", vec![txid.to_json(), 1.to_json()])
    }
    pub fn createrawtransaction<T, O>(&self,
                                      transactions: T,
                                      outputs: O,
                                      data: Option<String>)
                                      -> Result<String>
        where T: AsRef<[TransactionInput]>,
              O: AsRef<[TransactionOutput]>
    {
        let mut map = BTreeMap::new();
        map.extend(outputs.as_ref().iter().map(|x| (x.address.clone(), x.value.clone())));
        if let Some(data) = data {
            map.insert("data".into(), data);
        }

        let params = vec![transactions.as_ref().to_json(), map.to_json()];
        self.request("createrawtransaction", params)
    }
    pub fn dumpprivkey(&self, pub_key: &str) -> Result<String> {
        self.request("dumpprivkey", vec![pub_key.to_json()])
    }
    pub fn signrawtransaction<O, K>(&self,
                                    txhex: &str,
                                    outputs: O,
                                    priv_keys: K)
                                    -> Result<SignTxOutput>
        where O: AsRef<[DependentOutput]>,
              K: AsRef<[String]>
    {
        let params =
            vec![txhex.to_json(), outputs.as_ref().to_json(), priv_keys.as_ref().to_json()];
        self.request("signrawtransaction", params)
    }
    pub fn sendrawtransaction(&self, txhex: &str) -> Result<String> {
        self.request("sendrawtransaction", vec![txhex.to_json()])
    }
    pub fn decoderawtransaction(&self, txhex: &str) -> Result<RawTransactionInfo> {
        self.request("decoderawtransaction", vec![txhex.to_json()])
    }
    pub fn addwitnessaddress(&self, addr: &str) -> Result<String> {
        self.request("addwitnessaddress", vec![addr.to_json()])
    }
    pub fn listtransactions(&self,
                            count: u32,
                            from: u32,
                            include_watch_only: bool)
                            -> Result<Vec<TransactionInfo>> {
        let params =
            vec!["*".to_json(), count.to_json(), from.to_json(), include_watch_only.to_json()];
        self.request("listtransactions", params)
    }
    pub fn listunspent<'a, V: AsRef<[&'a str]>>(&self,
                                                min_confirmations: u32,
                                                max_confirmations: u32,
                                                addresses: V)
                                                -> Result<Vec<UnspentTransactionInfo>> {
        let params = vec![min_confirmations.to_json(),
                          max_confirmations.to_json(),
                          addresses.as_ref().to_json()];
        self.request("listunspent", params)

    }
    pub fn importaddress(&self, addr: &str, label: &str, rescan: bool, p2sh: bool) -> Result<()> {
        let params = vec![addr.to_json(), label.to_json(), rescan.to_json(), p2sh.to_json()];
        // special case for decode {"result":null}
        let r: Result<Option<bool>> = self.request("importaddress", params);
        match r {
            Ok(_) => Ok(()),
            Err(Error::Other(RpcError::NoErrorOrResult)) => Ok(()),
            Err(e) => Err(e), 
        }
    }

    pub fn generate(&self, nblocks: u64, maxtries: u64) -> Result<Vec<String>> {
        let params = vec![nblocks.to_json(), maxtries.to_json()];
        self.request("generate", params)
    }

    pub fn generatetoaddress(&self,
                             nblocks: u64,
                             addr: &str,
                             maxtries: u64)
                             -> Result<Vec<String>> {
        let params = vec![nblocks.to_json(), addr.to_json(), maxtries.to_json()];
        self.request("generatetoaddress", params)
    }

    pub fn stop(&self) -> Result<String> {
        self.request("stop", vec![])
    }
}
