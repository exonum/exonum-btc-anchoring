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

use std::collections::VecDeque;
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
pub struct SandboxClient {
    requests: Arc<Mutex<VecDeque<Request>>>,
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
    where
        T: ::std::fmt::Debug,
        for<'de> T: Deserialize<'de>,
    {
        let expected = self.requests.lock().unwrap().pop_front().expect(
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

    pub fn expect<I: IntoIterator<Item = Request>>(&self, requests: I) {
        {
            let requests = self.requests.lock().unwrap();
            assert!(
                requests.is_empty(),
                "Send unexpected requests: {:#?}",
                requests.deref()
            );
        }
        self.requests.lock().unwrap().extend(requests);
    }

    fn sendtoaddress(&self, addr: &str, amount: &str) -> Result<String> {
        let params = vec![
            serde_json::to_value(addr).unwrap(),
            serde_json::to_value(amount).unwrap(),
        ];
        self.request("sendtoaddress", params)
    }

    fn getrawtransaction(&self, txid: &str) -> Result<String> {
        let params = json!([txid, 0]).as_array().cloned().unwrap();
        self.request("getrawtransaction", params)
    }

    fn getrawtransaction_verbose(&self, txid: &str) -> Result<RawTransactionInfo> {
        let params = json!([txid, 1]).as_array().cloned().unwrap();
        self.request("getrawtransaction", params)
    }

    fn sendrawtransaction(&self, txhex: &str) -> Result<String> {
        self.request(
            "sendrawtransaction",
            vec![serde_json::to_value(txhex).unwrap()],
        )
    }

    fn listunspent<'a, V: AsRef<[&'a str]>>(
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

    fn importaddress(&self, addr: &str, label: &str, rescan: bool, p2sh: bool) -> Result<()> {
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
                    TxKind::Anchoring(tx) => txs.push(TxInfo {
                        body: tx.into(),
                        confirmations,
                    }),
                    TxKind::FundingTx(tx) => txs.push(TxInfo {
                        body: tx.into(),
                        confirmations,
                    }),
                    TxKind::Other(_) => {}
                }
            }
        }
        Ok(txs)
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
