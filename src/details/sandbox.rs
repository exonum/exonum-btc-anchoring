use std::collections::VecDeque;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use std::default::Default;
use std::ops::{Deref, Drop};

use bitcoinrpc::*;

use serde::Deserialize;
use serde_json;
use serde_json::value::{Value, from_value};

use details::rpc::AnchoringRpcConfig;

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

    fn request<T>(&self, method: &str, params: Params) -> Result<T>
        where T: ::std::fmt::Debug,
              for<'de> T: Deserialize<'de>
    {
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
        trace!("method: {}, params={:?}, respose={:#}",
               method,
               params,
               response);
        from_value(response).map_err(|e| Error::Other(RpcError::Json(e)))
    }

    pub fn expect<I: IntoIterator<Item = Request>>(&self, requests: I) {
        {
            let requests = self.requests.lock().unwrap();
            assert!(requests.is_empty(),
                    "Send unexpected requests: {:#?}",
                    requests.deref());
        }
        self.requests.lock().unwrap().extend(requests);
    }

    pub fn getinfo(&self) -> Result<Info> {
        self.request("getinfo", Vec::new())
    }
    pub fn getnewaddress(&self, account: &str) -> Result<String> {
        self.request("getnewaddress", vec![Value::String(account.to_owned())])
    }
    pub fn validateaddress(&self, addr: &str) -> Result<AddressInfo> {
        self.request("validateaddress", vec![Value::String(addr.to_owned())])
    }
    pub fn createmultisig<V: AsRef<[String]>>(&self, signs: u8, addrs: V) -> Result<MultiSig> {
        let n = serde_json::to_value(signs).unwrap();
        let addrs = serde_json::to_value(addrs.as_ref()).unwrap();
        self.request("createmultisig", vec![n, addrs])
    }
    pub fn sendtoaddress(&self, addr: &str, amount: &str) -> Result<String> {
        let params =
            vec![serde_json::to_value(addr).unwrap(), serde_json::to_value(amount).unwrap()];
        self.request("sendtoaddress", params)
    }
    pub fn getrawtransaction(&self, txid: &str) -> Result<String> {
        let params = json!([txid, 0]).as_array().cloned().unwrap();
        self.request("getrawtransaction", params)
    }
    pub fn getrawtransaction_verbose(&self, txid: &str) -> Result<RawTransactionInfo> {
        let params = json!([txid, 1]).as_array().cloned().unwrap();
        self.request("getrawtransaction", params)
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
        map.extend(outputs
                       .as_ref()
                       .iter()
                       .map(|x| (x.address.clone(), x.value.clone())));
        if let Some(data) = data {
            map.insert("data".into(), data);
        }

        let params = json!([transactions.as_ref(), map])
            .as_array()
            .cloned()
            .unwrap();
        self.request("createrawtransaction", params)
    }
    pub fn dumpprivkey(&self, pub_key: &str) -> Result<String> {
        let params = json!([pub_key]).as_array().cloned().unwrap();
        self.request("dumpprivkey", params)
    }
    pub fn signrawtransaction<O, K>(&self,
                                    txhex: &str,
                                    outputs: O,
                                    priv_keys: K)
                                    -> Result<SignTxOutput>
        where O: AsRef<[DependentOutput]>,
              K: AsRef<[String]>
    {
        let params = json!([txhex, outputs.as_ref(), priv_keys.as_ref()])
            .as_array()
            .cloned()
            .unwrap();
        self.request("signrawtransaction", params)
    }
    pub fn sendrawtransaction(&self, txhex: &str) -> Result<String> {
        self.request("sendrawtransaction",
                     vec![serde_json::to_value(txhex).unwrap()])
    }
    pub fn decoderawtransaction(&self, txhex: &str) -> Result<RawTransactionInfo> {
        self.request("decoderawtransaction",
                     vec![serde_json::to_value(txhex).unwrap()])
    }
    pub fn addwitnessaddress(&self, addr: &str) -> Result<String> {
        self.request("addwitnessaddress",
                     vec![serde_json::to_value(addr).unwrap()])
    }
    pub fn listtransactions(&self,
                            count: u32,
                            from: u32,
                            include_watch_only: bool)
                            -> Result<Vec<TransactionInfo>> {
        let params = json!(["*", count, from, include_watch_only])
            .as_array()
            .cloned()
            .unwrap();
        self.request("listtransactions", params)
    }
    pub fn listunspent<'a, V: AsRef<[&'a str]>>(&self,
                                                min_confirmations: u32,
                                                max_confirmations: u32,
                                                addresses: V)
                                                -> Result<Vec<UnspentTransactionInfo>> {
        let params = json!([min_confirmations, max_confirmations, addresses.as_ref()])
            .as_array()
            .cloned()
            .unwrap();
        self.request("listunspent", params)

    }
    pub fn importaddress(&self, addr: &str, label: &str, rescan: bool, p2sh: bool) -> Result<()> {
        let params = json!([addr, label, rescan, p2sh])
            .as_array()
            .cloned()
            .unwrap();
        // special case for decode {"result":null}
        let r: Result<Option<bool>> = self.request("importaddress", params);
        match r {
            Ok(_) => Ok(()),
            Err(Error::Other(RpcError::NoErrorOrResult)) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn generate(&self, nblocks: u64, maxtries: u64) -> Result<Vec<String>> {
        let params = json!([nblocks, maxtries]).as_array().cloned().unwrap();
        self.request("generate", params)
    }

    pub fn generatetoaddress(&self,
                             nblocks: u64,
                             addr: &str,
                             maxtries: u64)
                             -> Result<Vec<String>> {
        let params = json!([nblocks, addr, maxtries])
            .as_array()
            .cloned()
            .unwrap();
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
