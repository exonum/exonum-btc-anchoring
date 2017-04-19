use bitcoin::blockdata::transaction::SigHashType;

use exonum::blockchain::{Schema, Transaction};
use exonum::messages::Message;
use exonum::storage::{View, List, Error as StorageError};

use blockchain::dto::{AnchoringMessage, MsgAnchoringSignature, MsgAnchoringUpdateLatest};
use blockchain::schema::AnchoringSchema;
use blockchain::consensus_storage::AnchoringConfig;
use details::btc::transactions::{TxKind, FundingTx, AnchoringTx};

impl MsgAnchoringSignature {
    pub fn verify_content(&self) -> bool {
        // Do not verify signatures other than SigHashType::All
        let sighash_type_all = SigHashType::All.as_u32() as u8;
        if self.signature().last() != Some(&sighash_type_all) {
            warn!("Received msg with incorrect signature type, content={:#?}",
                  self);
            return false;
        }
        let tx = self.tx();
        // Check that the signature is provided for an existing anchoring tx input
        if tx.input.len() as u32 <= self.input() {
            warn!("Received msg for non-existing input index, content={:#?}",
                  self);
            return false;
        }
        // Check that input scriptSigs are empty
        for input in &tx.input {
            if !input.script_sig.is_empty() {
                warn!("Received msg with non empty input scriptSigs, content={:#?}",
                      self);
                return false;
            }
        }
        true
    }

    pub fn execute(&self, view: &View) -> Result<(), StorageError> {
        let anchoring_schema = AnchoringSchema::new(view);

        let tx = self.tx();
        let id = self.validator();
        // Verify from field
        let schema = Schema::new(view);
        let actual_cfg = schema.get_actual_configuration()?;
        if actual_cfg.validators.get(id as usize) != Some(self.from()) {
            warn!("Received msg from non-validator, content={:#?}", self);
            return Ok(());
        }
        // Verify signature
        let anchoring_cfg = anchoring_schema.current_anchoring_config()?;
        if let Some(pub_key) = anchoring_cfg.validators.get(id as usize) {
            let (redeem_script, addr) = anchoring_cfg.redeem_script();
            let tx_addr = tx.output_address(anchoring_cfg.network);
            // Use following address if it exists
            let addr = if let Some(following) = anchoring_schema.following_anchoring_config()? {
                following.config.redeem_script().1
            } else {
                addr
            };
            if tx_addr != addr {
                warn!("Received msg with incorrect output address, content={:#?}",
                      self);
                return Ok(());
            }
            if !verify_anchoring_tx_payload(&tx, &schema)? {
                warn!("Received msg with incorrect payload, content={:#?}", self);
                return Ok(());
            }
            if !tx.verify_input(&redeem_script, self.input(), pub_key, self.signature()) {
                warn!("Received msg with incorrect signature, content={:#?}", self);
                return Ok(());
            }
            anchoring_schema.add_known_signature(self.clone())
        } else {
            Ok(())
        }
    }
}

impl MsgAnchoringUpdateLatest {
    pub fn verify_content(&self) -> bool {
        true
    }

    pub fn execute(&self, view: &View) -> Result<(), StorageError> {
        let anchoring_schema = AnchoringSchema::new(view);

        let tx = self.tx();
        let id = self.validator();
        // Verify lect with actual cfg
        let schema = Schema::new(view);
        let actual_cfg = schema.get_actual_configuration()?;
        if actual_cfg.validators.get(id as usize) != Some(self.from()) {
            warn!("Received lect from non validator, content={:#?}", self);
            return Ok(());
        }
        let anchoring_cfg = anchoring_schema.current_anchoring_config()?;
        match TxKind::from(tx.clone()) {
            TxKind::Anchoring(tx) => {
                if !verify_anchoring_tx_payload(&tx, &schema)? {
                    warn!("Received lect with incorrect payload, content={:#?}", self);
                    return Ok(());
                }
            }
            TxKind::FundingTx(tx) => {
                if !verify_funding_tx(&tx, &anchoring_cfg)? {
                    warn!("Received lect with incorrect funding_tx, content={:#?}",
                          self);
                    return Ok(());
                }
            }
            TxKind::Other(_) => panic!("Incorrect fields deserialization."),
        }

        if anchoring_schema.lects(id).len()? != self.lect_count() {
            warn!("Received lect with wrong count, content={:#?}", self);
            return Ok(());
        }
        anchoring_schema.add_lect(id, tx)
    }
}

impl AnchoringMessage {
    pub fn verify_content(&self) -> bool {
        match *self {
            AnchoringMessage::Signature(ref msg) => msg.verify_content(),
            AnchoringMessage::UpdateLatest(ref msg) => msg.verify_content(),
        }
    }
}


impl Transaction for AnchoringMessage {
    fn verify(&self) -> bool {
        self.verify_signature(self.from()) && self.verify_content()
    }

    fn execute(&self, view: &View) -> Result<(), StorageError> {
        match *self {
            AnchoringMessage::Signature(ref msg) => msg.execute(view),
            AnchoringMessage::UpdateLatest(ref msg) => msg.execute(view),
        }
    }
}

fn verify_anchoring_tx_payload(tx: &AnchoringTx, schema: &Schema) -> Result<bool, StorageError> {
    let (height, hash) = tx.payload();
    Ok(schema.heights().get(height)? == Some(hash))
}

fn verify_funding_tx(tx: &FundingTx,
                     anchoring_cfg: &AnchoringConfig)
                     -> Result<bool, StorageError> {
    Ok(tx == &anchoring_cfg.funding_tx)
}
