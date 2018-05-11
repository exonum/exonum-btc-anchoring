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

use std::string::ToString;

use bitcoinrpc;

use exonum::encoding::serialize::FromHex;

use details::btc;
use details::btc::transactions::{BitcoinTx, FundingTx, TxKind};

pub use bitcoinrpc::Client as RpcClient;

pub type Result<T> = bitcoinrpc::Result<T>;
pub type Error = bitcoinrpc::Error;

/// Number of satoshis in a bitcoin.
///
/// Used to convert values in satoshis for the bitcoind `sendtoaddress` RPC endpoint,
/// which measures amounts in bitcoins (rather than satoshis).
pub const SATOSHI_DIVISOR: f64 = 100_000_000.0;

/// `Bitcoind` rpc configuration.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AnchoringRpcConfig {
    /// Rpc url.
    pub host: String,
    /// Rpc username.
    pub username: Option<String>,
    /// Rpc password.
    pub password: Option<String>,
}

/// Client for the `Bitcoind` rpc api, for more information visit
/// this [site](https://en.bitcoin.it/wiki/Original_Bitcoin_client/API_calls_list).
#[derive(Debug)]
pub struct AnchoringRpc(pub RpcClient);

impl From<AnchoringRpcConfig> for RpcClient {
    fn from(cfg: AnchoringRpcConfig) -> Self {
        RpcClient::new(cfg.host, cfg.username, cfg.password)
    }
}

/// Short information about bitcoin transaction.
#[derive(Clone, Debug)]
pub struct TxInfo {
    /// Transaction body,
    pub body: BitcoinTx,
    /// Number of confirmations.
    pub confirmations: Option<u64>,
}

impl From<bitcoinrpc::RawTransactionInfo> for TxInfo {
    fn from(info: bitcoinrpc::RawTransactionInfo) -> Self {
        TxInfo {
            body: BitcoinTx::from_hex(info.hex.expect("Transaction hex is absent in response."))
                .unwrap(),
            confirmations: info.confirmations,
        }
    }
}

pub trait BitcoinRelay: 'static + ::std::fmt::Debug + Send + Sync {
    /// Retrieves transaction from the bitcoin blockchain.
    fn get_transaction(&self, txid: btc::TxId) -> Result<Option<BitcoinTx>>;

    /// Retrieves information about transaction with the given id.
    fn get_transaction_info(&self, txid: btc::TxId) -> Result<Option<TxInfo>>;

    /// Observes the changes on given address.
    fn watch_address(&self, addr: &btc::Address, rescan: bool) -> Result<()>;

    /// Sends raw transaction to the bitcoin network.
    fn send_transaction(&self, tx: BitcoinTx) -> Result<()>;

    /// Sends funds to the given address.
    fn send_to_address(&self, addr: &btc::Address, satoshis: u64) -> Result<FundingTx>;

    /// Lists unspent transactions for the given address.
    fn unspent_transactions(&self, addr: &btc::Address) -> Result<Vec<TxInfo>>;

    /// Retrieves information about confirmations for transaction with the given id.
    fn get_transaction_confirmations(&self, txid: btc::TxId) -> Result<Option<u64>> {
        let info = self.get_transaction_info(txid)?;
        Ok(info.and_then(|x| x.confirmations))
    }

    /// Returns an actual relay configuration.
    fn config(&self) -> AnchoringRpcConfig;
}

macro_rules! retry {
    ($expr:expr) => {{
        use std::time::Duration;
        use std::thread;

        let mut delay = Duration::from_millis(500);
        let delay_increment = Duration::from_millis(1000);

        let mut res = $expr;
        for _ in 0..5 {
            if res.is_ok() {
                break;
            }
            res = $expr;
            thread::sleep(delay);
            delay += delay_increment;
        }
        res
    }};
}

impl BitcoinRelay for RpcClient {
    fn get_transaction(&self, txid: btc::TxId) -> Result<Option<BitcoinTx>> {
        let r = retry!(self.getrawtransaction(&txid.to_string()));
        match r {
            Ok(tx) => Ok(Some(BitcoinTx::from_hex(tx).unwrap())),
            Err(bitcoinrpc::Error::NoInformation(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn get_transaction_info(&self, txid: btc::TxId) -> Result<Option<TxInfo>> {
        let info = match retry!(self.getrawtransaction_verbose(&txid.to_string())) {
            Ok(info) => Ok(info),
            Err(bitcoinrpc::Error::NoInformation(_)) => return Ok(None),
            Err(e) => Err(e),
        }?;
        Ok(Some(info.into()))
    }

    fn watch_address(&self, addr: &btc::Address, rescan: bool) -> Result<()> {
        retry!(self.importaddress(&addr.to_string(), "multisig", false, rescan))
    }

    fn send_transaction(&self, tx: BitcoinTx) -> Result<()> {
        let tx_hex = tx.to_hex();
        retry!(self.sendrawtransaction(&tx_hex).map(drop))
    }

    fn send_to_address(&self, addr: &btc::Address, satoshis: u64) -> Result<FundingTx> {
        let addr = addr.to_string();
        let funds_str = (satoshis as f64 / SATOSHI_DIVISOR).to_string();
        let utxo_txid = retry!(self.sendtoaddress(&addr, &funds_str))?;
        // TODO rewrite Error types to avoid unwraps.
        let utxo_txid = btc::TxId::from_hex(&utxo_txid).unwrap();
        Ok(FundingTx::from(self.get_transaction(utxo_txid)?.unwrap()))
    }

    fn unspent_transactions(&self, addr: &btc::Address) -> Result<Vec<TxInfo>> {
        let unspent_txs = retry!(self.listunspent(0, 9_999_999, &[addr.to_string()]))?;
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

    fn config(&self) -> AnchoringRpcConfig {
        AnchoringRpcConfig {
            host: self.url().to_string(),
            username: self.username().clone(),
            password: self.password().clone(),
        }
    }
}

impl<'a, T: BitcoinRelay + 'a> From<T> for Box<BitcoinRelay> {
    fn from(t: T) -> Self {
        Box::new(t) as Box<BitcoinRelay>
    }
}
