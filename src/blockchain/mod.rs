// Copyright 2018 The Exonum Team
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

pub use self::schema::BtcAnchoringSchema;
pub use self::transactions::Transactions;

use exonum::helpers::Height;

use bitcoin::blockdata::script::Script;
use btc_transaction_utils::multisig::RedeemScript;
use btc_transaction_utils::p2wsh;

use btc::Address;
use config::GlobalConfig;

pub mod data_layout;
pub mod errors;
pub mod schema;
pub mod transactions;

#[derive(Debug, Clone)]
pub enum BtcAnchoringState {
    Regular {
        actual_configuration: GlobalConfig,
    },
    Transition {
        actual_configuration: GlobalConfig,
        following_configuration: GlobalConfig,
    },
}

impl BtcAnchoringState {
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

    pub fn script_pubkey(&self) -> Script {
        self.redeem_script().as_ref().to_v0_p2wsh()
    }

    pub fn output_address(&self) -> Address {
        p2wsh::address(&self.redeem_script(), self.actual_configuration().network).into()
    }

    pub fn is_regular(&self) -> bool {
        if let BtcAnchoringState::Regular { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn is_transition(&self) -> bool {
        if let BtcAnchoringState::Transition { .. } = self {
            true
        } else {
            false
        }
    }

    pub fn actual_configuration(&self) -> &GlobalConfig {
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

    pub fn following_configuration(&self) -> Option<&GlobalConfig> {
        match self {
            BtcAnchoringState::Regular { .. } => None,
            BtcAnchoringState::Transition {
                ref following_configuration,
                ..
            } => Some(following_configuration),
        }
    }

    pub fn following_anchoring_height(&self, latest_anchored_height: Option<Height>) -> Height {
        latest_anchored_height
            .map(|height| match self {
                BtcAnchoringState::Regular {
                    ref actual_configuration,
                } => actual_configuration.following_anchoring_height(height),
                BtcAnchoringState::Transition { .. } => height,
            })
            .unwrap_or_else(Height::zero)
    }
}
