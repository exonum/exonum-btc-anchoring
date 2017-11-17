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

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::default::Default;
use std::ops::{Deref, Drop};

use bitcoinrpc::*;

use serde::Deserialize;
use serde_json;
use serde_json::value::{from_value, Value};

use exonum::encoding::serialize::HexValue;

use details::rpc::{AnchoringRpcConfig, BitcoinRelay, TxInfo, SATOSHI_DIVISOR};
use details::btc;
use details::btc::transactions::{BitcoinTx, FundingTx, TxKind};

#[derive(Debug)]
pub struct Request {
    pub method: &'static str,
    pub params: Params,
    pub response: Result<Value>,
}

#[derive(Debug, Clone)]
pub struct Requests(Arc<Mutex<VecDeque<Request>>>);

impl Requests {
    pub fn new() -> Requests {
        Requests(Arc::new(Mutex::new(VecDeque::new())))
    }

    pub fn expect<I: IntoIterator<Item = Request>>(&self, requests: I) {
        {
            let requests = self.0.lock().unwrap();
            assert!(
                requests.is_empty(),
                "Send unexpected requests: {:#?}",
                requests.deref()
            );
        }
        self.0.lock().unwrap().extend(requests);
    }
}

impl Default for Requests {
    fn default() -> Requests {
        Requests::new()
    }
}

#[derive(Debug, Clone)]
pub struct SandboxClient {
    requests: Requests,
    rpc: AnchoringRpcConfig,
}

