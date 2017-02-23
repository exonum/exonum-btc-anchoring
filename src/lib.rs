extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate bitcoinrpc;
#[macro_use]
extern crate exonum;
extern crate bitcoin;
extern crate secp256k1;
extern crate byteorder;
#[macro_use]
extern crate log;
#[cfg(test)]
extern crate rand;
#[cfg(test)]
extern crate env_logger;

mod service;
mod schema;
mod crypto;
pub mod config;
pub mod transactions;
pub mod multisig;
#[cfg(feature="sandbox_tests")]
pub mod sandbox;

use bitcoin::blockdata::script::{Script, Builder};
use bitcoin::util::hash::Sha256dHash;
use bitcoin::network::serialize::deserialize;
use bitcoin::network::constants::Network;

use exonum::crypto::{FromHexError, ToHex, FromHex};

use multisig::RedeemScript;

pub use service::AnchoringService;
pub use schema::{AnchoringSchema, ANCHORING_SERVICE, TxAnchoringSignature, TxAnchoringUpdateLatest};
pub use transactions::{BitcoinTx, AnchoringTx, FundingTx, TxKind};

#[cfg(not(feature="sandbox_tests"))]
pub use bitcoinrpc::Client as RpcClient;
#[cfg(feature="sandbox_tests")]
pub use sandbox::SandboxClient as RpcClient;

pub const SATOSHI_DIVISOR: f64 = 100_000_000.0;
// TODO add feature for bitcoin network
pub const BITCOIN_NETWORK: Network = Network::Testnet;

pub type Result<T> = bitcoinrpc::Result<T>;
pub type Error = bitcoinrpc::Error;

pub type BitcoinAddress = String;
pub type BitcoinPublicKey = String;
pub type BitcoinPrivateKey = String;
pub type BitcoinSignature = Vec<u8>;

pub trait HexValue: Sized {
    fn to_hex(&self) -> String;
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError>;
}

impl HexValue for Script {
    fn to_hex(&self) -> String {
        self.clone().into_vec().to_hex()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError> {
        let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
        Ok(Builder::from(bytes).into_script())
    }
}

impl HexValue for Sha256dHash {
    fn to_hex(&self) -> String {
        self.be_hex_string()
    }
    fn from_hex<T: AsRef<str>>(v: T) -> ::std::result::Result<Self, FromHexError> {
        let bytes: Vec<u8> = FromHex::from_hex(v.as_ref())?;
        if let Ok(hash) = deserialize(bytes.as_ref()) {
            Ok(hash)
        } else {
            Err(FromHexError::InvalidHexLength)
        }
    }
}

pub trait AnchoringRpc {
    fn gen_keypair(&self, account: &str) -> Result<(String, String, String)>;
    fn get_transaction(&self, txid: &str) -> Result<BitcoinTx>;
    fn send_transaction(&self, tx: BitcoinTx) -> Result<String>;
    fn send_to_address(&self, address: &str, funds: u64) -> Result<BitcoinTx>;
    fn create_multisig_address<'a, I>(&self,
                                      count: u8,
                                      pub_keys: I)
                                      -> Result<bitcoinrpc::MultiSig>
        where I: Iterator<Item = &'a BitcoinAddress>;

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

