use std::fmt;

use byteorder::{BigEndian, ByteOrder};
use serde_json::value::from_value;

use exonum::blockchain::Schema;
use exonum::storage::{ListTable, MerkleTable, List, MapTable, View, Map,
                      Error as StorageError};
use exonum::crypto::{PublicKey, Hash};
use exonum::messages::{RawTransaction, Message, FromRaw, Error as MessageError};
use exonum::node::Height;

use config::AnchoringConfig;
use crypto::TxId;
use AnchoringTx;

pub const ANCHORING_SERVICE: u16 = 3;
const ANCHORING_TRANSACTION_SIGNATURE: u16 = 0;
const ANCHORING_TRANSACTION_LATEST: u16 = 1;

// Подпись за анкорящую транзакцию
message! {
    TxAnchoringSignature {
        const TYPE = ANCHORING_SERVICE;
        const ID = ANCHORING_TRANSACTION_SIGNATURE;
        const SIZE = 56;

        from:           &PublicKey   [00 => 32]
        validator:      u32          [32 => 36]
        tx:             AnchoringTx  [36 => 44]
        input:          u32          [44 => 48]
        signature:      &[u8]        [48 => 56]
    }
}

// Сообщение об обновлении последней корректной транзакции
message! {
    TxAnchoringUpdateLatest {
        const TYPE = ANCHORING_SERVICE;
        const ID = ANCHORING_TRANSACTION_LATEST;
        const SIZE = 44;

        from:           &PublicKey   [00 => 32]
        validator:      u32          [32 => 36]
        tx:             AnchoringTx  [36 => 44]
    }
}

#[derive(Clone)]
pub enum AnchoringTransaction {
    Signature(TxAnchoringSignature),
    UpdateLatest(TxAnchoringUpdateLatest),
}

pub struct AnchoringSchema<'a> {
    view: &'a View,
}

impl Into<AnchoringTransaction> for TxAnchoringSignature {
    fn into(self) -> AnchoringTransaction {
        AnchoringTransaction::Signature(self)
    }
}

impl Into<AnchoringTransaction> for TxAnchoringUpdateLatest {
    fn into(self) -> AnchoringTransaction {
        AnchoringTransaction::UpdateLatest(self)
    }
}

impl AnchoringTransaction {
    pub fn from(&self) -> &PublicKey {
        match *self {
            AnchoringTransaction::UpdateLatest(ref msg) => msg.from(),
            AnchoringTransaction::Signature(ref msg) => msg.from(),
        }
    }
}

impl Message for AnchoringTransaction {
    fn raw(&self) -> &RawTransaction {
        match *self {
            AnchoringTransaction::UpdateLatest(ref msg) => msg.raw(),
            AnchoringTransaction::Signature(ref msg) => msg.raw(),
        }
    }

    fn verify_signature(&self, public_key: &PublicKey) -> bool {
        match *self {
            AnchoringTransaction::UpdateLatest(ref msg) => msg.verify_signature(public_key),
            // TODO проверка, что подпись за анкорящую транзакцию верная
            AnchoringTransaction::Signature(ref msg) => msg.verify_signature(public_key),
        }
    }

    fn hash(&self) -> Hash {
        match *self {
            AnchoringTransaction::UpdateLatest(ref msg) => Message::hash(msg),
            AnchoringTransaction::Signature(ref msg) => Message::hash(msg),
        }
    }
}

impl FromRaw for AnchoringTransaction {
    fn from_raw(raw: RawTransaction) -> ::std::result::Result<AnchoringTransaction, MessageError> {
        match raw.message_type() {
            ANCHORING_TRANSACTION_SIGNATURE => {
                Ok(AnchoringTransaction::Signature(TxAnchoringSignature::from_raw(raw)?))
            }
            ANCHORING_TRANSACTION_LATEST => {
                Ok(AnchoringTransaction::UpdateLatest(TxAnchoringUpdateLatest::from_raw(raw)?))
            }
            _ => Err(MessageError::IncorrectMessageType { message_type: raw.message_type() }),
        }
    }
}

