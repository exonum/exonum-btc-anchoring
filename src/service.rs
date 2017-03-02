use std::sync::{Arc, Mutex, MutexGuard};
use std::collections::HashMap;

use bitcoinrpc::{MultiSig, Error as RpcError};
use serde_json::Value;
use serde_json::value::ToJson;
use bitcoin::util::base58::ToBase58;

use exonum::blockchain::{Service, Transaction, Schema, NodeState};
use exonum::storage::{List, View, Error as StorageError};
use exonum::crypto::{Hash, ToHex};
use exonum::messages::{RawTransaction, Message, FromRaw, Error as MessageError};
use exonum::node::Height;

use config::{AnchoringNodeConfig, AnchoringConfig};
use {BITCOIN_NETWORK, AnchoringRpc, RpcClient, BitcoinPrivateKey, HexValueEx, BitcoinSignature};
use schema::{ANCHORING_SERVICE, AnchoringTransaction, AnchoringSchema, TxAnchoringUpdateLatest,
             TxAnchoringSignature, FollowingConfig};
use transactions::{TxKind, FundingTx, AnchoringTx, BitcoinTx, TransactionBuilder};
use btc;

pub struct AnchoringState {
    proposal_tx: Option<AnchoringTx>,
}

pub struct AnchoringService {
    cfg: AnchoringNodeConfig,
    genesis: AnchoringConfig,
    client: RpcClient,
    state: Arc<Mutex<AnchoringState>>,
}

pub enum LectKind {
    Anchoring(AnchoringTx),
    Funding(FundingTx),
    None,
}

// TODO error chain

// алгоритм разбивается на две ситуации: когда есть переход на новый адрес и когда такого перехода нет

