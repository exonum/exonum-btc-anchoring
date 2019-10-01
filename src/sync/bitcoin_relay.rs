// Copyright 2019 The Exonum Team
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

use bitcoin::util::address::Address;
use bitcoincore_rpc::{Auth, Error as RpcError, RawTx, RpcApi};
use exonum::crypto::Hash;
use failure;
use hex::FromHex;
use serde_derive::{Deserialize, Serialize};

use crate::btc::Transaction;

/// Short information about bitcoin transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct TransactionInfo {
    /// Transaction content.
    // pub content: Transaction,
    /// Number of confirmations.
    pub confirmations: u32,
}

/// Information provider about the Bitcoin network.
pub trait BtcRelay: Send + Sync + std::fmt::Debug {
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
    /// Bitcoin RPC url.
    pub host: String,
    /// Bitcoin RPC username.
    pub username: Option<String>,
    /// Bitcoin RPC password.
    pub password: Option<String>,
}

/// Number of satoshis in a bitcoin.
///
/// Used to convert values in satoshis for the bitcoind `sendtoaddress` RPC endpoint,
/// which measures amounts in bitcoins (rather than satoshis).
const SATOSHI_DIVISOR: f64 = 100_000_000.0;

/// Client for the `Bitcoind` rpc api.
#[derive(Debug)]
pub struct BitcoinRpcClient(bitcoincore_rpc::Client);

impl BitcoinRpcClient {
    /// Creates a new rpc client for the given configuration.
    pub fn new(config: BitcoinRpcConfig) -> Self {
        let inner = bitcoincore_rpc::Client::new(
            config.host,
            Auth::UserPass(
                config.username.unwrap_or_default(),
                config.password.unwrap_or_default(),
            ),
        )
        .unwrap();
        Self(inner)
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
        unimplemented!();
    }

    fn transaction_info(&self, id: &Hash) -> Result<Option<TransactionInfo>, failure::Error> {
        let blockchain_info = self.0.get_blockchain_info()?;
        // TODO Rewrite proper or use Sha256d directly.
        let txid = {
            use bitcoin_hashes::Hash;
            let mut bytes = [0_u8; 32];
            bytes.copy_from_slice(id.as_ref());
            bytes.reverse();
            bitcoin_hashes::sha256d::Hash::from_slice(&bytes).unwrap()
        };

        let result = self.0.get_raw_transaction_verbose(&txid, None);
        let result = match result {
            Ok(result) => result,
            Err(RpcError::JsonRpc(_)) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        // let content = Transaction::from_hex(result.hex)?;
        // TODO Check attentively documentation of `getrawtransaction` rpc call.
        let confirmations = result.confirmations.unwrap_or_default();
        Ok(Some(TransactionInfo {
            // content,
            confirmations,
        }))
    }

    fn send_transaction(&self, transaction: &Transaction) -> Result<Hash, failure::Error> {
        let txid = transaction.id();
        self.0.send_raw_transaction(transaction.to_string())?;
        Ok(txid)
    }

    fn watch_address(&self, addr: &Address, rescan: bool) -> Result<(), failure::Error> {
        unimplemented!();
    }

    fn config(&self) -> BitcoinRpcConfig {
        unimplemented!();
    }
}
