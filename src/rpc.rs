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

//! Collections of helpers for synchronization with the Bitcoin network.

use exonum::crypto::Hash;

use bitcoin::util::address::Address;
use crate::bitcoin_rpc;
use failure;
use hex::FromHex;

use crate::btc::Transaction;

/// Short information about bitcoin transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct TransactionInfo {
    /// Transaction content.
    pub content: Transaction,
    /// Number of confirmations.
    pub confirmations: u64,
}

/// Information provider about the Bitcoin network.
pub trait BtcRelay: Send + Sync + ::std::fmt::Debug {
    /// Sends funds to the given address.
    fn send_to_address(&self, addr: &Address, satoshis: u64)
        -> Result<Transaction, failure::Error>;
    /// Retrieves information about transaction with the given id.
    fn transaction_info(&self, id: &Hash) -> Result<Option<TransactionInfo>, failure::Error>;
    /// Sends raw transaction to the bitcoin network.
    fn send_transaction(&self, transaction: &Transaction) -> Result<Hash, failure::Error>;
    /// Observes the changes on given address.
    fn watch_address(&self, addr: &Address, rescan: bool) -> Result<(), failure::Error>;
    /// Returns an actual relay configuration.
    fn config(&self) -> BitcoinRpcConfig;
}

/// `Bitcoind` rpc configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BitcoinRpcConfig {
    /// Rpc url.
    pub host: String,
    /// Rpc username.
    pub username: Option<String>,
    /// Rpc password.
    pub password: Option<String>,
}

/// Number of satoshis in a bitcoin.
///
/// Used to convert values in satoshis for the bitcoind `sendtoaddress` RPC endpoint,
/// which measures amounts in bitcoins (rather than satoshis).
const SATOSHI_DIVISOR: f64 = 100_000_000.0;

/// Client for the `Bitcoind` rpc api.
#[derive(Debug)]
pub struct BitcoinRpcClient(bitcoin_rpc::Client);

impl BitcoinRpcClient {
    /// Creates a new rpc client for the given configuration.
    pub fn new(config: BitcoinRpcConfig) -> Self {
        let inner = bitcoin_rpc::Client::new(config.host, config.username, config.password);
        BitcoinRpcClient(inner)
    }
}

impl From<BitcoinRpcConfig> for BitcoinRpcClient {
    fn from(cfg: BitcoinRpcConfig) -> Self {
        Self::new(cfg)
    }
}

impl From<BitcoinRpcClient> for Box<dyn BtcRelay> {
    fn from(client: BitcoinRpcClient) -> Self {
        Box::new(client) as Self
    }
}

impl BtcRelay for BitcoinRpcClient {
    fn send_to_address(
        &self,
        addr: &Address,
        satoshis: u64,
    ) -> Result<Transaction, failure::Error> {
        let amount = satoshis as f64 / SATOSHI_DIVISOR;
        let txid = self
            .0
            .sendtoaddress(&addr.to_string(), &amount.to_string())?;
        let tx_hex = self.0.getrawtransaction(&txid)?;

        Transaction::from_hex(tx_hex).map_err(From::from)
    }

    fn transaction_info(&self, id: &Hash) -> Result<Option<TransactionInfo>, failure::Error> {
        let txid = id.to_hex();
        let txinfo = match self.0.getrawtransaction_verbose(&txid) {
            Ok(info) => info,
            Err(bitcoin_rpc::Error::NoInformation(_)) => return Ok(None),
            Err(e) => Err(e)?,
        };

        let tx_hex = txinfo
            .hex
            .ok_or_else(|| bitcoin_rpc::Error::NoInformation(txid))?;
        let content = Transaction::from_hex(tx_hex)?;
        // TODO Check attentively documentation of `getrawtransaction` rpc call.
        let confirmations = txinfo.confirmations.unwrap_or_default();

        Ok(Some(TransactionInfo {
            content,
            confirmations,
        }))
    }

    fn send_transaction(&self, transaction: &Transaction) -> Result<Hash, failure::Error> {
        let txid = transaction.id();
        self.0.sendrawtransaction(&transaction.to_string())?;
        Ok(txid)
    }

    fn watch_address(&self, addr: &Address, rescan: bool) -> Result<(), failure::Error> {
        self.0
            .importaddress(&addr.to_string(), "multisig", false, rescan)
            .map_err(From::from)
    }

    fn config(&self) -> BitcoinRpcConfig {
        BitcoinRpcConfig {
            host: self.0.url().to_string(),
            username: self.0.username().clone(),
            password: self.0.password().clone(),
        }
    }
}
