use std::collections::BTreeMap;

use serde_json;
use serde::{Serialize, Serializer, Deserialize};
use serde::de::{Deserializer, Visitor, Error};
use bitcoinrpc::MultiSig;

use bitcoin::util::base58::{FromBase58, ToBase58};
use bitcoin::network::constants::Network;

use exonum::storage::StorageValue;
use exonum::crypto::{hash, Hash, HexValue};

use {BITCOIN_NETWORK, AnchoringTx, FundingTx, RpcClient, RedeemScript, AnchoringRpc};
use crypto::{BitcoinAddress, BitcoinPrivateKey, BitcoinPublicKey};
use btc;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AnchoringRpcConfig {
    pub host: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AnchoringNodeConfig {
    pub rpc: AnchoringRpcConfig,
    pub private_keys: BTreeMap<String, String>,
    pub check_lect_frequency: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct AnchoringConfig {
    pub validators: Vec<String>,
    pub funding_tx: FundingTx,
    pub fee: u64,
    pub frequency: u64,
    pub utxo_confirmations: u64,
}

pub fn generate_anchoring_config(client: &RpcClient,
                                 count: u8,
                                 total_funds: u64)
                                 -> (AnchoringConfig, Vec<AnchoringNodeConfig>) {
    let rpc = AnchoringRpcConfig {
        host: client.url().into(),
        username: client.username().clone(),
        password: client.password().clone(),
    };
    let mut pub_keys = Vec::new();
    let mut node_cfgs = Vec::new();
    let mut priv_keys = Vec::new();

    for idx in 0..count as usize {
        let account = format!("node_{}", idx);
        let (_, pub_key, priv_key) = client.gen_keypair(&account).unwrap();

        pub_keys.push(pub_key.clone());
        node_cfgs.push(AnchoringNodeConfig::new(rpc.clone()));
        priv_keys.push(priv_key.clone());
    }

    let majority_count = 2 * count / 3 + 1;
    let (_, address) =
        client.create_multisig_address(BITCOIN_NETWORK, majority_count, pub_keys.iter())
            .unwrap();
    let tx = FundingTx::create(&client, &address, total_funds).unwrap();

    let genesis_cfg = AnchoringConfig::new(pub_keys, tx);
    for (idx, node_cfg) in node_cfgs.iter_mut().enumerate() {
        node_cfg.private_keys.insert(address.to_base58check(), priv_keys[idx].clone());
    }

    (genesis_cfg, node_cfgs)
}

impl AnchoringRpcConfig {
    pub fn into_client(self) -> RpcClient {
        RpcClient::new(self.host, self.username, self.password)
    }
}

impl AnchoringConfig {
    pub fn new(validators: Vec<String>, tx: FundingTx) -> AnchoringConfig {
        AnchoringConfig {
            validators: validators,
            funding_tx: tx,
            fee: 1000,
            frequency: 50,
            utxo_confirmations: 24,
        }
    }

    pub fn network(&self) -> Network {
        BITCOIN_NETWORK
    }

    pub fn redeem_script(&self) -> (btc::RedeemScript, btc::Address) {
        let majority_count = self.majority_count();
        let redeem_script = RedeemScript::from_pubkeys(self.validators.iter(), majority_count)
            .compressed(self.network());
        let addr = btc::Address::from_script(&redeem_script, self.network());
        (redeem_script, addr)
    }

    pub fn multisig(&self) -> MultiSig {
        let (redeem_script, addr) = self.redeem_script();
        MultiSig {
            address: addr.to_base58check(),
            redeem_script: redeem_script.to_hex(),
        }
    }

    pub fn nearest_anchoring_height(&self, height: u64) -> u64 {
        height - height % self.frequency as u64
    }

    pub fn majority_count(&self) -> u8 {
        (2 * self.validators.len() / 3 + 1) as u8
    }
}

impl StorageValue for AnchoringConfig {
    fn serialize(self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    fn deserialize(v: Vec<u8>) -> Self {
        serde_json::from_slice(v.as_slice()).unwrap()
    }

    fn hash(&self) -> Hash {
        hash(serde_json::to_vec(&self).unwrap().as_slice())
    }
}

impl AnchoringNodeConfig {
    pub fn new(rpc: AnchoringRpcConfig) -> AnchoringNodeConfig {
        AnchoringNodeConfig {
            rpc: rpc,
            private_keys: BTreeMap::new(),
            check_lect_frequency: 30,
        }
    }
}

macro_rules! implement_serde_hex {
($name:ident) => (
    impl Serialize for $name {
        fn serialize<S>(&self, ser: &mut S) -> ::std::result::Result<(), S::Error>
            where S: Serializer
        {
            ser.serialize_str(&self.to_hex())
        }
    }

    impl Deserialize for $name {
        fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
            where D: Deserializer
        {
            struct HexVisitor;

            impl Visitor for HexVisitor {
                type Value = $name;

                fn visit_str<E>(&mut self, hex: &str) -> Result<$name, E>
                    where E: Error
                {
                    match $name::from_hex(hex) {
                        Ok(value) => Ok(value),
                        Err(_) => Err(Error::invalid_value("Wrong hex")),
                    }
                }
            }

            deserializer.deserialize_str(HexVisitor)
        }
    }
)
}

macro_rules! implement_serde_base58check {
($name:ident) => (
    impl Serialize for $name {
        fn serialize<S>(&self, ser: &mut S) -> ::std::result::Result<(), S::Error>
            where S: Serializer
        {
            ser.serialize_str(&self.to_base58check())
        }
    }

    impl Deserialize for $name {
        fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
            where D: Deserializer
        {
            struct Base58Visitor;

            impl Visitor for Base58Visitor {
                type Value = $name;

                fn visit_str<E>(&mut self, hex: &str) -> Result<$name, E>
                    where E: Error
                {
                    match $name::from_base58check(hex) {
                        Ok(value) => Ok(value),
                        Err(_) => Err(Error::invalid_value("Wrong base58")),
                    }
                }
            }

            deserializer.deserialize_str(Base58Visitor)
        }
    }
)
}

implement_serde_hex! {AnchoringTx}
implement_serde_hex! {FundingTx}
implement_serde_hex! {RedeemScript}
implement_serde_hex! {BitcoinPublicKey}
implement_serde_base58check! {BitcoinAddress}
implement_serde_base58check! {BitcoinPrivateKey}

#[cfg(test)]
mod tests {
    use serde_json::value::ToJson;
    use serde_json;

    use exonum::crypto::HexValue;
    use AnchoringTx;

    #[test]
    fn anchoring_tx_serde() {
        let hex = "010000000148f4ae90d8c514a739f17dbbd405442171b09f1044183080b23b6557ce82c0990100000000ffffffff0240899500000000001976a914b85133a96a5cadf6cddcfb1d17c79f42c3bbc9dd88ac00000000000000002e6a2c6a2a6a28020000000000000062467691cf583d4fa78b18fafaf9801f505e0ef03baf0603fd4b0cd004cd1e7500000000";
        let tx = AnchoringTx::from_hex(hex).unwrap();
        let json = tx.to_json().to_string();
        let tx2: AnchoringTx = serde_json::from_str(&json).unwrap();

        assert_eq!(tx2, tx);
    }
}
