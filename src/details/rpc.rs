use std::ops::Deref;

use bitcoinrpc;
use bitcoin::util::base58::ToBase58;

use exonum::crypto::HexValue;

use details::btc;
use details::btc::transactions::{BitcoinTx, TxKind};

#[doc(hidden)]
#[cfg(not(feature="sandbox_tests"))]
pub use bitcoinrpc::Client as RpcClient;
#[cfg(feature="sandbox_tests")]
pub use super::sandbox::SandboxClient as RpcClient;

pub type Result<T> = bitcoinrpc::Result<T>;
pub type Error = bitcoinrpc::Error;

// Rpc method `sendtoaddress` uses amount in btc instead of the satoshis.
const SATOSHI_DIVISOR: f64 = 100_000_000.0;

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

impl AnchoringRpc {
    pub fn new(cfg: AnchoringRpcConfig) -> AnchoringRpc {
        AnchoringRpc(RpcClient::new(cfg.host, cfg.username, cfg.password))
    }

    pub fn config(&self) -> AnchoringRpcConfig {
        AnchoringRpcConfig {
            host: self.0.url().to_string(),
            username: self.0.username().clone(),
            password: self.0.password().clone(),
        }
    }

    pub fn get_transaction(&self, txid: &str) -> Result<Option<BitcoinTx>> {
        let r = self.0.getrawtransaction(txid);
        match r {
            Ok(tx) => Ok(Some(BitcoinTx::from_hex(tx).unwrap())),
            Err(bitcoinrpc::Error::NoInformation(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_transaction_info(&self,
                                txid: &str)
                                -> Result<Option<bitcoinrpc::RawTransactionInfo>> {
        let r = self.0.getrawtransaction_verbose(txid);
        match r {
            Ok(tx) => Ok(Some(tx)),
            Err(bitcoinrpc::Error::NoInformation(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_transaction_confirmations(&self, txid: &btc::TxId) -> Result<Option<u64>> {
        let info = self.get_transaction_info(&txid.be_hex_string())?;
        Ok(info.and_then(|info| info.confirmations))
    }

    pub fn send_transaction(&self, tx: BitcoinTx) -> Result<()> {
        let tx_hex = tx.to_hex();
        self.0.sendrawtransaction(&tx_hex)?;
        Ok(())
    }

    pub fn send_to_address(&self, address: &btc::Address, funds: u64) -> Result<BitcoinTx> {
        let addr = address.to_base58check();
        let funds_str = (funds as f64 / SATOSHI_DIVISOR).to_string();
        let utxo_txid = self.0.sendtoaddress(&addr, &funds_str)?;
        Ok(self.get_transaction(&utxo_txid)?.unwrap())
    }

    pub fn create_multisig_address<'a, I>(&self,
                                          network: btc::Network,
                                          count: u8,
                                          pub_keys: I)
                                          -> Result<(btc::RedeemScript, btc::Address)>
        where I: IntoIterator<Item = &'a btc::PublicKey>
    {
        let redeem_script = btc::RedeemScript::from_pubkeys(pub_keys, count).compressed(network);
        let addr = btc::Address::from_script(&redeem_script, network);

        self.0
            .importaddress(&addr.to_base58check(), "multisig", false, false)?;
        Ok((redeem_script, addr))
    }

    pub fn get_last_anchoring_transactions(&self,
                                           addr: &str,
                                           limit: u32)
                                           -> Result<Vec<bitcoinrpc::TransactionInfo>> {
        self.0
            .listtransactions(limit, 0, true)
            .map(|v| {
                     v.into_iter()
                         .rev()
                         .filter(|tx| tx.address == Some(addr.into()))
                         .collect::<Vec<_>>()
                 })
    }

    pub fn get_unspent_transactions(&self,
                                    min_conf: u32,
                                    max_conf: u32,
                                    addr: &str)
                                    -> Result<Vec<bitcoinrpc::UnspentTransactionInfo>> {
        self.0.listunspent(min_conf, max_conf, [addr])
    }

    pub fn unspent_transactions(&self, addr: &btc::Address) -> Result<Vec<BitcoinTx>> {
        let unspent_txs = self.get_unspent_transactions(0, 9999999, &addr.to_base58check())?;
        let mut txs = Vec::new();
        for info in unspent_txs {
            if let Some(raw_tx) = self.get_transaction(&info.txid)? {
                match TxKind::from(raw_tx) {
                    TxKind::Anchoring(tx) => txs.push(tx.into()),
                    TxKind::FundingTx(tx) => txs.push(tx.into()),
                    TxKind::Other(_) => {}
                }
            }
        }
        Ok(txs)
    }
}

impl Deref for AnchoringRpc {
    type Target = RpcClient;

    fn deref(&self) -> &RpcClient {
        &self.0
    }
}
