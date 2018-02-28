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

use exonum::encoding::serialize::FromHex;

use exonum_btc_anchoring::details::rpc::{AnchoringRpcConfig, BitcoinRelay, TxInfo, SATOSHI_DIVISOR};
use exonum_btc_anchoring::details::btc;
use exonum_btc_anchoring::details::btc::transactions::{BitcoinTx, FundingTx, TxKind};

#[derive(Debug)]
pub struct TestRequest {
    pub method: &'static str,
    pub params: Params,
    pub response: Result<Value>,
}

#[derive(Debug, Clone)]
pub struct TestRequests(Arc<Mutex<VecDeque<TestRequest>>>);

impl TestRequests {
    pub fn new() -> TestRequests {
        TestRequests(Arc::new(Mutex::new(VecDeque::new())))
    }

    pub fn expect<I: IntoIterator<Item = TestRequest>>(&self, requests: I) {
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

impl Default for TestRequests {
    fn default() -> TestRequests {
        TestRequests::new()
    }
}

#[derive(Debug, Clone)]
pub struct TestClient {
    requests: TestRequests,
    rpc: AnchoringRpcConfig,
}

impl TestClient {
    pub fn requests(&self) -> TestRequests {
        self.requests.clone()
    }

    fn request<T, P>(&self, method: &str, params: P) -> Result<T>
    where
        T: ::std::fmt::Debug,
        P: AsRef<Params>,
        for<'de> T: Deserialize<'de>,
    {
        let params = params.as_ref();
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
            &expected.params,
            params,
            "Invalid params for method {}!",
            method
        );

        let response = expected.response?;
        trace!(
            "method: {}, params={:?}, response={:#}",
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

    pub fn sendrawtransaction(&self, txhex: &str) -> Result<String> {
        self.request(
            "sendrawtransaction",
            vec![serde_json::to_value(txhex).unwrap()],
        )
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
            Ok(_) |
            Err(Error::Other(RpcError::NoErrorOrResult)) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

impl BitcoinRelay for TestClient {
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

impl Default for TestClient {
    fn default() -> TestClient {
        TestClient {
            requests: TestRequests::default(),
            rpc: AnchoringRpcConfig {
                host: "127.0.0.1:1024".into(),
                username: None,
                password: None,
            },
        }
    }
}

impl Drop for TestClient {
    fn drop(&mut self) {
        if !::std::thread::panicking() {
            let requests = self.requests.0.lock().unwrap();
            if !requests.is_empty() {
                panic!("Unexpected requests: {:?}", requests.deref());
            }
        }
    }
}

#[test]
fn test_rpc_getnewaddress() {
    let client = TestClient::default();
    client.requests().expect(vec![
        request! {
            method: "getnewaddress",
            params: ["maintain"],
            response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
        },
    ]);
    let addr = client.getnewaddress("maintain").unwrap();
    assert_eq!(addr, "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY");
}

#[test]
#[should_panic(expected = "expected response for method=getnewaddress")]
fn test_rpc_expected_request() {
    let client = TestClient::default();
    client.getnewaddress("userid").unwrap();
}

#[test]
#[should_panic(expected = "assertion failed")]
fn test_rpc_wrong_request() {
    let client = TestClient::default();
    client.requests().expect(vec![
        request! {
            method: "getnewaddress",
            params: ["maintain"],
            response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
        },
    ]);
    client.getnewaddress("userid").unwrap();
}

#[test]
#[should_panic(expected = "assertion failed")]
fn test_rpc_unexpected_request() {
    let client = TestClient::default();
    client.requests().expect(vec![
        request! {
            method: "getnewaddress",
            params: ["maintain"],
            response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
        },
        request! {
            method: "getnewaddress",
            params: ["maintain2"],
            response: "mmoXxKhBwnhtFiAMvxJ82CKCBia751mzfY"
        },
    ]);
    client.getnewaddress("userid").unwrap();
    client.requests().expect(vec![
        request! {
            method: "getnewaddress",
            params: ["maintain"],
            response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
        },
    ]);
}

#[test]
fn test_rpc_validateaddress() {
    let client = TestClient::default();
    client.requests().expect(vec![
        request! {
            method: "validateaddress",
            params: ["n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb"],
            response: {
                "account": "node_0",
                "address": "n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb",
                "hdkeypath": "m/0'/0'/1023'",
                "hdmasterkeyid": "e2aabb596d105e11c1838c0b6bede91e1f2a95ee",
                "iscompressed": true,
                "ismine": true,
                "isscript": false,
                "isvalid": true,
                "iswatchonly": false,
                "pubkey": "0394a06ac465776c110cb43d530663d7e7df5684013075988917f02f\
                            f007edd364",
                "scriptPubKey": "76a914e7588549f0c4149e7949cd7ea933cfcdde45f8c888ac"
            }
        },
    ]);
    client
        .validateaddress("n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb")
        .unwrap();
}
