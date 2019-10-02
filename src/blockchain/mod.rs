// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Blockchain implementation details for the BTC anchoring service.

pub use self::{schema::BtcAnchoringSchema, transactions::Transactions};
pub use crate::proto::SignInput;

use bitcoin::blockdata::script::Script;
use btc_transaction_utils::{multisig::RedeemScript, p2wsh};
use exonum::helpers::Height;

use crate::{btc::Address, config::Config};

pub mod data_layout;
pub mod errors;
pub mod schema;
pub mod transactions;

/// Current state of the BTC anchoring service.
#[derive(Debug, Clone)]
pub enum BtcAnchoringState {
    /// The usual anchoring workflow.
    Regular {
        /// Current anchoring configuration.
        actual_configuration: Config,
    },
    /// The transition from the current anchoring address to the following one.
    Transition {
        /// Current anchoring configuration.
        actual_configuration: Config,
        /// Following anchoring configuration.
        following_configuration: Config,
    },
}

impl BtcAnchoringState {
    /// Return the redeem script corresponding to the address to which the anchoring
    /// transaction will be sent.
    pub fn redeem_script(&self) -> RedeemScript {
        match self {
            BtcAnchoringState::Regular {
                actual_configuration,
            } => actual_configuration.redeem_script(),
            BtcAnchoringState::Transition {
                following_configuration,
                ..
            } => following_configuration.redeem_script(),
        }
    }

    /// Return the `script_pubkey` for the corresponding redeem script.
    pub fn script_pubkey(&self) -> Script {
        self.redeem_script().as_ref().to_v0_p2wsh()
    }

    /// Return the output address for the corresponding redeem script.
    pub fn output_address(&self) -> Address {
        p2wsh::address(&self.redeem_script(), self.actual_configuration().network).into()
    }

    /// Check that anchoring state is regular.
    pub fn is_regular(&self) -> bool {
        if let BtcAnchoringState::Regular { .. } = self {
            true
        } else {
            false
        }
    }

    /// Check that anchoring is in the transition state.
    pub fn is_transition(&self) -> bool {
        if let BtcAnchoringState::Transition { .. } = self {
            true
        } else {
            false
        }
    }

    /// Return the actual anchoring configuration.
    pub fn actual_configuration(&self) -> &Config {
        match self {
            BtcAnchoringState::Regular {
                ref actual_configuration,
            } => actual_configuration,
            BtcAnchoringState::Transition {
                ref actual_configuration,
                ..
            } => actual_configuration,
        }
    }

    /// Return the following anchoring configuration if anchoring is in transition state.
    pub fn following_configuration(&self) -> Option<&Config> {
        match self {
            BtcAnchoringState::Regular { .. } => None,
            BtcAnchoringState::Transition {
                ref following_configuration,
                ..
            } => Some(following_configuration),
        }
    }

    /// Return the nearest following anchoring height for the given height.
    pub fn following_anchoring_height(&self, latest_anchored_height: Option<Height>) -> Height {
        latest_anchored_height.map_or_else(Height::zero, |height| match self {
            BtcAnchoringState::Regular {
                ref actual_configuration,
            } => actual_configuration.following_anchoring_height(height),
            BtcAnchoringState::Transition { .. } => height,
        })
    }
}
