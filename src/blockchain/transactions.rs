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
        // Check that input is enough for given tx
        if tx.input.len() as u32 <= self.input() {
            warn!("Received msg with incorrect signature content={:#?}", self);
            return false;
        }
        // Check that inputs are empty
        for input in &tx.input {
            if !input.script_sig.is_empty() {
                warn!("Received msg with non empty inputs, content={:#?}", self);
                return false;
            }
        }
        true
    }

    pub fn execute(&self, view: &View) -> Result<(), StorageError> {
        let schema = AnchoringSchema::new(view);

        let tx = self.tx();
        let id = self.validator();
        let cfg = schema.current_anchoring_config()?;
        // Verify signature
        if let Some(pub_key) = cfg.validators.get(id as usize) {
            let (redeem_script, _) = cfg.redeem_script();

            if !tx.verify_input(&redeem_script, self.input(), pub_key, self.signature()) {
                warn!("Received msg with incorrect signature content={:#?}", self);
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
            warn!("Received weird lect msg={:#?}", self);
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
