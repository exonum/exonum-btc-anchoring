use exonum::blockchain::NodeState;
use exonum::storage::{List, Error as StorageError};
use exonum::crypto::ToHex;

use bitcoin::util::base58::ToBase58;

use btc;
use client::AnchoringRpc;
use config::{AnchoringNodeConfig, AnchoringConfig};
use transactions::{BitcoinTx, AnchoringTx, FundingTx};

use super::{AnchoringSchema, TxKind};
use super::error::Error as ServiceError;
use super::schema::{FollowingConfig, TxAnchoringUpdateLatest, AnchoringTransaction};

pub struct AnchoringHandler {
    pub client: AnchoringRpc,
    pub node: AnchoringNodeConfig,
    pub proposal_tx: Option<AnchoringTx>,
}

#[derive(Debug)]
pub struct MultisigAddress<'a> {
    pub genesis: &'a AnchoringConfig,
    pub priv_key: btc::PrivateKey,
    pub addr: btc::Address,
    pub redeem_script: btc::RedeemScript,
}

#[derive(Debug)]
pub enum AnchoringState {
    Anchoring { cfg: AnchoringConfig },
    Transferring {
        from: AnchoringConfig,
        to: AnchoringConfig,
    },
    Waiting { cfg: AnchoringConfig },
    Broken,
}

pub enum LectKind {
    Anchoring(AnchoringTx),
    Funding(FundingTx),
    None,
}

impl AnchoringHandler {
    pub fn new(client: AnchoringRpc, node: AnchoringNodeConfig) -> AnchoringHandler {
        AnchoringHandler {
            client: client,
            node: node,
            proposal_tx: None,
        }
    }

