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

//! Helpers for the bitcoin rpc testing.

use bitcoin::util::address::Address;

use std::collections::VecDeque;

use exonum::crypto::Hash;

use btc;
use failure;
use std::sync::{Arc, Mutex};

use rpc::{BitcoinRpcConfig, BtcRelay, TransactionInfo as BtcTransactionInfo};

const UNEXPECTED_RESPONSE: &str = "Unexpected response. Error in test data.";

/// Possible rpc requests.
#[derive(Debug, PartialEq)]
pub enum FakeRelayRequest {
    /// Send some satoshis to the given address request.
    SendToAddress {
        /// Bitcoin address.
        addr: Address,
        /// Amount in satoshis.
        satoshis: u64,
    },
    /// Transaction information request.
    TransactionInfo {
        /// Transaction id.
        id: Hash,
    },
    /// Send transaction to bitcoin mempool request.
    SendTransaction {
        /// Raw bitcoin transaction
        transaction: btc::Transaction,
    },
    /// Observe changes on given address request.
    WatchAddress {
        /// Bitcoin address.
        addr: Address,
        /// Full blockchain rescan option.
        rescan: bool,
    },
}

/// Possible rpc responses.
#[derive(Debug)]
pub enum FakeRelayResponse {
    /// Response to the send to address request.
    SendToAddress(Result<btc::Transaction, failure::Error>),
    /// Response to the transaction info request.
    TransactionInfo(Result<Option<BtcTransactionInfo>, failure::Error>),
    /// Response to the send transaction request.
    SendTransaction(Result<Hash, failure::Error>),
    /// Response to the watch address request.
    WatchAddress(Result<(), failure::Error>),
}

/// Request response pair.
pub type TestRequest = (FakeRelayRequest, FakeRelayResponse);

/// Shared requests list.
#[derive(Debug, Clone, Default)]
pub struct TestRequests(Arc<Mutex<VecDeque<TestRequest>>>);

impl TestRequests {
    /// Creates a new shared requests instance.
    pub fn new() -> TestRequests {
        TestRequests(Arc::new(Mutex::new(VecDeque::new())))
    }

    /// The following requests are expecting
    pub fn expect(&self, requests: impl IntoIterator<Item = TestRequest>) {
        self.0.lock().unwrap().extend(requests);
    }
}

/// Fake btc relay client.
#[derive(Debug, Default)]
pub struct FakeBtcRelay {
    /// List of the expected requests.
    pub requests: TestRequests,
    rpc: BitcoinRpcConfig,
}

#[cfg_attr(feature = "cargo-clippy", allow(unused_variables))]
impl FakeBtcRelay {
    fn request(&self, request: &FakeRelayRequest) -> FakeRelayResponse {
        let (expected_request, response) = self
            .requests
            .0
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| panic!("expected request {:?}", request));

        assert_eq!(request, &expected_request);

        trace!("request: {:?}, response={:?}", expected_request, response);
        response
    }
}

impl BtcRelay for FakeBtcRelay {
    fn send_to_address(
        &self,
        addr: &Address,
        satoshis: u64,
    ) -> Result<btc::Transaction, failure::Error> {
        if let FakeRelayResponse::SendToAddress(r) =
            self.request(&FakeRelayRequest::SendToAddress {
                addr: addr.clone(),
                satoshis,
            }) {
            r
        } else {
            panic!(UNEXPECTED_RESPONSE);
        }
    }

    fn transaction_info(&self, id: &Hash) -> Result<Option<BtcTransactionInfo>, failure::Error> {
        if let FakeRelayResponse::TransactionInfo(r) =
            self.request(&FakeRelayRequest::TransactionInfo { id: *id })
        {
            r
        } else {
            panic!(UNEXPECTED_RESPONSE);
        }
    }

    fn send_transaction(&self, transaction: &btc::Transaction) -> Result<Hash, failure::Error> {
        if let FakeRelayResponse::SendTransaction(r) =
            self.request(&FakeRelayRequest::SendTransaction {
                transaction: transaction.clone(),
            }) {
            r
        } else {
            panic!(UNEXPECTED_RESPONSE);
        }
    }

    fn watch_address(&self, addr: &Address, rescan: bool) -> Result<(), failure::Error> {
        if let FakeRelayResponse::WatchAddress(r) = self.request(&FakeRelayRequest::WatchAddress {
            addr: addr.clone(),
            rescan,
        }) {
            r
        } else {
            panic!(UNEXPECTED_RESPONSE);
        }
    }

    fn config(&self) -> BitcoinRpcConfig {
        self.rpc.clone()
    }
}
