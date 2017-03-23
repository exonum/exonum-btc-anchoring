use std::collections::BTreeMap;

use serde_json;

use bitcoin::util::base58::ToBase58;
use rand;
use rand::Rng;

use exonum::storage::StorageValue;
use exonum::crypto::{hash, Hash, HexValue};

use transactions::{AnchoringTx, FundingTx};
use client::AnchoringRpc;
use btc;

/// A `Bitcoind` rpc configuration
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AnchoringRpcConfig {
    /// Rpc url
    pub host: String,
    /// Rpc username
    pub username: Option<String>,
    /// Rpc password
    pub password: Option<String>,
}

/// Private part of anchoring service configuration which stored in local machine.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AnchoringNodeConfig {
    /// Rpc configuration
    pub rpc: AnchoringRpcConfig,
    /// Set of private keys for each anchoring address
    pub private_keys: BTreeMap<String, btc::PrivateKey>,
    /// Frequency of lect check in blocks
    pub check_lect_frequency: u64,
}

/// Public part of anchoring service configuration which stored in blockchain.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct AnchoringConfig {
    /// Public keys validators of which the current `anchoring` address can be obtained.
    pub validators: Vec<btc::PublicKey>,
    /// The transaction that funds `anchoring` address.
    /// If the chain of transaction is empty it will be a first transaction in the chain.
    pub funding_tx: FundingTx,
    /// A fee for each transaction in chain
    pub fee: u64,
    /// The frequency in blocks with which occurs the generation of a new `anchoring` transactions in chain.
    pub frequency: u64,
    /// The minimum number of confirmations in bitcoin network for the transition to a new `anchoring` address.
    pub utxo_confirmations: u64,
    /// The current bitcoin network type
    pub network: btc::Network,
}

impl AnchoringConfig {
    /// Creates default anchoring configuration from given public keys and funding transaction
    /// which were created earlier by other way.
    pub fn new(validators: Vec<btc::PublicKey>, tx: FundingTx) -> AnchoringConfig {
        AnchoringConfig {
            validators: validators,
            funding_tx: tx,
            fee: 1000,
            frequency: 500,
            utxo_confirmations: 24,
            network: btc::Network::Testnet,
        }
    }

    #[doc(hidden)]
    /// Returns bitcoin network type.
    pub fn network(&self) -> btc::RawNetwork {
        self.network.into()
    }

    #[doc(hidden)]
    /// Creates compressed `redeem_script` from public keys in config.
    pub fn redeem_script(&self) -> (btc::RedeemScript, btc::Address) {
        let majority_count = self.majority_count();
        let redeem_script = btc::RedeemScript::from_pubkeys(self.validators.iter(), majority_count)
            .compressed(self.network());
        let addr = btc::Address::from_script(&redeem_script, self.network());
        (redeem_script, addr)
    }

    #[doc(hidden)]
    /// Returns the nearest height below the given `height` which needs to be anchored
    pub fn nearest_anchoring_height(&self, height: u64) -> u64 {
        height - height % self.frequency as u64
    }

    #[doc(hidden)]
    /// For test purpose only
    pub fn majority_count(&self) -> u8 {
        ::majority_count(self.validators.len() as u8)
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
    /// Creates blank configuration from given rpc config.
    pub fn new(rpc: AnchoringRpcConfig) -> AnchoringNodeConfig {
        AnchoringNodeConfig {
            rpc: rpc,
            private_keys: BTreeMap::new(),
            check_lect_frequency: 30,
        }
    }
}

implement_serde_hex! {AnchoringTx}
implement_serde_hex! {FundingTx}

/// Generates testnet configuration by given rpc for given given nodes amount
/// using given random number generator.
/// Note: Bitcoin node that used by rpc have to enough bitcoin amount to generate
/// funding transaction by given `total_funds`.
pub fn testnet_generate_anchoring_config_with_rng<R>
    (client: &AnchoringRpc,
     network: btc::Network,
     count: u8,
     total_funds: u64,
     rng: &mut R)
     -> (AnchoringConfig, Vec<AnchoringNodeConfig>)
    where R: Rng
{
    let network = network.into();
    let rpc = AnchoringRpcConfig {
        host: client.url().into(),
        username: client.username().clone(),
        password: client.password().clone(),
    };
    let mut pub_keys = Vec::new();
    let mut node_cfgs = Vec::new();
    let mut priv_keys = Vec::new();

    for _ in 0..count as usize {
        let (pub_key, priv_key) = btc::gen_keypair_with_rng(network, rng);

        pub_keys.push(pub_key.clone());
        node_cfgs.push(AnchoringNodeConfig::new(rpc.clone()));
        priv_keys.push(priv_key.clone());
    }

    let majority_count = ::majority_count(count);
    let (_, address) =
        client.create_multisig_address(network.into(), majority_count, pub_keys.iter()).unwrap();
    let tx = FundingTx::create(client, &address, total_funds).unwrap();

    let genesis_cfg = AnchoringConfig::new(pub_keys, tx);
    for (idx, node_cfg) in node_cfgs.iter_mut().enumerate() {
        node_cfg.private_keys.insert(address.to_base58check(), priv_keys[idx].clone());
    }

    (genesis_cfg, node_cfgs)
}

/// Similar to [`testnet_generate_anchoring_config_with_rng`](fn.testnet_generate_anchoring_config_with_rng.html)
/// but it use default random number generator.
pub fn testnet_generate_anchoring_config(client: &AnchoringRpc,
                                         network: btc::Network,
                                         count: u8,
                                         total_funds: u64)
                                         -> (AnchoringConfig, Vec<AnchoringNodeConfig>) {
    let mut rng = rand::thread_rng();
    testnet_generate_anchoring_config_with_rng(client, network, count, total_funds, &mut rng)
}


#[cfg(test)]
mod tests {
    use serde_json::value::ToJson;
    use serde_json;

    use exonum::crypto::HexValue;
    use transactions::AnchoringTx;

    #[test]
    fn anchoring_tx_serde() {
        let hex = "010000000148f4ae90d8c514a739f17dbbd405442171b09f1044183080b23b6557ce82c0990100000000ffffffff0240899500000000001976a914b85133a96a5cadf6cddcfb1d17c79f42c3bbc9dd88ac00000000000000002e6a2c6a2a6a28020000000000000062467691cf583d4fa78b18fafaf9801f505e0ef03baf0603fd4b0cd004cd1e7500000000";
        let tx = AnchoringTx::from_hex(hex).unwrap();
        let json = tx.to_json().to_string();
        let tx2: AnchoringTx = serde_json::from_str(&json).unwrap();

        assert_eq!(tx2, tx);
    }
}
