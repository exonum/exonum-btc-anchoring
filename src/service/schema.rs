use std::fmt;

use byteorder::{BigEndian, ByteOrder};
use bitcoin::blockdata::transaction::SigHashType;
use serde_json::value::from_value;

use exonum::blockchain::{Schema, StoredConfiguration};
use exonum::storage::{ListTable, MerkleTable, List, MapTable, View, Map, Error as StorageError};
use exonum::crypto::{PublicKey, Hash};
use exonum::messages::{RawTransaction, Message, FromRaw, Error as MessageError};

use bitcoin::util::base58::ToBase58;

use btc;
use btc::TxId;
use transactions::{AnchoringTx, BitcoinTx};
use service::config::AnchoringConfig;

#[doc(hidden)]
pub const ANCHORING_SERVICE: u16 = 3;
const ANCHORING_MESSAGE_SIGNATURE: u16 = 0;
const ANCHORING_MESSAGE_LATEST: u16 = 1;

message! {
    MsgAnchoringSignature {
        const TYPE = ANCHORING_SERVICE;
        const ID = ANCHORING_MESSAGE_SIGNATURE;
        const SIZE = 56;

        from:           &PublicKey   [00 => 32]
        validator:      u32          [32 => 36]
        tx:             AnchoringTx  [36 => 44]
        input:          u32          [44 => 48]
        signature:      &[u8]        [48 => 56]
    }
}

message! {
    MsgAnchoringUpdateLatest {
        const TYPE = ANCHORING_SERVICE;
        const ID = ANCHORING_MESSAGE_LATEST;
        const SIZE = 52;

        from:           &PublicKey   [00 => 32]
        validator:      u32          [32 => 36]
        tx:             BitcoinTx    [36 => 44]
        lect_count:     u64          [44 => 52]
    }
}

#[doc(hidden)]
#[derive(Clone)]
pub enum AnchoringMessage {
    Signature(MsgAnchoringSignature),
    UpdateLatest(MsgAnchoringUpdateLatest),
}

#[doc(hidden)]
pub struct AnchoringSchema<'a> {
    view: &'a View,
}

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct FollowingConfig {
    pub actual_from: u64,
    pub config: AnchoringConfig,
}

impl Into<AnchoringMessage> for MsgAnchoringSignature {
    fn into(self) -> AnchoringMessage {
        AnchoringMessage::Signature(self)
    }
}

impl Into<AnchoringMessage> for MsgAnchoringUpdateLatest {
    fn into(self) -> AnchoringMessage {
        AnchoringMessage::UpdateLatest(self)
    }
}

impl AnchoringMessage {
    pub fn from(&self) -> &PublicKey {
        match *self {
            AnchoringMessage::UpdateLatest(ref msg) => msg.from(),
            AnchoringMessage::Signature(ref msg) => msg.from(),
        }
    }
}

impl Message for AnchoringMessage {
    fn raw(&self) -> &RawTransaction {
        match *self {
            AnchoringMessage::UpdateLatest(ref msg) => msg.raw(),
            AnchoringMessage::Signature(ref msg) => msg.raw(),
        }
    }

    fn verify_signature(&self, public_key: &PublicKey) -> bool {
        match *self {
            AnchoringMessage::UpdateLatest(ref msg) => msg.verify_signature(public_key),
            AnchoringMessage::Signature(ref msg) => msg.verify_signature(public_key),
        }
    }

    fn hash(&self) -> Hash {
        match *self {
            AnchoringMessage::UpdateLatest(ref msg) => Message::hash(msg),
            AnchoringMessage::Signature(ref msg) => Message::hash(msg),
        }
    }
}

impl FromRaw for AnchoringMessage {
    fn from_raw(raw: RawTransaction) -> ::std::result::Result<AnchoringMessage, MessageError> {
        match raw.message_type() {
            ANCHORING_MESSAGE_SIGNATURE => {
                Ok(AnchoringMessage::Signature(MsgAnchoringSignature::from_raw(raw)?))
            }
            ANCHORING_MESSAGE_LATEST => {
                Ok(AnchoringMessage::UpdateLatest(MsgAnchoringUpdateLatest::from_raw(raw)?))
            }
            _ => Err(MessageError::IncorrectMessageType { message_type: raw.message_type() }),
        }
    }
}

impl fmt::Debug for AnchoringMessage {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            AnchoringMessage::UpdateLatest(ref msg) => write!(fmt, "{:?}", msg),
            AnchoringMessage::Signature(ref msg) => write!(fmt, "{:?}", msg),
        }
    }
}

