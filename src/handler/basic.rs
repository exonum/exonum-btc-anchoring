use bitcoin::util::base58::ToBase58;

use exonum::blockchain::NodeState;
use exonum::storage::{Error as StorageError, List};

use error::Error as ServiceError;
use handler::error::Error as HandlerError;
use details::rpc::AnchoringRpc;
use details::btc;
use details::btc::transactions::{BitcoinTx, FundingTx, TxKind};
use local_storage::AnchoringNodeConfig;
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::schema::AnchoringSchema;
use blockchain::dto::{AnchoringMessage, MsgAnchoringUpdateLatest};

use super::{AnchoringHandler, AnchoringState, LectKind, MultisigAddress};

impl AnchoringHandler {
    #[cfg(not(feature="sandbox_tests"))]
    #[doc(hidden)]
    pub fn new(client: Option<AnchoringRpc>, node: AnchoringNodeConfig) -> AnchoringHandler {
        AnchoringHandler {
            client: client,
            node: node,
            proposal_tx: None,
        }
    }

    #[cfg(feature="sandbox_tests")]
    #[doc(hidden)]
    pub fn new(client: Option<AnchoringRpc>, node: AnchoringNodeConfig) -> AnchoringHandler {
        AnchoringHandler {
            client: client,
            node: node,
            proposal_tx: None,
            errors: Vec::new(),
        }
    }

    #[doc(hidden)]
    pub fn validator_id(&self, state: &NodeState) -> u32 {
        state
            .validator_state()
            .as_ref()
            .expect("Request `validator_id` only from validator node.")
            .id()
    }

