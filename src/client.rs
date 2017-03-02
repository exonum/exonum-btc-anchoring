use bitcoinrpc;
use bitcoin::util::base58::ToBase58;
use bitcoin::network::constants::Network;

use exonum::crypto::HexValue;

use transactions::{AnchoringTx, BitcoinTx, TxKind};
use {SATOSHI_DIVISOR, BITCOIN_NETWORK};
use multisig::RedeemScript;
use btc;

#[cfg(not(feature="sandbox_tests"))]
pub use bitcoinrpc::Client as RpcClient;
#[cfg(feature="sandbox_tests")]
pub use sandbox::SandboxClient as RpcClient;

pub type Result<T> = bitcoinrpc::Result<T>;
pub type Error = bitcoinrpc::Error;

pub trait AnchoringRpc {
    fn gen_keypair(&self, account: &str) -> Result<(String, String, String)>;
    fn get_transaction(&self, txid: &str) -> Result<BitcoinTx>;
    fn send_transaction(&self, tx: BitcoinTx) -> Result<String>;
    fn send_to_address(&self, address: &str, funds: u64) -> Result<BitcoinTx>;
    fn create_multisig_address<'a, I>(&self,
                                      network: Network,
                                      count: u8,
                                      pub_keys: I)
                                      -> Result<(RedeemScript, btc::Address)>
        where I: Iterator<Item = &'a String>;

    fn get_last_anchoring_transactions(&self,
                                       addr: &str,
                                       limit: u32)
                                       -> Result<Vec<bitcoinrpc::TransactionInfo>>;

    fn get_unspent_transactions(&self,
                                min_conf: u32,
                                max_conf: u32,
                                addr: &str)
                                -> Result<Vec<bitcoinrpc::UnspentTransactionInfo>>;

    fn majority_count(total_count: u8) -> u8 {
        total_count * 2 / 3 + 1
    }

    fn get_lect(&self, multisig: &bitcoinrpc::MultiSig) -> Result<Option<AnchoringTx>> {
        let txs = self.get_last_anchoring_transactions(&multisig.address, 30)?;
        if let Some(info) = txs.first() {
            let tx = self.get_transaction(&info.txid)?;
            Ok(Some(AnchoringTx::from(tx)))
        } else {
            Ok(None)
        }
    }

    fn unspent_lects(&self, addr: &btc::Address) -> Result<Vec<BitcoinTx>> {
        let unspent_txs = self.get_unspent_transactions(0, 9999999, &addr.to_base58check())?;
        // FIXME Develop searching algorhytm
        let mut txs = Vec::new();
        for info in unspent_txs {
            let raw_tx = self.get_transaction(&info.txid)?;
            match TxKind::from(raw_tx) {
                TxKind::Anchoring(tx) => txs.push(tx.into()),
                TxKind::FundingTx(tx) => txs.push(tx.into()),
                TxKind::Other(_) => {}
            }
        }
        Ok(txs)
    }
}

impl AnchoringRpc for RpcClient {
    fn gen_keypair(&self, account: &str) -> Result<(String, String, String)> {
        let addr = self.getnewaddress(account)?;
        let info = self.validateaddress(&addr)?;
        let privkey = self.dumpprivkey(&addr)?;
        Ok((addr, info.pubkey, privkey))
    }

    fn get_transaction(&self, txid: &str) -> Result<BitcoinTx> {
        let tx = self.getrawtransaction(txid)?;
        Ok(BitcoinTx::from_hex(tx).unwrap())
    }

    fn send_transaction(&self, tx: BitcoinTx) -> Result<String> {
        let tx_hex = tx.to_hex();
        self.sendrawtransaction(&tx_hex)
    }

    fn send_to_address(&self, address: &str, funds: u64) -> Result<BitcoinTx> {
        let funds_str = (funds as f64 / SATOSHI_DIVISOR).to_string();
        let utxo_txid = self.sendtoaddress(address, &funds_str)?;
        Ok(self.get_transaction(&utxo_txid)?)
    }

    fn create_multisig_address<'a, I>(&self,
                                      network: Network,
                                      count: u8,
                                      pub_keys: I)
                                      -> Result<(RedeemScript, btc::Address)>
        where I: Iterator<Item = &'a String>
    {
        let redeem_script = RedeemScript::from_pubkeys(pub_keys, count).compressed(network);
        let addr = btc::Address::from_script(&redeem_script, network);

        self.importaddress(&addr.to_base58check(), "multisig", false, false)?;
        Ok((redeem_script, addr))
    }

    fn get_last_anchoring_transactions(&self,
                                       addr: &str,
                                       limit: u32)
                                       -> Result<Vec<bitcoinrpc::TransactionInfo>> {
        self.listtransactions(limit, 0, true)
            .map(|v| {
                v.into_iter()
                    .rev()
                    .filter(|tx| tx.address == Some(addr.into()))
                    .collect::<Vec<_>>()
            })
    }

    fn get_unspent_transactions(&self,
                                min_conf: u32,
                                max_conf: u32,
                                addr: &str)
                                -> Result<Vec<bitcoinrpc::UnspentTransactionInfo>> {
        self.listunspent(min_conf, max_conf, [addr])
    }
}