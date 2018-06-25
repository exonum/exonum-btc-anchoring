// Copyright 2018 The Exonum Team
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

use bitcoin::util::address::Address;

use std::collections::VecDeque;

use exonum::crypto::Hash;

use btc;
use failure;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::value::{from_value, to_value, Value};

use bitcoin_rpc;

// use rand::{thread_rng, Rng, SeedableRng, tdRng};
use exonum::encoding::serialize::FromHex;

use rpc::{BitcoinRpcConfig, BtcRelay, TransactionInfo as BtcTransactionInfo};

const SATOSHI_DIVISOR: f64 = 100_000_000.0;

#[derive(Debug)]
pub struct TestRequest {
    pub method: &'static str,
    pub params: bitcoin_rpc::Params,
    pub response: bitcoin_rpc::Result<Value>,
}

#[derive(Debug, Clone, Default)]
pub struct TestRequests(Arc<Mutex<VecDeque<TestRequest>>>);

impl TestRequests {
    pub fn new() -> TestRequests {
        TestRequests(Arc::new(Mutex::new(VecDeque::new())))
    }

    pub fn expect<I: IntoIterator<Item = TestRequest>>(&self, requests: I) {
        self.0.lock().unwrap().extend(requests);
    }
}

#[derive(Debug)]
pub struct FakeBitcoinRpcClient {
    pub requests: TestRequests,
    rpc: BitcoinRpcConfig,
}

impl FakeBitcoinRpcClient {
    pub fn new() -> Self {
        Self {
            requests: TestRequests::new(),
            rpc: BitcoinRpcConfig {
                host: String::from("http://127.0.0.1:1234"),
                username: None,
                password: None,
            },
        }
    }

    fn request<T, P>(&self, method: &str, params: P) -> bitcoin_rpc::Result<T>
    where
        T: ::std::fmt::Debug,
        P: AsRef<bitcoin_rpc::Params>,
        for<'de> T: Deserialize<'de>,
    {
        let params = params.as_ref();
        let expected = self.requests.0.lock().unwrap().pop_front().expect(
            format!(
                "expected response for method={}, \
                 params={:#?}",
                method, params
            ).as_str(),
        );

        assert_eq!(expected.method, method);
        assert_eq!(
            &expected.params, params,
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
        from_value(response).map_err(|e| bitcoin_rpc::Error::Rpc(bitcoin_rpc::RpcError::Json(e)))
    }

    pub fn sendrawtransaction(&self, txhex: &str) -> bitcoin_rpc::Result<Hash> {
        self.request("sendrawtransaction", vec![to_value(txhex).unwrap()])
    }

    pub fn getrawtransaction(&self, txid: &str) -> bitcoin_rpc::Result<String> {
        let params = json!([txid, 0]).as_array().cloned().unwrap();
        self.request("getrawtransaction", params)
    }

    pub fn getrawtransaction_verbose(
        &self,
        txid: &str,
    ) -> bitcoin_rpc::Result<bitcoin_rpc::RawTransactionInfo> {
        let params = json!([txid, 1]).as_array().cloned().unwrap();
        self.request("getrawtransaction", params)
    }

    pub fn sendtoaddress(&self, addr: &str, amount: &str) -> bitcoin_rpc::Result<String> {
        let params = vec![to_value(addr).unwrap(), to_value(amount).unwrap()];
        self.request("sendtoaddress", params)
    }
}

impl Default for FakeBitcoinRpcClient {
    fn default() -> Self {
        Self::new()
    }
}

impl From<FakeBitcoinRpcClient> for Box<BtcRelay> {
    fn from(client: FakeBitcoinRpcClient) -> Self {
        Box::new(client) as Box<BtcRelay>
    }
}

impl BtcRelay for FakeBitcoinRpcClient {
    fn send_to_address(
        &self,
        addr: &Address,
        satoshis: u64,
    ) -> Result<btc::Transaction, failure::Error> {
        let addr = addr.to_string();
        let funds_str = (satoshis as f64 / SATOSHI_DIVISOR).to_string();

        let txid = self.sendtoaddress(&addr, &funds_str)?;

        let tx_hex = self.getrawtransaction(&txid)?;
        btc::Transaction::from_hex(tx_hex).map_err(From::from)
    }

    fn transaction_info(&self, id: &Hash) -> Result<Option<BtcTransactionInfo>, failure::Error> {
        let txid = id.to_string();

        let response = self.getrawtransaction_verbose(&txid);

        let txinfo = match response {
            Ok(info) => info,
            Err(bitcoin_rpc::Error::NoInformation(_)) => return Ok(None),
            Err(e) => Err(e)?,
        };
        let tx_hex = txinfo
            .hex
            .ok_or_else(|| bitcoin_rpc::Error::NoInformation(txid))?;

        let content = btc::Transaction::from_hex(tx_hex)?;
        let confirmations = txinfo.confirmations.unwrap_or_default();

        Ok(Some(BtcTransactionInfo {
            content,
            confirmations,
        }))
    }

    fn send_transaction(&self, transaction: &btc::Transaction) -> Result<Hash, failure::Error> {
        let tx_hex = transaction.to_string();
        self.sendrawtransaction(&tx_hex).map_err(From::from)
    }

    fn watch_address(&self, addr: &Address, rescan: bool) -> Result<(), failure::Error> {
        debug!("watch address {:?}, rescan {}", addr, rescan);
        Ok(())
    }

    fn config(&self) -> BitcoinRpcConfig {
        self.rpc.clone()
    }
}

#[macro_export]
macro_rules! request {
    (
        method: $method:expr,
        params: [$($params:tt)+]
    ) => {
        $crate::test_helpers::rpc::TestRequest {
            method: $method,
            params: json!([$($params)+]).as_array().unwrap().clone(),
            response: Ok(::serde_json::Value::Null)
        }
    };
    (
        method: $method:expr,
        params: [$($params:tt)+],
        response: $($response:tt)+
    ) => {
        $crate::test_helpers::rpc::TestRequest {
            method: $method,
            params: json!([$($params)+]).as_array().unwrap().clone(),
            response: Ok(json!($($response)+)),
        }
    };
    (
        method: $method:expr,
        params: [$($params:tt)+],
        error: $($err:tt)+
    ) => {
        $crate::test_helpers::rpc::TestRequest {
            method: $method,
            params: json!([$($params)+]).as_array().unwrap().clone(),
            response: Err($($err)+)
        }
    };
}
