use std::collections::VecDeque;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use std::default::Default;
use std::ops::{Drop, Deref};

use bitcoinrpc::*;

use serde_json::value::{Value, ToJson, from_value};
use serde::Deserialize;

use super::config::AnchoringRpcConfig;

#[derive(Debug)]
pub struct Request {
    pub method: &'static str,
    pub params: Params,
    pub response: Result<Value>,
}

#[derive(Debug, Clone)]
pub struct SandboxClient {
    requests: Arc<Mutex<VecDeque<Request>>>,
    rpc: AnchoringRpcConfig,
}

impl SandboxClient {
    pub fn new<S>(host: S, username: Option<String>, password: Option<String>) -> SandboxClient
        where S: Into<String>
    {
        SandboxClient {
            rpc: AnchoringRpcConfig {
                host: host.into(),
                username: username,
                password: password,
            },
            requests: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn url(&self) -> &str {
        &self.rpc.host
    }
    pub fn password(&self) -> &Option<String> {
        &self.rpc.password
    }
    pub fn username(&self) -> &Option<String> {
        &self.rpc.username
    }

    fn request<T: Deserialize + ::std::fmt::Debug>(&self,
                                                   method: &str,
                                                   params: Params)
                                                   -> Result<T> {
        // TODO return error
        let expected = self.requests
            .lock()
            .unwrap()
            .pop_front()
            .expect(format!("expected response for method={}, params={:#?}",
                            method,
                            params)
                            .as_str());

        assert_eq!(expected.method, method);
        assert_eq!(expected.params,
                   params,
                   "Invalid params for method {}!",
                   method);

        let response = expected.response?;
        from_value(response).map_err(|e| Error::Other(RpcError::Json(e)))
    }
    pub fn expect<I: IntoIterator<Item = Request>>(&self, requests: I) {
        {
            let requests = self.requests.lock().unwrap();
            assert!(requests.is_empty(),
                    "Send unexpected requests: {:#?}",
                    requests.deref());
        }
        self.requests
            .lock()
            .unwrap()
            .extend(requests);
    }
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

impl Default for SandboxClient {
    fn default() -> SandboxClient {
        SandboxClient {
            requests: Arc::new(Mutex::new(VecDeque::new())),
            rpc: AnchoringRpcConfig {
                host: "127.0.0.1:1024".into(),
                username: None,
                password: None,
            },
        }
    }
}

impl Drop for SandboxClient {
    fn drop(&mut self) {
        if !::std::thread::panicking() {
            let requests = self.requests.lock().unwrap();
            if !requests.is_empty() {
                panic!("Expected requests: {:?}", requests.deref());
            }
        }
    }
}