impl SandboxClient {
    pub fn new<S>(host: S, username: Option<String>, password: Option<String>) -> SandboxClient
    where
        S: Into<String>,
    {
        SandboxClient {
            rpc: AnchoringRpcConfig {
                host: host.into(),
                username: username,
                password: password,
            },
            requests: Requests::default(),
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

    pub fn requests(&self) -> Requests {
        self.requests.clone()
    }

    fn request<T>(&self, method: &str, params: Params) -> Result<T>
    where
        T: ::std::fmt::Debug,
        for<'de> T: Deserialize<'de>,
    {
        let expected = self.requests.0.lock().unwrap().pop_front().expect(
            format!(
                "expected response for method={}, \
                 params={:#?}",
                method,
                params
            ).as_str(),
        );

        assert_eq!(expected.method, method);
        assert_eq!(
            expected.params,
            params,
            "Invalid params for method {}!",
            method
        );

        let response = expected.response?;
        trace!(
            "method: {}, params={:?}, respose={:#}",
            method,
            params,
            response
        );
        from_value(response).map_err(|e| Error::Other(RpcError::Json(e)))
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
        let params = vec![
            serde_json::to_value(addr).unwrap(),
            serde_json::to_value(amount).unwrap(),
        ];
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

    pub fn createrawtransaction<T, O>(
        &self,
        transactions: T,
        outputs: O,
        data: Option<String>,
    ) -> Result<String>
    where
        T: AsRef<[TransactionInput]>,
        O: AsRef<[TransactionOutput]>,
    {
        let mut map = BTreeMap::new();
        map.extend(outputs.as_ref().iter().map(|x| {
            (x.address.clone(), x.value.clone())
        }));
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

    pub fn signrawtransaction<O, K>(
        &self,
        txhex: &str,
        outputs: O,
        priv_keys: K,
    ) -> Result<SignTxOutput>
    where
        O: AsRef<[DependentOutput]>,
        K: AsRef<[String]>,
    {
        let params = json!([txhex, outputs.as_ref(), priv_keys.as_ref()])
            .as_array()
            .cloned()
            .unwrap();
        self.request("signrawtransaction", params)
    }

    pub fn sendrawtransaction(&self, txhex: &str) -> Result<String> {
        self.request(
            "sendrawtransaction",
            vec![serde_json::to_value(txhex).unwrap()],
        )
    }

    pub fn decoderawtransaction(&self, txhex: &str) -> Result<RawTransactionInfo> {
        self.request(
            "decoderawtransaction",
            vec![serde_json::to_value(txhex).unwrap()],
        )
    }

    pub fn addwitnessaddress(&self, addr: &str) -> Result<String> {
        self.request(
            "addwitnessaddress",
            vec![serde_json::to_value(addr).unwrap()],
        )
    }

    pub fn listtransactions(
        &self,
        count: u32,
        from: u32,
        include_watch_only: bool,
    ) -> Result<Vec<TransactionInfo>> {
        let params = json!(["*", count, from, include_watch_only])
            .as_array()
            .cloned()
            .unwrap();
        self.request("listtransactions", params)
    }

    pub fn listunspent<'a, V: AsRef<[&'a str]>>(
        &self,
        min_confirmations: u32,
        max_confirmations: u32,
        addresses: V,
    ) -> Result<Vec<UnspentTransactionInfo>> {
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

    pub fn generatetoaddress(
        &self,
        nblocks: u64,
        addr: &str,
        maxtries: u64,
    ) -> Result<Vec<String>> {
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

impl BitcoinRelay for SandboxClient {
    fn get_transaction(&self, txid: btc::TxId) -> Result<Option<BitcoinTx>> {
        let r = self.getrawtransaction(&txid.to_string());
        match r {
            Ok(tx) => Ok(Some(BitcoinTx::from_hex(tx).unwrap())),
            Err(Error::NoInformation(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn get_transaction_info(&self, txid: btc::TxId) -> Result<Option<TxInfo>> {
        let info = match self.getrawtransaction_verbose(&txid.to_string()) {
            Ok(info) => Ok(info),
            Err(Error::NoInformation(_)) => return Ok(None),
            Err(e) => Err(e),
        }?;
        Ok(Some(info.into()))
    }

    fn watch_address(&self, addr: &btc::Address, rescan: bool) -> Result<()> {
        self.importaddress(&addr.to_string(), "multisig", false, rescan)
    }

    fn send_transaction(&self, tx: BitcoinTx) -> Result<()> {
        let tx_hex = tx.to_string();
        self.sendrawtransaction(&tx_hex)?;
        Ok(())
    }

    fn send_to_address(&self, addr: &btc::Address, satoshis: u64) -> Result<FundingTx> {
        let addr = addr.to_string();
        let funds_str = (satoshis as f64 / SATOSHI_DIVISOR).to_string();
        let utxo_txid = self.sendtoaddress(&addr, &funds_str)?;
        // TODO rewrite Error types to avoid unwraps.
        let utxo_txid = btc::TxId::from_hex(&utxo_txid).unwrap();
        Ok(FundingTx::from(self.get_transaction(utxo_txid)?.unwrap()))
    }

    fn unspent_transactions(&self, addr: &btc::Address) -> Result<Vec<TxInfo>> {
        let unspent_txs = self.listunspent(0, 9_999_999, [addr.to_string().as_ref()])?;
        let mut txs = Vec::new();
        for info in unspent_txs {
            let txid = btc::TxId::from_hex(&info.txid).unwrap();
            let confirmations = Some(info.confirmations);
            if let Some(raw_tx) = self.get_transaction(txid)? {
                match TxKind::from(raw_tx) {
                    TxKind::Anchoring(tx) => {
                        txs.push(TxInfo {
                            body: tx.into(),
                            confirmations,
                        })
                    }
                    TxKind::FundingTx(tx) => {
                        txs.push(TxInfo {
                            body: tx.into(),
                            confirmations,
                        })
                    }
                    TxKind::Other(_) => {}
                }
            }
        }
        Ok(txs)
    }

    fn config(&self) -> AnchoringRpcConfig {
        self.rpc.clone()
    }
}

impl Default for SandboxClient {
    fn default() -> SandboxClient {
        SandboxClient {
            requests: Requests::default(),
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
            let requests = self.requests.0.lock().unwrap();
            if !requests.is_empty() {
                panic!("Expected requests: {:?}", requests.deref());
            }
        }
    }
}
