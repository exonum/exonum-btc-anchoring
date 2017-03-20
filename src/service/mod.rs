pub mod schema;
pub mod config;

mod handler;
mod anchoring;
mod transferring;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use serde_json::Value;
use serde_json::value::ToJson;
use bitcoin::util::base58::ToBase58;

use exonum::blockchain::{Service, Transaction, NodeState};
use exonum::crypto::Hash;
use exonum::messages::{RawTransaction, Message, FromRaw, Error as MessageError};
use exonum::storage::{View, Error as StorageError};

use {AnchoringRpc, BitcoinSignature};
use transactions::{TxKind, AnchoringTx};
use error::Error as ServiceError;
use service::schema::{ANCHORING_SERVICE, AnchoringMessage, AnchoringSchema, MsgAnchoringSignature};
use service::handler::{LectKind, MultisigAddress};
use service::config::{AnchoringNodeConfig, AnchoringConfig};

pub use self::handler::AnchoringHandler;

pub struct AnchoringService {
    genesis: AnchoringConfig,
    handler: Arc<Mutex<AnchoringHandler>>,
}

impl AnchoringService {
    pub fn new(client: AnchoringRpc,
               genesis: AnchoringConfig,
               cfg: AnchoringNodeConfig)
               -> AnchoringService {
        AnchoringService {
            genesis: genesis,
            handler: Arc::new(Mutex::new(AnchoringHandler::new(client, cfg))),
        }
    }

    pub fn handler(&self) -> Arc<Mutex<AnchoringHandler>> {
        self.handler.clone()
    }
}

impl Transaction for AnchoringMessage {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, view: &View) -> Result<(), StorageError> {
        match *self {
            AnchoringMessage::Signature(ref msg) => msg.execute(view),
            AnchoringMessage::UpdateLatest(ref msg) => msg.execute(view),
        }
    }
}

impl Service for AnchoringService {
    fn service_id(&self) -> u16 {
        ANCHORING_SERVICE
    }

    fn state_hash(&self, _: &View) -> Result<Vec<Hash>, StorageError> {
        Ok(Vec::new())
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        AnchoringMessage::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn handle_genesis_block(&self, view: &View) -> Result<Value, StorageError> {
        let handler = self.handler.lock().unwrap();
        let cfg = self.genesis.clone();
        let (_, addr) = cfg.redeem_script();
        handler.client.importaddress(&addr.to_base58check(), "multisig", false, false).unwrap();

        AnchoringSchema::new(view).create_genesis_config(&cfg)?;
        Ok(cfg.to_json())
    }

    fn handle_commit(&self, state: &mut NodeState) -> Result<(), StorageError> {
        debug!("Handle commit, height={}", state.height());
        match self.handler
                  .lock()
                  .unwrap()
                  .handle_commit(state) {
            Err(ServiceError::Storage(e)) => Err(e),
            Err(e) => {
                error!("An error occured: {:?}", e);
                Ok(())
            }
            Ok(()) => Ok(()),
        }
    }
}

pub fn collect_signatures<'a, I>(proposal: &AnchoringTx,
                                 genesis: &AnchoringConfig,
                                 msgs: I)
                                 -> Option<HashMap<u32, Vec<BitcoinSignature>>>
    where I: Iterator<Item = &'a MsgAnchoringSignature>
{
    let mut signatures = HashMap::new();
    for input in proposal.inputs() {
        signatures.insert(input, vec![None; genesis.validators.len()]);
    }

    for msg in msgs {
        let input = msg.input();
        let validator = msg.validator() as usize;

        let mut signatures_by_input = signatures.get_mut(&input).unwrap();
        signatures_by_input[validator] = Some(msg.signature().to_vec());
    }

    let majority_count = genesis.majority_count() as usize;

    // remove holes from signatures preserve order
    let mut actual_signatures = HashMap::new();
    for (input, signatures) in signatures {
        let signatures = signatures.into_iter()
            .filter_map(|x| x)
            .take(majority_count)
            .collect::<Vec<_>>();

        debug!("signatures for input={}, count={}, majority_count={}",
               input,
               signatures.len(),
               majority_count);
        if signatures.len() < majority_count {
            return None;
        }
        actual_signatures.insert(input, signatures);
    }
    Some(actual_signatures)
}