    #[doc(hidden)]
    pub fn validator_key<'a>(&self,
                             cfg: &'a AnchoringConfig,
                             state: &NodeState)
                             -> &'a btc::PublicKey {
        let validator_id = state
            .validator_state()
            .as_ref()
            .expect("Request `validator_id` only from validator node.")
            .id();
        &cfg.validators[validator_id as usize]
    }

    #[doc(hidden)]
    pub fn client(&self) -> &AnchoringRpc {
        self.client
            .as_ref()
            .expect("Bitcoind client needs to be present for validator node")
    }

    #[doc(hidden)]
    pub fn multisig_address<'a>(&self, common: &'a AnchoringConfig) -> MultisigAddress<'a> {
        let (redeem_script, addr) = common.redeem_script();
        let addr_str = addr.to_base58check();
        let priv_key = self.node
            .private_keys
            .get(&addr_str)
            .expect(&format!("Expected private key for address={}", addr_str))
            .clone();
        MultisigAddress {
            common: common,
            priv_key: priv_key,
            redeem_script: redeem_script,
            addr: addr,
        }
    }

    #[doc(hidden)]
    pub fn import_address(&self,
                          addr: &btc::Address,
                          state: &NodeState)
                          -> Result<(), ServiceError> {
        let schema = AnchoringSchema::new(state.view());
        if !schema.is_address_known(addr)? {
            let addr_str = addr.to_base58check();
            self.client()
                .importaddress(&addr_str, "multisig", false, false)?;
            schema.add_known_address(addr)?;

            trace!("Add address to known, addr={}", addr_str);
        }
        Ok(())
    }

    /// Adds a private_key for the corresponding anchoring address.
    pub fn add_private_key(&mut self, addr: &btc::Address, priv_key: btc::PrivateKey) {
        self.node
            .private_keys
            .insert(addr.to_base58check(), priv_key);
    }

    #[doc(hidden)]
    pub fn actual_config(&self, state: &NodeState) -> Result<AnchoringConfig, ServiceError> {
        let schema = AnchoringSchema::new(state.view());
        let common = schema.current_anchoring_config()?;
        Ok(common)
    }

    #[doc(hidden)]
    pub fn following_config(&self,
                            state: &NodeState)
                            -> Result<Option<AnchoringConfig>, ServiceError> {
        let schema = AnchoringSchema::new(state.view());
        let cfg = schema.following_anchoring_config()?;
        Ok(cfg)
    }

    fn following_config_is_transition
        (&self,
         actual_addr: &btc::Address,
         state: &NodeState)
         -> Result<Option<(AnchoringConfig, btc::Address)>, ServiceError> {
        if let Some(following) = self.following_config(state)? {
            let following_addr = following.redeem_script().1;
            if actual_addr != &following_addr {
                return Ok(Some((following, following_addr)));
            }
        }
        Ok(None)
    }

    #[doc(hidden)]
    pub fn current_state(&self, state: &NodeState) -> Result<AnchoringState, ServiceError> {
        let actual = self.actual_config(state)?;
        let actual_addr = actual.redeem_script().1;
        let anchoring_schema = AnchoringSchema::new(state.view());

        // Ensure that bitcoind watching for the current addr
        self.import_address(&actual_addr, state)?;

        if state.validator_state().is_none() {
            return Ok(AnchoringState::Auditing { cfg: actual });
        }

        let key = self.validator_key(&actual, state).clone();

        // If we do not have any 'lect', then we have been added
        // later and can only be in the anchoring state.
        let current_lect = if let Some(lect) = anchoring_schema.lect(&key)? {
            lect
        } else {
            return Ok(AnchoringState::Anchoring { cfg: actual });
        };

        // Check that the following cfg exists and its anchoring address is different.
        let result = self.following_config_is_transition(&actual_addr, state)?;
        let state = if let Some((following, following_addr)) = result {
            // Ensure that bitcoind watching for following addr.
            self.import_address(&following_addr, state)?;

            match TxKind::from(current_lect) {
                TxKind::Anchoring(lect) => {
                    let lect_addr = lect.output_address(actual.network);
                    if lect_addr == following_addr {
                        let confirmations = get_confirmations(self.client(), &lect.txid())?;
                        // Lect now is transition transaction
                        AnchoringState::Waiting {
                            lect: lect.into(),
                            confirmations: confirmations,
                        }
                    } else {
                        AnchoringState::Transition {
                            from: actual,
                            to: following,
                        }
                    }
                }
                TxKind::FundingTx(lect) => {
                    debug_assert_eq!(&lect, actual.funding_tx());
                    AnchoringState::Transition {
                        from: actual,
                        to: following,
                    }
                }
                TxKind::Other(tx) => panic!("Incorrect lect found={:#?}", tx),
            }
        } else {
            match TxKind::from(current_lect) {
                TxKind::FundingTx(tx) => {
                    if tx.find_out(&actual_addr).is_some() {
                        trace!("Checking funding_tx={:#?}, txid={}", tx, tx.txid());
                        /// Wait until funding_tx got enough confirmation
                        let confirmations = get_confirmations(self.client(), &tx.txid())?;
                        if !is_enough_confirmations(&actual, confirmations) {
                            let state = AnchoringState::Waiting {
                                lect: tx.into(),
                                confirmations: confirmations,
                            };
                            return Ok(state);
                        }
                        AnchoringState::Anchoring { cfg: actual }
                    } else {
                        AnchoringState::Recovering {
                            prev_cfg: anchoring_schema.previous_anchoring_config()?.unwrap(),
                            actual_cfg: actual,
                        }
                    }
                }
                TxKind::Anchoring(current_lect) => {
                    let current_lect_addr = current_lect.output_address(actual.network);
                    // Ensure that we did not miss transition lect
                    if current_lect_addr != actual_addr {
                        let state = AnchoringState::Recovering {
                            prev_cfg: anchoring_schema.previous_anchoring_config()?.unwrap(),
                            actual_cfg: actual,
                        };
                        return Ok(state);
                    }
                    // If the lect encodes a transition to a new anchoring address,
                    // we need to wait until it reaches enough confirmations.
                    if current_lect_is_transition(&actual,
                                                  &key,
                                                  &current_lect_addr,
                                                  &anchoring_schema)? {
                        let confirmations = get_confirmations(self.client(), &current_lect.txid())?;
                        if !is_enough_confirmations(&actual, confirmations) {
                            let state = AnchoringState::Waiting {
                                lect: current_lect.into(),
                                confirmations: confirmations,
                            };
                            return Ok(state);
                        }
                    }

                    AnchoringState::Anchoring { cfg: actual }
                }
                TxKind::Other(tx) => panic!("Incorrect lect found={:#?}", tx),
            }
        };
        Ok(state)
    }

    #[doc(hidden)]
    pub fn handle_commit(&mut self, state: &mut NodeState) -> Result<(), ServiceError> {
        match self.current_state(state)? {
            AnchoringState::Anchoring { cfg } => self.handle_anchoring_state(cfg, state),
            AnchoringState::Transition { from, to } => {
                self.handle_transition_state(from, to, state)
            }
            AnchoringState::Recovering {
                prev_cfg,
                actual_cfg,
            } => self.handle_recovering_state(prev_cfg, actual_cfg, state),
            AnchoringState::Waiting {
                lect,
                confirmations,
            } => self.handle_waiting_state(lect, confirmations),
            AnchoringState::Auditing { cfg } => self.handle_auditing_state(cfg, state),
            AnchoringState::Broken => panic!("Broken anchoring state detected!"),
        }
    }

    #[doc(hidden)]
    pub fn collect_lects_for_validator(&self,
                                       validator_key: &btc::PublicKey,
                                       anchoring_cfg: &AnchoringConfig,
                                       state: &NodeState)
                                       -> Result<LectKind, StorageError> {
        let anchoring_schema = AnchoringSchema::new(state.view());

        let our_lect = if let Some(lect) = anchoring_schema.lect(validator_key)? {
            lect
        } else {
            return Ok(LectKind::None);
        };

        let mut count = 0;

        let validators_count = state.validators().len() as u32;
        for key in &anchoring_cfg.validators {
            let validators_lect = anchoring_schema.lect(key)?;
            if Some(&our_lect) == validators_lect.as_ref() {
                count += 1;
            }
        }

        if count >= ::majority_count(validators_count as u8) {
            match TxKind::from(our_lect) {
                TxKind::Anchoring(tx) => Ok(LectKind::Anchoring(tx)),
                TxKind::FundingTx(tx) => Ok(LectKind::Funding(tx)),
                TxKind::Other(tx) => panic!("Found incorrect lect transaction, content={:#?}", tx),
            }
        } else {
            Ok(LectKind::None)
        }
    }

    #[doc(hidden)]
    pub fn collect_lects(&self, state: &NodeState) -> Result<LectKind, ServiceError> {
        let anchoring_schema = AnchoringSchema::new(state.view());
        let actual_cfg = anchoring_schema.current_anchoring_config()?;
        let kind = if let Some(lect) = anchoring_schema.collect_lects(&actual_cfg)? {
            match TxKind::from(lect) {
                TxKind::Anchoring(tx) => LectKind::Anchoring(tx),
                TxKind::FundingTx(tx) => LectKind::Funding(tx),
                TxKind::Other(tx) => {
                    let e = HandlerError::IncorrectLect {
                        reason: "Incorrect lect transaction".to_string(),
                        tx: tx.into(),
                    };
                    return Err(e.into());
                }
            }
        } else {
            LectKind::None
        };
        Ok(kind)
    }

    #[doc(hidden)]
    /// We list unspent transaction by 'listunspent' and search among
    /// them only one that prev_hash is exists in our `lects` or it equals first `funding_tx`
    /// if all `lects` have disappeared.
    pub fn find_lect(&self,
                     multisig: &MultisigAddress,
                     state: &NodeState)
                     -> Result<Option<BitcoinTx>, ServiceError> {
        let lects: Vec<_> = self.client().unspent_transactions(&multisig.addr)?;
        for lect in lects {
            if let Some(tx) = self.find_lect_deep(lect, multisig, state)? {
                return Ok(Some(tx));
            }
        }
        Ok(None)
    }

    #[doc(hidden)]
    pub fn update_our_lect(&mut self,
                           multisig: &MultisigAddress,
                           state: &mut NodeState)
                           -> Result<Option<BitcoinTx>, ServiceError> {
        let key = self.validator_key(multisig.common, state);
        trace!("Update our lect");
        if let Some(lect) = self.find_lect(multisig, state)? {
            /// New lect with different signatures set.
            let (our_lect, lects_count) = {
                let schema = AnchoringSchema::new(state.view());
                let our_lect = schema.lect(key)?;
                let count = schema.lects(key).len()?;
                (our_lect, count)
            };

            if Some(&lect) != our_lect.as_ref() {
                self.send_updated_lect(lect.clone(), lects_count, state)?;
            }

            Ok(Some(lect.into()))
        } else {
            Ok(None)
        }
    }

    #[doc(hidden)]
    pub fn avaliable_funding_tx(&self,
                                multisig: &MultisigAddress)
                                -> Result<Option<FundingTx>, ServiceError> {
        let funding_tx = multisig.common.funding_tx();
        trace!("Checking funding_tx={:#?}, addr={} availability",
               funding_tx,
               multisig.addr.to_base58check());
        if let Some(info) = funding_tx
               .has_unspent_info(self.client(), &multisig.addr)? {
            trace!("avaliable_funding_tx={:#?}, confirmations={}",
                   funding_tx,
                   info.confirmations);
            return Ok(Some(funding_tx.clone()));
        }
        Ok(None)
    }

    #[doc(hidden)]
    /// Deep search that check entire previous transaction chain that we know.
    /// Each transaction in chain must be anchoring and we must know its output address.
    /// The first transaction in chain is initial `funding_tx`.
    fn find_lect_deep(&self,
                      lect: BitcoinTx,
                      multisig: &MultisigAddress,
                      state: &NodeState)
                      -> Result<Option<BitcoinTx>, ServiceError> {
        let schema = AnchoringSchema::new(state.view());
        let key = self.validator_key(multisig.common, state);

        // Check that we know tx
        if schema.find_lect_position(key, &lect.id())?.is_some() {
            return Ok(Some(lect.into()));
        }

        let kind = TxKind::from(lect.clone());
        match kind {
            TxKind::FundingTx(tx) => {
                if let Some(first_funding_tx) = schema.lects(key).get(0)? {
                    if tx == first_funding_tx.tx() {
                        Ok(Some(lect.into()))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            TxKind::Anchoring(tx) => {
                let lect_addr = tx.output_address(multisig.common.network);
                if !schema.is_address_known(&lect_addr)? {
                    return Ok(None);
                }
                if schema
                       .find_lect_position(key, &tx.prev_hash())?
                       .is_some() {
                    return Ok(Some(lect.into()));
                }

                let txid = tx.prev_hash();
                let prev_lect = if let Some(tx) = self.client()
                       .get_transaction(&txid.be_hex_string())? {
                    tx
                } else {
                    return Ok(None);
                };

                trace!("Check prev lect={:#?}", prev_lect);

                let lect_height = match TxKind::from(prev_lect) {
                    TxKind::FundingTx(_) => 0,
                    TxKind::Anchoring(tx) => tx.payload().block_height,
                    TxKind::Other(_) => return Ok(None),
                };
                let cfg = schema.anchoring_config_by_height(lect_height)?;

                let mut prev_lect_count = 0;
                for key in &cfg.validators {
                    if schema.find_lect_position(key, &txid)?.is_some() {
                        prev_lect_count += 1;
                    }
                }

                if prev_lect_count >= cfg.majority_count() {
                    Ok(lect.into())
                } else {
                    Ok(None)
                }
            }
            TxKind::Other(_) => Ok(None),
        }
    }

    #[doc(hidden)]
    fn send_updated_lect(&mut self,
                         lect: BitcoinTx,
                         lects_count: u64,
                         state: &mut NodeState)
                         -> Result<(), StorageError> {
        if self.proposal_tx.is_some() {
            self.proposal_tx = None;
        }

        info!("LECT ====== txid={}, total_count={}",
              lect.txid(),
              lects_count);

        let lect_msg = MsgAnchoringUpdateLatest::new(state.public_key(),
                                                     self.validator_id(state),
                                                     lect.clone(),
                                                     lects_count,
                                                     state.secret_key());
        state.add_transaction(AnchoringMessage::UpdateLatest(lect_msg));
        Ok(())
    }
}

/// Transition lects cannot be recovered without breaking of current anchoring chain.
fn current_lect_is_transition(actual: &AnchoringConfig,
                              validator_key: &btc::PublicKey,
                              current_lect_addr: &btc::Address,
                              schema: &AnchoringSchema)
                              -> Result<bool, ServiceError> {
    let r = if let Some(prev_lect) = schema.prev_lect(validator_key)? {
        match TxKind::from(prev_lect) {
            TxKind::Anchoring(prev_lect) => {
                let prev_lect_addr = prev_lect.output_address(actual.network);
                &prev_lect_addr != current_lect_addr
            }
            TxKind::FundingTx(tx) => {
                let genesis_cfg = schema.anchoring_config_by_height(0)?;
                if &tx == genesis_cfg.funding_tx() {
                    let prev_lect_addr = genesis_cfg.redeem_script().1;
                    &prev_lect_addr != current_lect_addr
                } else {
                    false
                }
            }
            TxKind::Other(tx) => panic!("Incorrect prev_lect found={:#?}", tx),
        }
    } else {
        false
    };
    Ok(r)
}

fn get_confirmations(client: &AnchoringRpc, txid: &str) -> Result<Option<u64>, ServiceError> {
    let info = client.get_transaction_info(txid)?;
    Ok(info.and_then(|info| info.confirmations))
}

fn is_enough_confirmations(cfg: &AnchoringConfig, confirmations: Option<u64>) -> bool {
    if let Some(confirmations) = confirmations {
        confirmations >= cfg.utxo_confirmations
    } else {
        false
    }
}