impl<'a> AnchoringSchema<'a> {
    pub fn new(view: &'a View) -> AnchoringSchema {
        AnchoringSchema { view: view }
    }

    pub fn signatures(&self,
                      txid: &TxId)
                      -> ListTable<MapTable<View, [u8], Vec<u8>>, u64, MsgAnchoringSignature> {
        let prefix = [&[ANCHORING_SERVICE as u8, 2], txid.as_ref()].concat();
        ListTable::new(MapTable::new(prefix, self.view))
    }

    pub fn lects(&self,
                 validator: u32)
                 -> MerkleTable<MapTable<View, [u8], Vec<u8>>, u64, BitcoinTx> {
        let mut prefix = vec![ANCHORING_SERVICE as u8, 3, 0, 0, 0, 0, 0, 0, 0, 0];
        BigEndian::write_u32(&mut prefix[2..], validator);
        MerkleTable::new(MapTable::new(prefix, self.view))
    }

    pub fn lect_indexes(&self, validator: u32) -> MapTable<View, TxId, u64> {
        let mut prefix = vec![ANCHORING_SERVICE as u8, 4, 0, 0, 0, 0, 0, 0, 0, 0];
        BigEndian::write_u32(&mut prefix[2..], validator);
        MapTable::new(prefix, self.view)
    }

    // List of known anchoring addresses
    pub fn known_addresses(&self) -> MapTable<View, str, Vec<u8>> {
        let prefix = vec![ANCHORING_SERVICE as u8, 5];
        MapTable::new(prefix, self.view)
    }

    pub fn current_anchoring_config(&self) -> Result<AnchoringConfig, StorageError> {
        let actual = Schema::new(self.view).get_actual_configuration()?;
        Ok(self.parse_config(&actual))
    }

    pub fn following_anchoring_config(&self) -> Result<Option<FollowingConfig>, StorageError> {
        let schema = Schema::new(self.view);
        if let Some(stored) = schema.get_following_configuration()? {
            let following_cfg = FollowingConfig {
                actual_from: stored.actual_from,
                config: self.parse_config(&stored),
            };
            Ok(Some(following_cfg))
        } else {
            Ok(None)
        }
    }

    pub fn create_genesis_config(&self, cfg: &AnchoringConfig) -> Result<(), StorageError> {
        let (_, addr) = cfg.redeem_script();
        self.add_known_address(&addr)?;
        for idx in 0..cfg.validators.len() {
            self.add_lect(idx as u32, cfg.funding_tx.clone())?;
        }
        Ok(())
    }

    pub fn add_lect<Tx>(&self, validator: u32, tx: Tx) -> Result<(), StorageError>
        where Tx: Into<BitcoinTx>
    {
        let lects = self.lects(validator);

        let tx = tx.into();
        let idx = lects.len()?;
        let txid = tx.id();
        lects.append(tx)?;
        self.lect_indexes(validator).put(&txid, idx)
    }

    pub fn lect(&self, validator: u32) -> Result<BitcoinTx, StorageError> {
        self.lects(validator).last().map(|x| x.unwrap())
    }

    pub fn prev_lect(&self, validator: u32) -> Result<Option<BitcoinTx>, StorageError> {
        let lects = self.lects(validator);

        let idx = lects.len()?;
        if idx > 1 {
            lects.get(idx - 2)
        } else {
            Ok(None)
        }
    }

    pub fn find_lect_position(&self,
                              validator: u32,
                              txid: &TxId)
                              -> Result<Option<u64>, StorageError> {
        self.lect_indexes(validator).get(txid)
    }

    pub fn add_known_address(&self, addr: &btc::Address) -> Result<(), StorageError> {
        self.known_addresses()
            .put(&addr.to_base58check(), vec![])
    }

    pub fn is_address_known(&self, addr: &btc::Address) -> Result<bool, StorageError> {
        self.known_addresses()
            .get(&addr.to_base58check())
            .map(|x| x.is_some())
    }

    pub fn state_hash(&self) -> Result<Vec<Hash>, StorageError> {
        let cfg = Schema::new(self.view).get_actual_configuration()?;
        let service_id = ANCHORING_SERVICE.to_string();
        if let Some(cfg) = cfg.services.get(&service_id) {
            let cfg: AnchoringConfig =
                from_value(cfg.clone()).expect("Valid configuration expected");

            let mut lect_hashes = Vec::new();
            for id in 0..cfg.validators.len() as u32 {
                lect_hashes.push(self.lects(id).root_hash()?);
            }
            Ok(lect_hashes)
        } else {
            Ok(Vec::new())
        }
    }

