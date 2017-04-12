use bitcoin::util::base58::ToBase58;

use exonum::blockchain::NodeState;
use exonum::storage::{List, Error as StorageError};

use error::Error as ServiceError;
use details::rpc::AnchoringRpc;
use details::btc;
use details::btc::transactions::{TxKind, BitcoinTx, FundingTx};
use local_storage::AnchoringNodeConfig;
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::schema::{AnchoringSchema, FollowingConfig};
use blockchain::dto::{AnchoringMessage, MsgAnchoringUpdateLatest};

use super::{AnchoringHandler, MultisigAddress, AnchoringState, LectKind};

impl AnchoringHandler {
    #[doc(hidden)]
    pub fn new(client: AnchoringRpc, node: AnchoringNodeConfig) -> AnchoringHandler {
        AnchoringHandler {
            client: client,
            node: node,
            proposal_tx: None,
        }
    }

    #[doc(hidden)]
    pub fn multisig_address<'a>(&self, common: &'a AnchoringConfig) -> MultisigAddress<'a> {
        let (redeem_script, addr) = common.redeem_script();
        let priv_key = self.node.private_keys[&addr.to_base58check()].clone();
        MultisigAddress {
            common: common,
            priv_key: priv_key,
            redeem_script: redeem_script,
            addr: addr,
        }
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
                            -> Result<Option<FollowingConfig>, ServiceError> {
        let schema = AnchoringSchema::new(state.view());
        let cfg = schema.following_anchoring_config()?;
        Ok(cfg)
    }

    #[doc(hidden)]
    pub fn current_state(&self, state: &NodeState) -> Result<AnchoringState, ServiceError> {
        let actual = self.actual_config(state)?;
        let state = if let Some(cfg) = self.following_config(state)? {
            if actual.redeem_script().1 != cfg.config.redeem_script().1 {
                AnchoringState::Transition {
                    from: actual,
                    to: cfg.config,
                }
            } else {
                AnchoringState::Anchoring { cfg: actual }
            }
        } else {
            let schema = AnchoringSchema::new(state.view());

            let current_lect = schema.lect(state.id())?;
            if let TxKind::Anchoring(current_lect) = TxKind::from(current_lect) {
                let current_addr = current_lect.output_address(actual.network);

                if current_addr != actual.redeem_script().1 {
                    AnchoringState::Recoverring { cfg: actual }
                } else {
                    AnchoringState::Anchoring { cfg: actual }
                }
            } else {
                AnchoringState::Anchoring { cfg: actual }
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
            AnchoringState::Recoverring { cfg } => self.handle_recovering_state(cfg, state),
            AnchoringState::Broken => panic!("Broken anchoring state detected!"),
        }
    }

    #[doc(hidden)]
    pub fn collect_lects(&self, state: &NodeState) -> Result<LectKind, StorageError> {
        let anchoring_schema = AnchoringSchema::new(state.view());

        let our_lect = anchoring_schema.lect(state.id())?;
        let mut count = 1;

        let validators_count = state.validators().len() as u32;
        for id in 0..validators_count {
            let lects = anchoring_schema.lects(id);
            if Some(&our_lect) == lects.last()?.as_ref() {
                count += 1;
            }
        }

        if count >= ::majority_count(validators_count as u8) {
            match TxKind::from(our_lect) {
                TxKind::Anchoring(tx) => Ok(LectKind::Anchoring(tx)),
                TxKind::FundingTx(tx) => Ok(LectKind::Funding(tx)),
                TxKind::Other(_) => panic!("We are fucked up..."),
            }
        } else {
            Ok(LectKind::None)
        }
    }

    #[doc(hidden)]
    /// We list unspent transaction by 'listunspent' and search among
    /// them only one that prev_hash is exists in our `lects` or it equals first `funding_tx`
    /// if all `lects` have disappeared.
    pub fn find_lect(&self,
                     multisig: &MultisigAddress,
                     state: &NodeState)
                     -> Result<Option<BitcoinTx>, ServiceError> {
        let lects: Vec<_> = self.client.unspent_transactions(&multisig.addr)?;
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
        trace!("Update our lect");
        // убеждаемся, что нам известен этот адрес
        {
            let schema = AnchoringSchema::new(state.view());
            if !schema.is_address_known(&multisig.addr)? {
                let addr = multisig.addr.to_base58check();
                self.client
                    .importaddress(&addr, "multisig", false, false)?;
                schema.add_known_address(&multisig.addr)?;

                trace!("Add address to known, addr={}", addr);
            }
        }

        if let Some(lect) = self.find_lect(multisig, state)? {
            /// Случай, когда появился новый lect с другим набором подписей
            let (our_lect, lects_count) = {
                let schema = AnchoringSchema::new(state.view());
                let our_lect = schema.lect(state.id())?;
                let count = schema.lects(state.id()).len()?;
                (our_lect, count)
            };

            if lect != our_lect {
                self.send_updated_lect(lect.clone(), lects_count, state)?;
            }

            Ok(Some(lect.into()))
        } else {
            let (prev_lect, current_lect, lects_count) = {
                let schema = AnchoringSchema::new(state.view());

                let prev_lect = schema.prev_lect(state.id())?.map(TxKind::from);
                let current_lect = TxKind::from(schema.lect(state.id())?);
                let lects_count = schema.lects(state.id()).len()?;
                (prev_lect, current_lect, lects_count)
            };

            if let (Some(TxKind::Anchoring(prev_lect)), TxKind::Anchoring(current_lect)) =
                (prev_lect, current_lect) {

                let network = multisig.common.network;
                let prev_lect_addr = prev_lect.output_address(network);
                let current_lect_addr = current_lect.output_address(network);

                if current_lect_addr == multisig.addr && current_lect_addr != prev_lect_addr {
                    self.send_updated_lect(prev_lect.into(), lects_count, state)?;
                }
            }
            Ok(None)
        }
    }

    #[doc(hidden)]
    pub fn avaliable_funding_tx(&self,
                                multisig: &MultisigAddress)
                                -> Result<Option<FundingTx>, ServiceError> {
        let funding_tx = &multisig.common.funding_tx;
        if let Some(info) = funding_tx
               .has_unspent_info(&self.client, &multisig.addr)? {
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
        let id = state.id();
        let first_funding_tx = schema.lects(id).get(0)?.unwrap();

        // Check that we know tx
        if schema.find_lect_position(id, &lect.id())?.is_some() {
            return Ok(Some(lect.into()));
        }

        let mut times = 10000;
        let mut current_tx = lect.clone();
        while times > 0 {
            let kind = TxKind::from(current_tx.clone());
            match kind {
                TxKind::FundingTx(tx) => {
                    if tx == first_funding_tx {
                        return Ok(Some(lect.into()));
                    } else {
                        return Ok(None);
                    }
                }
                TxKind::Anchoring(tx) => {
                    let lect_addr = tx.output_address(multisig.common.network);
                    if !schema.is_address_known(&lect_addr)? {
                        break;
                    }
                    if schema
                           .find_lect_position(id, &tx.prev_hash())?
                           .is_some() {
                        return Ok(Some(lect.into()));
                    } else {
                        times -= 1;
                        let txid = tx.prev_hash().be_hex_string();
                        current_tx = self.client.get_transaction(&txid)?;
                        trace!("Check prev lect={:#?}", current_tx);
                    }
                }
                TxKind::Other(_) => return Ok(None),
            }
        }
        Ok(None)
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
                                                     state.id(),
                                                     lect.clone(),
                                                     lects_count,
                                                     state.secret_key());
        state.add_transaction(AnchoringMessage::UpdateLatest(lect_msg));
        // Cache lect
        AnchoringSchema::new(state.view())
            .add_lect(state.id(), lect)?;
        Ok(())
    }
}
