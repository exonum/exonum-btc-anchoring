// Copyright 2017 The Exonum Team
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

#![cfg_attr(feature = "cargo-clippy", allow(large_enum_variant))]

mod anchoring;
mod auditing;
mod basic;
pub mod error;
pub mod observer;
mod transition;

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::mpsc;

use blockchain::consensus_storage::AnchoringConfig;
use blockchain::dto::MsgAnchoringSignature;
use details::btc;
use details::btc::transactions::{AnchoringTx, BitcoinTx, FundingTx};
use details::rpc::BitcoinRelay;
use local_storage::AnchoringNodeConfig;

/// Internal anchoring service handler. Can be used to manage the service.
#[derive(Debug)]
pub struct AnchoringHandler {
    #[doc(hidden)]
    pub client: Option<Box<BitcoinRelay>>,
    #[doc(hidden)]
    pub node: AnchoringNodeConfig,
    #[doc(hidden)]
    pub proposal_tx: Option<AnchoringTx>,
    #[doc(hidden)]
    pub errors_sink: Option<mpsc::Sender<error::Error>>,
    #[doc(hidden)]
    pub known_addresses: HashSet<String>,
}

#[doc(hidden)]
#[derive(Debug)]
pub struct MultisigAddress<'a> {
    pub common: &'a AnchoringConfig,
    pub priv_key: btc::PrivateKey,
    pub addr: btc::Address,
    pub redeem_script: btc::RedeemScript,
}

#[doc(hidden)]
#[derive(Debug)]
pub enum AnchoringState {
    Anchoring {
        cfg: AnchoringConfig,
    },
    Transition {
        from: AnchoringConfig,
        to: AnchoringConfig,
    },
    Recovering {
        prev_cfg: AnchoringConfig,
        actual_cfg: AnchoringConfig,
    },
    Waiting {
        lect: BitcoinTx,
        confirmations: Option<u64>,
    },
    Auditing {
        cfg: AnchoringConfig,
    },
    Broken,
}

#[doc(hidden)]
#[derive(Debug)]
pub enum LectKind {
    Anchoring(AnchoringTx),
    Funding(FundingTx),
    None,
}

#[doc(hidden)]
/// The function extracts signatures from messages and order them by inputs.
pub fn collect_signatures<I>(
    proposal: &AnchoringTx,
    common: &AnchoringConfig,
    msgs: I,
) -> Option<HashMap<u32, Vec<btc::Signature>>>
where
    I: IntoIterator<Item = MsgAnchoringSignature>,
{
    let mut signatures = HashMap::new();
    for input in proposal.inputs() {
        signatures.insert(input, vec![None; common.anchoring_keys.len()]);
    }

    for msg in msgs {
        let input = msg.input();
        let validator = msg.validator().0 as usize;

        let signatures_by_input = signatures.get_mut(&input).unwrap();
        signatures_by_input[validator] = Some(msg.signature().to_vec());
    }

    let majority_count = common.majority_count() as usize;

    // remove "holes" from signatures preserve order
    let mut actual_signatures = HashMap::new();
    for (input, signatures) in signatures {
        let signatures = signatures
            .into_iter()
            .filter_map(|x| x)
            .take(majority_count)
            .collect::<Vec<_>>();

        trace!(
            "signatures for input={}, count={}, majority_count={}",
            input,
            signatures.len(),
            majority_count
        );
        if signatures.len() < majority_count {
            return None;
        }
        actual_signatures.insert(input, signatures);
    }
    Some(actual_signatures)
}