impl fmt::Debug for AnchoringTransaction {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            AnchoringTransaction::UpdateLatest(ref msg) => write!(fmt, "{:?}", msg),
            AnchoringTransaction::Signature(ref msg) => write!(fmt, "{:?}", msg),
        }
    }
}

//FIXME use Sha256dHash instead exonum::Hash
impl<'a> AnchoringSchema<'a> {
    pub fn new(view: &'a View) -> AnchoringSchema {
        AnchoringSchema { view: view }
    }

    // хэш это txid
    pub fn signatures(&self,
                      txid: &TxId)
                      -> ListTable<MapTable<View, [u8], Vec<u8>>, u64, TxAnchoringSignature> {
        let prefix = [&[ANCHORING_SERVICE as u8, 02], txid.as_ref()].concat();
        ListTable::new(MapTable::new(prefix, self.view))
    }

    pub fn lects(&self,
                 validator: u32)
                 -> MerkleTable<MapTable<View, [u8], Vec<u8>>, u64, AnchoringTx> {
        let mut prefix = vec![ANCHORING_SERVICE as u8, 03, 0, 0, 0, 0, 0, 0, 0, 0];
        BigEndian::write_u32(&mut prefix[2..], validator);
        MerkleTable::new(MapTable::new(prefix, self.view))
    }

    pub fn lect_indexes(&self, validator: u32) -> MapTable<View, TxId, u64> {
        let mut prefix = vec![ANCHORING_SERVICE as u8, 04, 0, 0, 0, 0, 0, 0, 0, 0];
        BigEndian::write_u32(&mut prefix[2..], validator);
        MapTable::new(prefix, self.view)
    }

    pub fn current_anchoring_config(&self) -> Result<AnchoringConfig, StorageError> {
        let height = self.config_height().get(0)?.unwrap();
        let cfg = Schema::new(self.view).get_configuration_at_height(height)?.unwrap();
        Ok(from_value(cfg.services[&ANCHORING_SERVICE].clone()).unwrap())
    }

    pub fn update_anchoring_config(&self) -> Result<(), StorageError> {
        let height = Schema::new(self.view).get_actual_configurations_height()?;
        self.config_height().set(0, height)
    }

    pub fn create_genesis_config(&self) -> Result<(), StorageError> {
        self.config_height().append(0)
    }

    pub fn add_lect(&self, validator: u32, tx: AnchoringTx) -> Result<(), StorageError> {
        let lects = self.lects(validator);
        let idx = lects.len()?;
        let txid = tx.id();
        lects.append(tx)?;
        self.lect_indexes(validator).put(&txid, idx)
    }

    pub fn find_lect_position(&self, validator: u32, txid: &TxId) -> Result<Option<u64>, StorageError> {
        self.lect_indexes(validator).get(txid)
    }

    // TODO rewrite with value table
    fn config_height(&self) -> ListTable<MapTable<View, [u8], Vec<u8>>, u64, Height> {
        let prefix = vec![ANCHORING_SERVICE as u8, 04];
        ListTable::new(MapTable::new(prefix, self.view))
    }
}

impl TxAnchoringSignature {
    pub fn execute(&self, view: &View) -> Result<(), StorageError> {
        let schema = AnchoringSchema::new(view);
        let tx = self.tx();
        // Verify signature
        let cfg = schema.current_anchoring_config()?;
        let redeem_script = cfg.redeem_script();
        let ref pub_key = cfg.validators[self.validator() as usize];
        if !tx.verify(&redeem_script, self.input(), &pub_key, self.signature()) {
            error!("Received tx with incorrect signature content={:#?}", self);
            return Ok(());
        }
        schema.signatures(&tx.id()).append(self.clone())
    }
}

impl TxAnchoringUpdateLatest {
    pub fn execute(&self, view: &View) -> Result<(), StorageError> {
        let schema = AnchoringSchema::new(view);
        let tx = self.tx();
        // Verify lect
        schema.add_lect(self.validator(), tx)
    }
}