    pub fn multisig_address<'a>(&self, genesis: &'a AnchoringConfig) -> MultisigAddress<'a> {
        let (redeem_script, addr) = genesis.redeem_script();
        let priv_key = self.node.private_keys[&addr.to_base58check()].clone();
        MultisigAddress {
            genesis: genesis,
            priv_key: priv_key,
            redeem_script: redeem_script,
            addr: addr,
        }
    }

    pub fn add_private_key(&mut self, addr: &btc::Address, priv_key: btc::PrivateKey) {
        self.node.private_keys.insert(addr.to_base58check(), priv_key);
    }

    pub fn actual_config(&self, state: &NodeState) -> Result<AnchoringConfig, ServiceError> {
        let schema = AnchoringSchema::new(state.view());
        let genesis = schema.current_anchoring_config()?;
        Ok(genesis)
    }

    pub fn following_config(&self,
                            state: &NodeState)
                            -> Result<Option<FollowingConfig>, ServiceError> {
        let schema = AnchoringSchema::new(state.view());
        let cfg = schema.following_anchoring_config()?;
        Ok(cfg)
    }

    pub fn current_state(&self, state: &NodeState) -> Result<AnchoringState, ServiceError> {
        let actual = self.actual_config(state)?;
        let state = if let Some(cfg) = self.following_config(state)? {
            if actual.redeem_script().1 != cfg.config.redeem_script().1 {
                AnchoringState::Transferring {
                    from: actual,
                    to: cfg.config,
                }
            } else {
                AnchoringState::Anchoring { cfg: actual }
            }
        } else {
            let schema = AnchoringSchema::new(state.view());

            let current_lect = schema.lect(state.id())?;
            if let Some(prev_lect) = schema.prev_lect(state.id())? {
                let current_lect = if let TxKind::Anchoring(tx) = TxKind::from(current_lect) {
                    tx
                } else {
                    return Ok(AnchoringState::Anchoring { cfg: actual });
                };
                let prev_lect = if let TxKind::Anchoring(tx) = TxKind::from(prev_lect) {
                    tx
                } else {
                    return Ok(AnchoringState::Anchoring { cfg: actual });
                };

                debug!("current_lect={:#?}", current_lect);
                debug!("prev_lect={:#?}", prev_lect);

                let current_addr = current_lect.output_address(actual.network());
                if current_addr != actual.redeem_script().1 {
                    AnchoringState::Waiting { cfg: actual }
                } else {
                    let prev_addr = prev_lect.output_address(actual.network());
                    if current_addr != prev_addr {
                        let confirmations = current_lect.confirmations(&self.client)?
                            .unwrap_or_else(|| 0);

                        if confirmations < actual.utxo_confirmations {
                            AnchoringState::Waiting { cfg: actual }
                        } else {
                            AnchoringState::Anchoring { cfg: actual }
                        }
                    } else {
                        AnchoringState::Anchoring { cfg: actual }
                    }
                }
            } else {
                AnchoringState::Anchoring { cfg: actual }
            }
        };
        Ok(state)
    }

    pub fn handle_commit(&mut self, state: &mut NodeState) -> Result<(), ServiceError> {
        match self.current_state(state)? {
            AnchoringState::Anchoring { cfg } => self.handle_anchoring_state(cfg, state),
            AnchoringState::Transferring { from, to } => {
                self.handle_transferring_state(from, to, state)
            }
            AnchoringState::Waiting { cfg } => self.handle_waiting_state(cfg, state),
            AnchoringState::Broken => panic!("Broken anchoring state detected!"),
        }
    }

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

    // Перебираем все анкорящие транзакции среди listunspent и ищем среди них
    // ту единственную, у которой prev_hash содержится в нашем массиве lectов
    // или первую funding транзакцию, если все анкорящие пропали
    pub fn find_lect(&self,
                     state: &NodeState,
                     addr: &btc::Address)
                     -> Result<Option<BitcoinTx>, ServiceError> {
        let lects: Vec<_> = self.client.unspent_transactions(addr)?;
        let schema = AnchoringSchema::new(state.view());
        let id = state.id();

        debug!("lects={:#?}", lects);

        let find_lect_deep = |first_funding_tx: &BitcoinTx,
                              mut lect: BitcoinTx|
                              -> Result<Option<BitcoinTx>, ServiceError> {
            let mut times = 1000;
            while times > 0 {
                let kind = TxKind::from(lect.clone());
                match kind {
                    TxKind::FundingTx(tx) => {
                        if &tx == first_funding_tx {
                            return Ok(Some(tx.into()));
                        }
                    }
                    TxKind::Anchoring(tx) => {
                        if schema.find_lect_position(id, &tx.prev_hash())?.is_some() {
                            return Ok(Some(tx.into()));
                        } else {
                            times -= 1;
                            let txid = tx.prev_hash().be_hex_string();
                            lect = self.client.get_transaction(&txid)?;
                            debug!("Check prev lect={:#?}", lect);
                        }
                    }
                    TxKind::Other(_) => return Ok(None),
                }
            }
            Ok(None)
        };

        let first_funding_tx = schema.lects(id).get(0)?.unwrap();
        for lect in lects.into_iter() {
            if let Some(tx) = find_lect_deep(&first_funding_tx, lect)? {
                return Ok(Some(tx));
            }
        }
        Ok(None)
    }

    // Пытаемся обновить нашу последнюю известную анкорящую транзакцию
    // Помимо этого, если мы обнаруживаем, что она набрала достаточно подтверждений
    // для перехода на новый адрес, то переходим на него
    pub fn update_our_lect(&self,
                           multisig: &MultisigAddress,
                           state: &mut NodeState)
                           -> Result<(), ServiceError> {
        debug!("Update our lect");
        if let Some(lect) = self.find_lect(state, &multisig.addr)? {
            /// Случай, когда появился новый lect с другим набором подписей
            let (our_lect, lects_count) = {
                let schema = AnchoringSchema::new(state.view());
                let our_lect = schema.lect(state.id())?;
                let count = schema.lects(state.id()).len()?;
                (our_lect, count)
            };

            debug!("lect={:#?}", lect);
            debug!("our_lect={:#?}", our_lect);

            if lect != our_lect {
                info!("LECT ====== txid={}, total_count={}",
                      lect.txid().to_hex(),
                      lects_count);
                let lect_msg = TxAnchoringUpdateLatest::new(&state.public_key(),
                                                            state.id(),
                                                            lect,
                                                            lects_count,
                                                            &state.secret_key());
                state.add_transaction(AnchoringTransaction::UpdateLatest(lect_msg));
            }
        } else {
            // случай, когда транзакция пропала из за форка и была единственная на этот адрес
            let (lect, our_lect, lects_count) = {
                let schema = AnchoringSchema::new(state.view());

                let lect = {
                    if let Some(lect) = schema.prev_lect(state.id())? {
                        lect
                    } else {
                        warn!("Unable to find previous lect!");
                        return Ok(());
                    }
                };
                let our_lect: AnchoringTx = schema.lect(state.id())?
                    .into();
                let count = schema.lects(state.id()).len()?;
                (lect, our_lect, count)
            };

            if our_lect.output_address(multisig.genesis.network()) == multisig.addr {
                debug!("lect={:#?}", lect);
                debug!("our_lect={:#?}", our_lect);

                info!("PREV_LECT ====== txid={}, total_count={}",
                      lect.txid().to_hex(),
                      lects_count);

                let lect_msg = TxAnchoringUpdateLatest::new(&state.public_key(),
                                                            state.id(),
                                                            lect,
                                                            lects_count,
                                                            &state.secret_key());
                state.add_transaction(AnchoringTransaction::UpdateLatest(lect_msg));
            }
        }
        Ok(())
    }

    pub fn avaliable_funding_tx(&self,
                                multisig: &MultisigAddress)
                                -> Result<Option<FundingTx>, ServiceError> {
        let ref funding_tx = multisig.genesis.funding_tx;
        if let Some(info) = funding_tx.is_unspent(&self.client, &multisig.addr)? {
            debug!("avaliable_funding_tx={:#?}, confirmations={}",
                   funding_tx,
                   info.confirmations);
            if info.confirmations >= multisig.genesis.utxo_confirmations {
                return Ok(Some(funding_tx.clone()));
            }
        }
        Ok(None)
    }
}