    fn parse_config(&self, cfg: &StoredConfiguration) -> AnchoringConfig {
        let service_id = ANCHORING_SERVICE.to_string();
        from_value(cfg.services[&service_id].clone()).unwrap()
    }
}

impl MsgAnchoringSignature {
    pub fn verify(&self) -> bool {
        // Do not verify signatures other than SigHashType::All
        let sighash_type_all = SigHashType::All.as_u32() as u8;
        self.signature().last() == Some(&sighash_type_all)
    }

    pub fn execute(&self, view: &View) -> Result<(), StorageError> {
        let schema = AnchoringSchema::new(view);

        let tx = self.tx();
        let id = self.validator();
        let cfg = schema.current_anchoring_config()?;
        // Verify signature
        if let Some(pub_key) = cfg.validators.get(id as usize) {
            let (redeem_script, _) = cfg.redeem_script();

            if tx.input.len() as u32 <= self.input() {
                error!("Received msg with incorrect signature content={:#?}", self);
                return Ok(());
            }
            if !tx.verify_input(&redeem_script, self.input(), pub_key, self.signature()) {
                error!("Received msg with incorrect signature content={:#?}", self);
                return Ok(());
            }
            schema.signatures(&tx.id()).append(self.clone())
        } else {
            Ok(())
        }
    }
}

impl MsgAnchoringUpdateLatest {
    pub fn verify(&self) -> bool {
        true
    }

    pub fn execute(&self, view: &View) -> Result<(), StorageError> {
        let schema = AnchoringSchema::new(view);

        let tx = self.tx();
        let id = self.validator();
        // Verify lect with actual cfg
        let actual_cfg = Schema::new(view).get_actual_configuration()?;
        if actual_cfg.validators.get(id as usize) != Some(self.from()) {
            error!("Received weird lect msg={:#?}", self);
            return Ok(());
        }
        if schema.lects(id).len()? != self.lect_count() {
            return Ok(());
        }
        schema.add_lect(id, tx)
    }
}

#[cfg(test)]
mod tests {
    use exonum::crypto::{PublicKey, Signature, HexValue};

    use bitcoin::blockdata::transaction::SigHashType;

    use transactions::AnchoringTx;
    use btc;
    use service::schema::MsgAnchoringSignature;

    #[test]
    fn test_sighash_type_all_in_msg_signature() {
        let tx = AnchoringTx::from_hex("01000000019aaf09d7e73a5f9ab394f1358bfb3dbde7b15b983d715f5c98f369a3f0a288a70000000000ffffffff02b80b00000000000017a914f18eb74087f751109cc9052befd4177a52c9a30a8700000000000000002c6a2a012800000000000000007fab6f66a0f7a747c820cd01fa30d7bdebd26b91c6e03f742abac0b3108134d900000000").unwrap();
        let btc_signature = btc::Signature::from_hex("3044022061d0bd408ec10f4f901c6d548151cc53031a3083f28dbcfc132319a162421d24022074f8a1c182088389bfae8646d9d99dea5b47db8f795d02efcc41ab4da0a8e11b01").unwrap();
        let msg = MsgAnchoringSignature::new_with_signature(&PublicKey::zero(),
                                                            0,
                                                            tx,
                                                            0,
                                                            &btc_signature,
                                                            &Signature::zero());

        assert!(msg.verify());
    }

    #[test]
    fn test_sighash_type_single_in_msg_signature() {
        let tx = AnchoringTx::from_hex("01000000019aaf09d7e73a5f9ab394f1358bfb3dbde7b15b983d715f5c98f369a3f0a288a70000000000ffffffff02b80b00000000000017a914f18eb74087f751109cc9052befd4177a52c9a30a8700000000000000002c6a2a012800000000000000007fab6f66a0f7a747c820cd01fa30d7bdebd26b91c6e03f742abac0b3108134d900000000").unwrap();
        let mut btc_signature = btc::Signature::from_hex("3044022061d0bd408ec10f4f901c6d548151cc53031a3083f28dbcfc132319a162421d24022074f8a1c182088389bfae8646d9d99dea5b47db8f795d02efcc41ab4da0a8e11b01").unwrap();
        *btc_signature.last_mut().unwrap() = SigHashType::Single.as_u32() as u8;
        let msg = MsgAnchoringSignature::new_with_signature(&PublicKey::zero(),
                                                            0,
                                                            tx,
                                                            0,
                                                            &btc_signature,
                                                            &Signature::zero());

        assert!(!msg.verify());
    }
}