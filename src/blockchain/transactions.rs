use bitcoin::blockdata::transaction::SigHashType;

use exonum::blockchain::{Schema, Transaction};
use exonum::messages::Message;
use exonum::storage::{View, List, Error as StorageError};

use blockchain::dto::{AnchoringMessage, MsgAnchoringSignature, MsgAnchoringUpdateLatest};
use blockchain::schema::AnchoringSchema;

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
        let schema = AnchoringSchema::new(view);

        let tx = self.tx();
        let id = self.validator();
        // Verify from field
        let actual_cfg = Schema::new(view).get_actual_configuration()?;
        if actual_cfg.validators.get(id as usize) != Some(self.from()) {
            warn!("Received msg from non-validator, content={:#?}", self);
            return Ok(());
        }
        // Verify signature
        let anchoring_cfg = schema.current_anchoring_config()?;
        if let Some(pub_key) = anchoring_cfg.validators.get(id as usize) {
            let (redeem_script, addr) = anchoring_cfg.redeem_script();
            let tx_addr = tx.output_address(anchoring_cfg.network);
            // Use following address if it exists
            let addr = if let Some(following) = schema.following_anchoring_config()? {
                following.config.redeem_script().1
            } else {
                addr
            };
            if tx_addr != addr {
                warn!("Received msg with incorrect output address, content={:#?}",
                      self);
                return Ok(());
            }
            if !tx.verify_input(&redeem_script, self.input(), pub_key, self.signature()) {
                warn!("Received msg with incorrect signature, content={:#?}", self);
                return Ok(());
            }
            schema.add_known_signature(self.clone())
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
        let schema = AnchoringSchema::new(view);

        let tx = self.tx();
        let id = self.validator();
        // Verify lect with actual cfg
        let actual_cfg = Schema::new(view).get_actual_configuration()?;
        if actual_cfg.validators.get(id as usize) != Some(self.from()) {
            warn!("Received lect from non validator, content={:#?}", self);
            return Ok(());
        }
        if schema.lects(id).len()? != self.lect_count() {
            return Ok(());
        }
        schema.add_lect(id, tx)
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