// Код общий для обеих ситуаций
impl AnchoringService {
    pub fn new(client: RpcClient,
               genesis: AnchoringConfig,
               cfg: AnchoringNodeConfig)
               -> AnchoringService {
        let state = AnchoringState { proposal_tx: None };

        AnchoringService {
            cfg: cfg,
            genesis: genesis,
            client: client,
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub fn majority_count(&self, state: &NodeState) -> Result<usize, StorageError> {
        let (_, cfg) = self.actual_config(state)?;
        Ok(cfg.validators.len() * 2 / 3 + 1)
    }

    pub fn client(&self) -> &RpcClient {
        &self.client
    }

    pub fn service_state(&self) -> MutexGuard<AnchoringState> {
        self.state.lock().unwrap()
    }

    pub fn actual_config(&self,
                         state: &NodeState)
                         -> Result<(BitcoinPrivateKey, AnchoringConfig), StorageError> {
        let genesis: AnchoringConfig =
            AnchoringSchema::new(state.view()).current_anchoring_config()?;
        let (redeem_script, _) = genesis.redeem_script();
        let key = self.cfg.private_keys[&redeem_script.to_address(BITCOIN_NETWORK)].clone();
        Ok((key, genesis))
    }

    pub fn following_config(&self,
                            state: &NodeState)
                            -> Result<Option<FollowingConfig>, StorageError> {
        AnchoringSchema::new(state.view()).following_anchoring_config()
    }

    pub fn nearest_anchoring_height(&self, state: &NodeState) -> Result<Height, StorageError> {
        let (_, genesis) = self.actual_config(state)?;
        let height = genesis.nearest_anchoring_height(state.height());
        Ok(height)
    }

    pub fn address_transfer_state(&self, state: &NodeState) -> Result<bool, StorageError> {
        if let Some(_) = self.following_config(state)? {
            Ok(true)
        } else {
            let schema = AnchoringSchema::new(state.view());
            let lects = schema.lects(state.id());
            let last_idx = lects.len()? - 1;
            if last_idx == 0 {
                return Ok(false);
            }

            let current_lect = lects.get(last_idx)?.unwrap();
            if let Some(prev_lect) = lects.get(last_idx - 1)? {
                let current_lect = if let TxKind::Anchoring(tx) = TxKind::from(current_lect) {
                    tx
                } else {
                    return Ok(false);
                };
                let prev_lect = if let TxKind::Anchoring(tx) = TxKind::from(prev_lect) {
                    tx
                } else {
                    return Ok(false);
                };
                debug!("current_lect={:#?}", current_lect);
                debug!("prev_lect={:#?}", prev_lect);

                if current_lect.output_address(BITCOIN_NETWORK) !=
                   prev_lect.output_address(BITCOIN_NETWORK) {
                    let info = current_lect.get_info(&self.client).unwrap().unwrap();
                    let (_, cfg) = self.actual_config(state)?;
                    let confirmations = info.confirmations.unwrap() as u64;
                    Ok(confirmations < cfg.utxo_confirmations)
                } else {
                    Ok(false)
                }
            } else {
                Ok(false)
            }
        }
    }

    pub fn collect_lects(&self, state: &NodeState) -> Result<LectKind, StorageError> {
        let anchoring_schema = AnchoringSchema::new(state.view());

        let our_lect = anchoring_schema.lect(state.id())?;
        let mut count = 1;
        for id in 0..state.validators().len() as u32 {
            let lects = anchoring_schema.lects(id);
            if Some(&our_lect) == lects.last()?.as_ref() {
                count += 1;
            }
        }

        if count >= self.majority_count(state)? {
            match TxKind::from(our_lect) {
                TxKind::Anchoring(tx) => Ok(LectKind::Anchoring(tx)),
                TxKind::FundingTx(tx) => Ok(LectKind::Funding(tx)),
                TxKind::Other(_) => panic!("We are fucked up..."),
            }
        } else {
            Ok(LectKind::None)
        }
    }

    pub fn sign_proposal_tx(&self,
                            state: &mut NodeState,
                            proposal: AnchoringTx,
                            redeem_script: &btc::RedeemScript,
                            private_key: &BitcoinPrivateKey)
                            -> Result<(), RpcError> {
        debug!("sign proposal tx");
        for input in proposal.inputs() {
            let signature = proposal.sign(&redeem_script, input, &private_key);

            let sign_msg = TxAnchoringSignature::new(state.public_key(),
                                                     state.id(),
                                                     proposal.clone(),
                                                     input,
                                                     &signature,
                                                     state.secret_key());

            debug!("Sign_msg={:#?}, sighex={}", sign_msg, signature.to_hex());
            state.add_transaction(AnchoringTransaction::Signature(sign_msg));
        }
        self.service_state().proposal_tx = Some(proposal);
        Ok(())
    }

    pub fn try_finalize_proposal_tx(&self,
                                    state: &mut NodeState,
                                    proposal: AnchoringTx)
                                    -> Result<(), RpcError> {
        debug!("try finalize proposal tx");
        let txid = proposal.id();
        let (_, genesis) = self.actual_config(state).unwrap();

        let proposal_height = proposal.payload().0;
        if genesis.nearest_anchoring_height(state.height()) !=
           genesis.nearest_anchoring_height(proposal_height) {
            warn!("Unable to finalize anchoring tx for height={}",
                  proposal_height);
            self.service_state().proposal_tx = None;
            return Ok(());
        }

        let msgs = AnchoringSchema::new(state.view())
            .signatures(&txid)
            .values()
            .unwrap();

        if let Some(signatures) = collect_signatures(&proposal, &genesis, msgs.iter()) {
            let (redeem_script, _) = genesis.redeem_script();
            let new_lect = proposal.finalize(&redeem_script, signatures)?;
            if new_lect.get_info(self.client())?.is_none() {
                self.client.send_transaction(new_lect.clone().into())?;
            }

            debug!("sended signed_tx={:#?}, to={}",
                   new_lect,
                   new_lect.output_address(BITCOIN_NETWORK).to_base58check());

            info!("ANCHORING ====== anchored_height={}, txid={}, remaining_funds={}",
                  new_lect.payload().0,
                  new_lect.txid().to_hex(),
                  new_lect.amount());

            self.service_state().proposal_tx = None;

            let prev_txid = new_lect.prev_hash();
            let lect_msg = TxAnchoringUpdateLatest::new(state.public_key(),
                                                        state.id(),
                                                        new_lect.into(),
                                                        &prev_txid,
                                                        state.secret_key());
            state.add_transaction(AnchoringTransaction::UpdateLatest(lect_msg));
        }
        Ok(())
    }


    pub fn proposal_tx(&self) -> Option<AnchoringTx> {
        self.service_state().proposal_tx.clone()
    }

    pub fn avaliable_funding_tx(&self, state: &NodeState) -> Result<Option<FundingTx>, RpcError> {
        let (_, genesis) = self.actual_config(state).unwrap();

        let (redeem_script, _) = genesis.redeem_script();
        let addr = btc::Address::from_script(&redeem_script, genesis.network());
        if let Some(info) = genesis.funding_tx
            .is_unspent(&self.client, &addr)? {
            if info.confirmations >= genesis.utxo_confirmations {
                return Ok(Some(genesis.funding_tx));
            }
        }
        Ok(None)
    }

    // Перебираем все анкорящие транзакции среди listunspent и ищем среди них
    // ту единственную, у которой prev_hash содержится в нашем массиве lectов
    // или первую funding транзакцию, если все анкорящие пропали
    pub fn find_lect(&self,
                     state: &NodeState,
                     addr: &btc::Address)
                     -> Result<Option<BitcoinTx>, RpcError> {
        let lects: Vec<_> = self.client().unspent_lects(addr)?;
        let schema = AnchoringSchema::new(state.view());
        let id = state.id();

        debug!("lects={:#?}", lects);

        let first_funding_tx = schema.lects(id).get(0).unwrap().unwrap();
        for lect in lects.into_iter() {
            let kind = TxKind::from(lect);
            match kind {
                TxKind::FundingTx(tx) => {
                    if tx == first_funding_tx {
                        return Ok(Some(tx.into()));
                    }
                }
                TxKind::Anchoring(tx) => {
                    if schema.find_lect_position(id, &tx.prev_hash()).unwrap().is_some() {
                        return Ok(Some(tx.into()));
                    }
                }
                TxKind::Other(_) => {}
            }
        }
        Ok(None)
    }

    // Пытаемся обновить нашу последнюю известную анкорящую транзакцию
    // Помимо этого, если мы обнаруживаем, что она набрала достаточно подтверждений
    // для перехода на новый адрес, то переходим на него
    pub fn update_our_lect(&self,
                           state: &mut NodeState,
                           addr: &btc::Address)
                           -> Result<(), RpcError> {
        debug!("Update our lect");
        // We needs to update our lect
        if let Some(lect) = self.find_lect(state, &addr)? {
            let our_lect = AnchoringSchema::new(state.view())
                .lect(state.id())
                .unwrap();

            debug!("lect={:#?}", lect);
            debug!("our_lect={:#?}", our_lect);

            if lect != our_lect {
                info!("LECT ====== txid={}", lect.txid().to_hex());
                let lect_msg = TxAnchoringUpdateLatest::new(&state.public_key(),
                                                            state.id(),
                                                            lect,
                                                            &our_lect.id(),
                                                            &state.secret_key());
                state.add_transaction(AnchoringTransaction::UpdateLatest(lect_msg));
            }
        } else {
            // TODO
            // если у последней транзакции в базе выход на addr, то значит она не прошла
            // и нужно вставлять предпоследнюю
        }
        Ok(())
    }

    pub fn try_create_proposal_tx(&self, state: &mut NodeState) -> Result<(), RpcError> {
        match self.collect_lects(state).unwrap() {
            LectKind::Funding(_) => self.create_first_proposal_tx(state),
            LectKind::Anchoring(tx) => {
                let (_, genesis) = self.actual_config(state).unwrap();
                let anchored_height = tx.payload().0;
                let nearest_anchored_height = genesis.nearest_anchoring_height(state.height());
                if nearest_anchored_height > anchored_height {
                    return self.create_proposal_tx(state,
                                                   tx,
                                                   genesis.redeem_script().1,
                                                   nearest_anchored_height);
                }
                Ok(())
            }
            LectKind::None => {
                warn!("Unable to reach consensus in a lect");
                Ok(())
            }
        }
    }

    pub fn create_proposal_tx(&self,
                              state: &mut NodeState,
                              lect: AnchoringTx,
                              to: btc::Address,
                              height: Height)
                              -> Result<(), RpcError> {
        let (priv_key, genesis) = self.actual_config(state).unwrap();
        let genesis: AnchoringConfig = genesis;

        // Create proposal tx
        let (redeem_script, from) = genesis.redeem_script();
        let hash = Schema::new(state.view())
            .heights()
            .get(height)
            .unwrap()
            .unwrap();
        let funding_tx = self.avaliable_funding_tx(state)?
            .into_iter()
            .collect::<Vec<_>>();
        let proposal = lect.proposal(from, to.clone(), genesis.fee, &funding_tx, height, hash)?;
        debug!("proposal={:#?}, to={:?}, height={}, hash={}",
               proposal,
               to,
               height,
               hash.to_hex());
        // Sign proposal
        self.sign_proposal_tx(state, proposal, &redeem_script, &priv_key)
    }

    // Create first anchoring tx proposal from funding tx in AnchoringNodeConfig
    pub fn create_first_proposal_tx(&self, state: &mut NodeState) -> Result<(), RpcError> {
        debug!("Create first proposal tx");
        if let Some(funding_tx) = self.avaliable_funding_tx(state)? {
            // Create anchoring proposal
            let height = self.nearest_anchoring_height(state).unwrap();
            let hash = Schema::new(state.view())
                .heights()
                .get(height)
                .unwrap()
                .unwrap();

            let (priv_key, genesis) = self.actual_config(state).unwrap();
            let (redeem_script, addr) = genesis.redeem_script();

            let out = funding_tx.find_out(&addr).unwrap();
            let proposal = TransactionBuilder::with_prev_tx(&funding_tx, out)
                .fee(genesis.fee)
                .payload(height, hash)
                .send_to(addr)
                .into_transaction();

            debug!("initial_proposal={:#?}, txhex={}",
                   proposal,
                   proposal.0.to_hex());

            // Sign proposal
            self.sign_proposal_tx(state, proposal, &redeem_script, &priv_key)?;
        } else {
            warn!("Funding transaction is not suitable.");
        }
        Ok(())
    }
}

// код для случая обычного процесса анкоринга
impl AnchoringService {
    pub fn handle_anchoring(&self, state: &mut NodeState) -> Result<(), RpcError> {
        let (_, genesis) = self.actual_config(state).unwrap();
        let (_, addr) = genesis.redeem_script();

        if state.height() % self.cfg.check_lect_frequency == 0 {
            // First of all we try to update our lect and actual configuration
            self.update_our_lect(state, &addr)?;
        }
        // Now if we have anchoring tx proposal we must try to finalize it
        if let Some(proposal) = self.proposal_tx() {
            self.try_finalize_proposal_tx(state, proposal)?;
        } else {
            // Or try to create proposal
            self.try_create_proposal_tx(state)?;
        }
        Ok(())
    }
}

// код для случая обновления конфигурации
impl AnchoringService {
    pub fn handle_update_address(&self, state: &mut NodeState) -> Result<(), RpcError> {
        let (_, actual) = self.actual_config(state).unwrap();
        if let Some(following) = self.following_config(state).unwrap() {
            let (redeem_script, addr) = following.config.redeem_script();
            // Точно так же обновляем lect каждые n блоков
            if state.height() % self.cfg.check_lect_frequency == 0 {
                // First of all we try to update our lect and actual configuration
                self.update_our_lect(state, &addr)?;
            }

            // Now if we have anchoring tx proposal we must try to finalize it
            if let Some(proposal) = self.proposal_tx() {
                self.try_finalize_proposal_tx(state, proposal)?;
            } else {
                // Or try to create proposal
                match self.collect_lects(state).unwrap() {
                    LectKind::Anchoring(lect) => {
                        debug!("lect={:#?}", lect);
                        // в этом случае ничего делать не нужно
                        if lect.output_address(actual.network()) == addr {
                            return Ok(());
                        }

                        debug!("lect_addr={}",
                               lect.output_address(BITCOIN_NETWORK).to_base58check());
                        debug!("following_addr={:?}", addr);
                        // проверяем, что нам хватает подтверждений
                        let info = lect.get_info(&self.client)?.unwrap();
                        debug!("info={:?}", info);
                        if info.confirmations.unwrap() as u64 >= actual.utxo_confirmations {
                            // FIXME зафиксировать высоту для анкоринга
                            let height = self.nearest_anchoring_height(state).unwrap();
                            self.create_proposal_tx(state, lect, addr, height)?;
                        } else {
                            warn!("Insufficient confirmations for create transfer transaction")
                        }
                    }
                    LectKind::Funding(_) => panic!("We must not to change genesis configuration!"),
                    LectKind::None => {
                        warn!("Unable to reach consensus in a lect");
                    }
                }
            }
        } else {
            let (_, addr) = actual.redeem_script();
            // Точно так же обновляем lect каждые n блоков
            if state.height() % self.cfg.check_lect_frequency == 0 {
                // First of all we try to update our lect and actual configuration
                self.update_our_lect(state, &addr)?;
            }
        }
        Ok(())
    }
}

impl Transaction for AnchoringTransaction {
    fn verify(&self) -> bool {
        self.verify_signature(self.from())
    }

    fn execute(&self, view: &View) -> Result<(), StorageError> {
        match *self {
            AnchoringTransaction::Signature(ref msg) => msg.execute(view),
            AnchoringTransaction::UpdateLatest(ref msg) => msg.execute(view),
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
        AnchoringTransaction::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn handle_genesis_block(&self, view: &View) -> Result<Value, StorageError> {
        let cfg = self.genesis.clone();
        let (_, addr) = cfg.redeem_script();
        self.client
            .importaddress(&addr.to_base58check(), "multisig", false, false)
            .unwrap();

        AnchoringSchema::new(view).create_genesis_config(&cfg)?;
        Ok(cfg.to_json())
    }

    fn handle_commit(&self, state: &mut NodeState) -> Result<(), StorageError> {
        debug!("Handle commit, height={}", state.height());
        if self.address_transfer_state(state)? {
            debug!("Address transfer state");
            let _ = self.handle_update_address(state)
                .log_error("Unable to process config transfer");
        } else {
            debug!("Normal anchoring state");
            let _ = self.handle_anchoring(state)
                .log_error("Unable to process anchoring");
        }
        Ok(())
    }
}

pub fn collect_signatures<'a, I>(proposal: &AnchoringTx,
                                 genesis: &AnchoringConfig,
                                 msgs: I)
                                 -> Option<HashMap<u32, Vec<BitcoinSignature>>>
    where I: Iterator<Item = &'a TxAnchoringSignature>
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
    for (input, signatures) in signatures.into_iter() {
        let signatures = signatures.into_iter()
            .filter_map(|x| x)
            .take(majority_count)
            .collect::<Vec<_>>();

        if signatures.len() < majority_count {
            return None;
        }
        actual_signatures.insert(input, signatures);
    }
    Some(actual_signatures)
}

trait LogError: Sized {
    fn log_error<S: AsRef<str>>(self, msg: S) -> Self;
}

impl<T, E> LogError for ::std::result::Result<T, E>
    where E: ::std::fmt::Display
{
    fn log_error<S: AsRef<str>>(self, msg: S) -> Self {
        if let Err(ref error) = self {
            error!("{}, an error occured: {}", msg.as_ref(), error);
        }
        self
    }
}
