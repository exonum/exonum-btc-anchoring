use serde_json;
use serde::{Deserialize, Deserializer};

use exonum::storage::StorageValue;
use exonum::crypto::{Hash, hash};

use details::btc;
use details::btc::transactions::FundingTx;

/// Public part of anchoring service configuration stored in blockchain.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct AnchoringConfig {
    /// Validators' public keys from which the current anchoring address can be calculated.
    pub anchoring_keys: Vec<btc::PublicKey>,
    /// The transaction that funds anchoring address.
    /// If the anchoring transactions chain is empty, it will be the first transaction in the chain.
    /// Note: you must specify a suitable transaction before the network launching.
    pub funding_tx: Option<FundingTx>,
    /// Fee for each transaction in chain.
    pub fee: u64,
    /// The frequency in blocks with which the generation of new anchoring
    /// transactions in the chain occurs.
    pub frequency: u64,
    /// The minimum number of confirmations in bitcoin network for the transition to a
    /// new anchoring address.
    pub utxo_confirmations: u64,
    /// The current bitcoin network type.
    #[serde(serialize_with = "btc_network_to_str", deserialize_with = "btc_network_from_str")]
    pub network: btc::Network,
}

impl Default for AnchoringConfig {
    fn default() -> AnchoringConfig {
        AnchoringConfig {
            anchoring_keys: vec![],
            funding_tx: None,
            fee: 1000,
            frequency: 500,
            utxo_confirmations: 5,
            network: btc::Network::Testnet,
        }
    }
}

impl AnchoringConfig {
    /// Creates anchoring configuration for the given `anchoring_keys` without funding transaction.
    /// This is usable for deploying procedure when the network participants exchange
    /// the public configuration before launching.
    /// Do not forget to send funding transaction to the final multisig address
    /// and add it to the final configuration.
    pub fn new<I>(network: btc::Network, anchoring_keys: I) -> AnchoringConfig
        where I: IntoIterator<Item = btc::PublicKey>
    {
        AnchoringConfig {
            anchoring_keys: anchoring_keys.into_iter().collect(),
            network: network,
            ..Default::default()
        }
    }

    /// Creates default anchoring configuration from given public keys and funding transaction
    /// which were created earlier by other way.
    pub fn new_with_funding_tx<I>(network: btc::Network,
                                  anchoring_keys: I,
                                  tx: FundingTx)
                                  -> AnchoringConfig
        where I: IntoIterator<Item = btc::PublicKey>
    {
        AnchoringConfig {
            anchoring_keys: anchoring_keys.into_iter().collect(),
            funding_tx: Some(tx),
            network: network,
            ..Default::default()
        }
    }

    #[doc(hidden)]
    /// Creates compressed `RedeemScript` from public keys in config.
    pub fn redeem_script(&self) -> (btc::RedeemScript, btc::Address) {
        let majority_count = self.majority_count();
        let redeem_script = btc::RedeemScript::from_pubkeys(self.anchoring_keys.iter(),
                                                            majority_count)
            .compressed(self.network);
        let addr = btc::Address::from_script(&redeem_script, self.network);
        (redeem_script, addr)
    }

    #[doc(hidden)]
    /// Returns the latest height below the given `height` which needs to be anchored.
    pub fn latest_anchoring_height(&self, height: u64) -> u64 {
        height - height % self.frequency as u64
    }

    #[doc(hidden)]
    pub fn majority_count(&self) -> u8 {
        ::majority_count(self.anchoring_keys.len() as u8)
    }

    /// Returns the funding transaction.
    ///
    /// # Panics
    ///
    /// If funding transaction is not specified.
    pub fn funding_tx(&self) -> &FundingTx {
        self.funding_tx
            .as_ref()
            .expect("You need to specify suitable funding_tx")
    }
}

fn btc_network_to_str<S>(network: &btc::Network, ser: S) -> Result<S::Ok, S::Error>
    where S: ::serde::Serializer
{
    match *network {
        btc::Network::Bitcoin => ser.serialize_str("bitcoin"),
        btc::Network::Testnet => ser.serialize_str("testnet"),
    }
}

fn btc_network_from_str<'de, D>(deserializer: D) -> Result<btc::Network, D::Error>
    where D: Deserializer<'de>
{
    let s: String = Deserialize::deserialize(deserializer)?;

    const VARIANTS: &[&str] = &["bitcoin", "testnet"];
    match s.as_str() {
        "bitcoin" => Ok(btc::Network::Bitcoin),
        "testnet" => Ok(btc::Network::Testnet),
        other => Err(::serde::de::Error::unknown_variant(other, VARIANTS)),
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