    fn find_lect(&self, addr: &str) -> Result<Vec<AnchoringTx>> {
        let txs = self.get_unspent_transactions(0, 9999999, &addr)?;
        // FIXME Develop searching algorhytm
        let mut anchoring_txs = Vec::new();
        for info in txs {
            let raw_tx = self.get_transaction(&info.txid)?;
            if let TxKind::Anchoring(tx) = TxKind::from(raw_tx) {
                anchoring_txs.push(tx);
            }
        }
        Ok(anchoring_txs)
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
        Ok(BitcoinTx::from_hex(tx.hex.unwrap()).unwrap())
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

    fn create_multisig_address<'a, I>(&self, count: u8, pub_keys: I) -> Result<bitcoinrpc::MultiSig>
        where I: Iterator<Item = &'a BitcoinAddress>
    {
        let redeem_script = RedeemScript::from_pubkeys(pub_keys, count).compressed(BITCOIN_NETWORK);
        let multisig = bitcoinrpc::MultiSig {
            address: redeem_script.to_address(BITCOIN_NETWORK),
            redeem_script: redeem_script.to_hex(),
        };
        self.importaddress(&multisig.address, "multisig", false, false)?;
        Ok(multisig)
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

#[cfg(test)]
mod tests {
    extern crate bitcoin;
    extern crate bitcoinrpc;
    extern crate rand;
    extern crate blockchain_explorer;

    use std::env;
    use std::collections::HashMap;

    use env_logger;
    use rand::Rng;

    use exonum::crypto::{Hash, hash, HexValue};
    use exonum::storage::StorageValue;

    use super::{AnchoringRpc, AnchoringTx, BitcoinTx, FundingTx, BitcoinPublicKey,
                BitcoinPrivateKey, BitcoinSignature, HexValue as HexValueEx, RpcClient};
    use super::config::AnchoringRpcConfig;
    use super::crypto::TxId;

    fn anchoring_client() -> RpcClient {
        let rpc = AnchoringRpcConfig {
            host: env::var("ANCHORING_HOST").unwrap().parse().unwrap(),
            username: env::var("ANCHORING_USER").ok(),
            password: env::var("ANCHORING_PASSWORD").ok(),
        };
        RpcClient::new(rpc.host, rpc.username, rpc.password)
    }

    fn gen_anchoring_keys(client: &RpcClient,
                          count: usize)
                          -> (Vec<BitcoinPublicKey>, Vec<BitcoinPrivateKey>) {
        let mut validators = Vec::new();
        let mut priv_keys = Vec::new();
        for i in 0..count {
            let account = format!("node_{}", i);
            let (_, pub_key, priv_key) = client.gen_keypair(&account).unwrap();
            validators.push(pub_key);
            priv_keys.push(priv_key);
        }
        (validators, priv_keys)
    }

    fn make_signatures(redeem_script: &str,
                       proposal: &AnchoringTx,
                       inputs: &[u32],
                       priv_keys: &[BitcoinPrivateKey])
                       -> HashMap<u32, Vec<BitcoinSignature>> {
        let majority_count = RpcClient::majority_count(priv_keys.len() as u8);

        let mut signatures = inputs.iter()
            .map(|input| (*input, vec![None; priv_keys.len()]))
            .collect::<Vec<_>>();
        let mut priv_keys = priv_keys.iter()
            .enumerate()
            .collect::<Vec<_>>();
        rand::thread_rng().shuffle(&mut priv_keys);

        for (input_idx, input) in inputs.iter().enumerate() {
            let priv_keys_iter = priv_keys.iter().take(majority_count as usize);
            for &(id, priv_key) in priv_keys_iter {
                let sign = proposal.sign(redeem_script, *input, &priv_key);
                signatures[input_idx].1[id] = Some(sign);
            }
        }

        signatures.iter()
            .map(|signs| {
                let input = signs.0;
                let signs = signs.1
                    .iter()
                    .filter_map(|x| x.clone())
                    .take(majority_count as usize)
                    .collect::<Vec<_>>();
                (input, signs)
            })
            .collect::<HashMap<_, _>>()
    }

    fn send_anchoring_tx(client: &RpcClient,
                         from: &bitcoinrpc::MultiSig,
                         to: &bitcoinrpc::MultiSig,
                         block_height: u64,
                         block_hash: Hash,
                         priv_keys: &[BitcoinPrivateKey],
                         anchoring_tx: AnchoringTx,
                         additional_funds: &[FundingTx],
                         fee: u64)
                         -> AnchoringTx {
        let tx = anchoring_tx.proposal(&from,
                      &to,
                      fee,
                      additional_funds,
                      block_height,
                      block_hash.clone())
            .unwrap();
        debug!("Proposal anchoring_tx={:#?}, txid={}",
               tx,
               tx.txid().to_hex());


        let inputs = (0..additional_funds.len() as u32 + 1).collect::<Vec<_>>();
        let signatures = make_signatures(&from.redeem_script, &tx, inputs.as_slice(), &priv_keys);
        let tx = tx.send(&client, &from, signatures).unwrap();
        assert_eq!(tx.payload(), (block_height, block_hash));

        debug!("Sended anchoring_tx={:#?}, txid={}", tx, tx.txid().to_hex());
        let lect_tx = client.find_lect(&to.address).unwrap().first().unwrap().clone();
        assert_eq!(lect_tx, tx);
        lect_tx
    }

    #[test]
    fn test_anchoring_txid() {
        let tx = AnchoringTx::from_hex("010000000195a4472606ae658f1b9cbebd43f440def00c94341a3515024855a1da8d80932800000000fd3d020047304402204e11d63db849f253095e1e0a400f2f0c01894083e97bfaef644b1407b9fe5c4102207cc99ca986dfd99230e6641564d1f70009c5ec9a37da815c4e024c3ba837c01301483045022100d32536daa6e13989ebc7c908c27a0608517d5d967c8b6069dc047faa01e2a096022030f9c46738d9b701dd944ce3e31af9898b9266460b2de6ff3319f2a8c51f7b430147304402206b8e4491e3b98861ba06cf64e78f425cc553110535310f56f71dcd37de590b7f022051f0fa53cb74a1c73247224180cf026b61b7959d587ab6365dd19a279d14cf45014830450221009fa024c767d8004eef882c6cffe9602f781c60d1a7c629d58576e3de41833a5b02206d3b8dc86d052e112305e1fb32f61de77236f057523e22d58d82cbe37222e8fa01483045022100f1784c5e321fb2753fe725381d6f922d3f0edb94ff2eef52063f9c812489f61802202bec2903af6a5405db484ac73ab844707382f39a0b286a0453f2ed41d217c89e014ccf5521027b3e1c603ead09953bd0a8bd13a7a4830a1446289969220b96515dd1745e06f521026b64f403914e43b7ebe9aa23017eb75eef1bc74469f8b1fa342e622565ab28db2103503745e14331dac53528e666f1abab2c6b6e28767539a2827fe080bb475ec25021030a2ff505279a0e58cc3951ada56bcf323955550d1b993c4cb1b7e94a672b31252102ebb5a22d5ec3c2bc36ab7e104553a89c69684a4dfb3c8ea8fe2cb785c63425872102d9fea63c62d7cafcd4a3d20d77e06cf80cb25f3277ffce27d99c98f439323cee56aeffffffff02000000000000000017a914ab6db56dbd716114594a0d3f072ec447f6d8fc698700000000000000002c6a2a0128020000000000000062467691cf583d4fa78b18fafaf9801f505e0ef03baf0603fd4b0cd004cd1e7500000000").unwrap();

        let txid_hex = "0e4167aeb4769de5ad8d64d1b2342330c2b6aadc0ed9ad0d26ae8eafb18d9c87";
        let txid = TxId::from_hex(txid_hex).unwrap();
        let txid2 = tx.txid();

        assert_eq!(txid2.to_hex(), txid_hex);
        assert_eq!(txid2, txid);
    }

    #[test]
    fn test_anchoring_tx_storage_value() {
        let hex = "010000000148f4ae90d8c514a739f17dbbd405442171b09f1044183080b23b6557ce82c0990100000000ffffffff0240899500000000001976a914b85133a96a5cadf6cddcfb1d17c79f42c3bbc9dd88ac00000000000000002e6a2c6a2a6a28020000000000000062467691cf583d4fa78b18fafaf9801f505e0ef03baf0603fd4b0cd004cd1e7500000000";
        let tx = AnchoringTx::from(BitcoinTx::from_hex(hex).unwrap());
        let data = tx.clone().serialize();
        let tx2: AnchoringTx = AnchoringTx::deserialize(data);

        assert_eq!(tx2, tx);
    }

    #[test]
    fn test_transaction_info() {
        let _ = env_logger::init();

        let rpc = AnchoringRpcConfig {
            host: env::var("ANCHORING_HOST").unwrap().parse().unwrap(),
            username: env::var("ANCHORING_USER").ok(),
            password: env::var("ANCHORING_PASSWORD").ok(),
        };

        let client = RpcClient::new(rpc.host, rpc.username, rpc.password);

        let exists_hex = "0100000001467510b9ceafacba7a7ad2fc816622408b20bf514e6b0c9ff828eb2a63591de300000000fd6901004830450221008d590771fcd5dc1f197e686747423e89bf3575b3119191a75108f44da45f5e69022002a87258d7f830f097b44c4c1d5886a3a086d5258b2b4b8d7d287bcaf1b2d84101483045022100c4a5eceaf68f5ac0aa55ecab726bbb111313fda97e4d0ef3431eaf51d44f833a02201aa50734c275d4e77c5c0c33b679922c5009d20dcc4b8ff651dce0daac57f641014830450221009fcc94c63a00ae1d1862ad3f0e15a1e4e65366e7413fd99600b87304bb151fe4022021f6e01c313c9e3f628cc92f3f5710009593c1b1876210fa6c2ed745ecf3edf6014c8b532103ff02badf5feaa9b764a55830d738db909f67ba09be93fee890d735474992d9ac21036cb28f25be8dbc100477b9ef0d104110efe7d1ad5279531fefa0f1b93bab2d6b21029b8c2c2e88ccaa3a5471e84692e69696c6887343ba36e666d5f931050aa384cc210300abc4f927419b6862a13a295c410f2d0f7e317ba101ef3785284260f273222c54aeffffffff02d00101000000000017a914ff1fc6bb4705ac95bcd40dba6c85beeec46effe78700000000000000002c6a2a6a28e40c000000000000a836052f6a326313a17903cec8f9229c193dbedcd72e98118164609c3b6dd2e900000000";
        let tx = AnchoringTx::from(BitcoinTx::from_hex(exists_hex).unwrap());

        let info = tx.get_info(&client).unwrap();
        debug!("tx_info={:#?}", info);

        let some_hex = "010000000148f4ae90d8c514a739f17dbbd405442171b09f1044183080b23b6557ce82c0990100000000ffffffff0240899500000000001976a914b85133a96a5cadf6cddcfb1d17c79f42c3bbc9dd88ac00000000000000002e6a2c6a2a6a28020000000000000062467691cf583d4fa78b18fafaf9801f505e0ef03baf0603fd4b0cd004cd1e7500000000";
        let tx = AnchoringTx::from(BitcoinTx::from_hex(some_hex).unwrap());
        let info = tx.get_info(&client).unwrap();
        debug!("tx_info={:#?}", info);

        assert!(info.is_none());
    }

    #[test]
    fn test_unspent_funding_tx() {
        let _ = blockchain_explorer::helpers::init_logger();

        let client = anchoring_client();
        let (validators, _) = gen_anchoring_keys(&client, 4);

        let majority_count = RpcClient::majority_count(4);
        let multisig = client.create_multisig_address(majority_count, validators.iter())
            .unwrap();

        {
            use bitcoin::blockdata::script::Script;
            let redeem_script = Vec::<u8>::from_hex(multisig.redeem_script.clone()).unwrap();
            let script = Script::from(redeem_script);
            debug!("{:#?}", script);
        }

        let funding_tx = FundingTx::create(&client, &multisig, 1000).unwrap();
        let info = funding_tx.is_unspent(&client, &multisig).unwrap();
        assert!(info.is_some());
        debug!("{:#?}", info);
    }

    #[test]
    fn test_anchoring_3_4() {
        let _ = blockchain_explorer::helpers::init_logger();

        let client = anchoring_client();

        let (validators, priv_keys) = gen_anchoring_keys(&client, 4);
        let majority_count = RpcClient::majority_count(4);
        let multisig = client.create_multisig_address(majority_count, validators.iter())
            .unwrap();
        debug!("multisig_address={:#?}", multisig);

        let fee = 1000;
        let block_height = 2;
        let block_hash = hash(&[1, 3, 5]);

        // Make anchoring txs chain
        let total_funds = 4000;
        let mut utxo_tx = {
            let funding_tx = FundingTx::create(&client, &multisig, total_funds).unwrap();
            let tx = funding_tx.clone()
                .make_anchoring_tx(&multisig, fee, block_height, block_hash)
                .unwrap();
            debug!("Proposal anchoring_tx={:#?}, txid={}",
                   tx,
                   tx.txid().to_hex());

            let signatures = make_signatures(&multisig.redeem_script, &tx, &[0], &priv_keys);
            let tx = tx.send(&client, &multisig, signatures).unwrap();
            debug!("Sended anchoring_tx={:#?}, txid={}", tx, tx.txid().to_hex());

            assert!(funding_tx.is_unspent(&client, &multisig).unwrap().is_none());
            let lect_tx = client.find_lect(&multisig.address).unwrap().first().unwrap().clone();
            assert_eq!(lect_tx, tx);
            tx
        };

        let utxos = client.listunspent(0, 9999999, &[multisig.address.as_str()]).unwrap();
        debug!("utxos={:#?}", utxos);

        // Send anchoring txs
        let mut out_funds = utxo_tx.amount();
        debug!("out_funds={}", out_funds);
        while out_funds >= fee {
            utxo_tx = send_anchoring_tx(&client,
                                        &multisig,
                                        &multisig,
                                        block_height,
                                        block_hash.clone(),
                                        &priv_keys,
                                        utxo_tx,
                                        &[],
                                        fee);
            assert_eq!(utxo_tx.payload(), (block_height, block_hash));
            out_funds -= fee;
        }

        // Try to add funding input
        let funding_tx = FundingTx::create(&client, &multisig, fee * 3).unwrap();
        utxo_tx = send_anchoring_tx(&client,
                                    &multisig,
                                    &multisig,
                                    block_height,
                                    block_hash.clone(),
                                    &priv_keys,
                                    utxo_tx,
                                    &[funding_tx],
                                    fee);

        // Send to next addr
        let (validators2, priv_keys2) = gen_anchoring_keys(&client, 6);
        let majority_count2 = RpcClient::majority_count(6);
        let multisig2 = client.create_multisig_address(majority_count2, validators2.iter())
            .unwrap();

        debug!("new_multisig_address={:#?}", multisig2);
        utxo_tx = send_anchoring_tx(&client,
                                    &multisig,
                                    &multisig2,
                                    block_height,
                                    block_hash.clone(),
                                    &priv_keys,
                                    utxo_tx,
                                    &[],
                                    fee);

        send_anchoring_tx(&client,
                          &multisig2,
                          &multisig2,
                          block_height,
                          block_hash.clone(),
                          &priv_keys2,
                          utxo_tx,
                          &[],
                          fee);
    }

    #[test]
    fn test_anchoring_different_txs() {
        let _ = blockchain_explorer::helpers::init_logger();

        let client = anchoring_client();
        let (validators, priv_keys) = gen_anchoring_keys(&client, 4);

        let majority_count = RpcClient::majority_count(4);
        let multisig = client.create_multisig_address(majority_count, validators.iter())
            .unwrap();

        let total_funds = 10000;
        let fee = total_funds;
        let tx = FundingTx::create(&client, &multisig, total_funds).unwrap();

        debug!("multisig_address={:#?}", multisig);
        debug!("utxo_tx={:#?}", tx);

        let block_height = 2;
        let block_hash = hash(&[1, 3, 5]);

        let proposal = tx.make_anchoring_tx(&multisig, fee, block_height, block_hash.clone())
            .unwrap();

        let signs1 = make_signatures(&multisig.redeem_script, &proposal, &[0], &priv_keys);
        let signs2 = make_signatures(&multisig.redeem_script, &proposal, &[0], &priv_keys);

        let tx1 = proposal.clone().send(&client, &multisig, signs1).unwrap();
        debug!("tx1={:#?}", tx1);
        let tx2 = proposal.clone().send(&client, &multisig, signs2);
        debug!("tx2={:#?}", tx2);

        let txs = client.get_last_anchoring_transactions(&multisig.address, 144).unwrap();
        debug!("txs={:#?}", txs);

        // assert!(tx2.is_err());
    }
}