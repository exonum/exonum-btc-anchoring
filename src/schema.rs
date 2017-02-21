use std::fmt;

use byteorder::{BigEndian, ByteOrder};
use serde_json::value::from_value;

use exonum::blockchain::Schema;
use exonum::storage::{ListTable, MerkleTable, List, MapTable, View, Error as StorageError};
use exonum::crypto::{PublicKey, Hash};
use exonum::messages::{RawTransaction, Message, FromRaw, Error as MessageError};
use exonum::node::Height;

use config::AnchoringConfig;
use {AnchoringTx};

pub const ANCHORING_SERVICE: u16 = 3;
const ANCHORING_TRANSACTION_SIGNATURE: u16 = 0;
const ANCHORING_TRANSACTION_LATEST: u16 = 1;

// Подпись за анкорящую транзакцию
message! {
    TxAnchoringSignature {
        const TYPE = ANCHORING_SERVICE;
        const ID = ANCHORING_TRANSACTION_SIGNATURE;
        const SIZE = 52;

        from:           &PublicKey  [00 => 32]
        validator:      u32         [32 => 36]
        tx:             &[u8]       [36 => 44]
        signature:      &[u8]       [44 => 52]
    }
}

// Сообщение об обновлении последней корректной транзакции
message! {
    TxAnchoringUpdateLatest {
        const TYPE = ANCHORING_SERVICE;
        const ID = ANCHORING_TRANSACTION_LATEST;
        const SIZE = 44;

        from:           &PublicKey  [00 => 32]
        validator:      u32         [32 => 36]
        tx:             &[u8]       [36 => 44] // TODO store AnchoringTx
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

impl<'a> AnchoringSchema<'a> {
    pub fn new(view: &'a View) -> AnchoringSchema {
        AnchoringSchema { view: view }
    }

    // хэш это txid
    pub fn signatures(&self,
                      txid: &Hash)
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

    // TODO rewrite with value table
    fn config_height(&self) -> ListTable<MapTable<View, [u8], Vec<u8>>, u64, Height> {
        let prefix = vec![ANCHORING_SERVICE as u8, 04];
        ListTable::new(MapTable::new(prefix, self.view))
    }
}
