mod anchoring;
mod auditing;
mod transition;
mod basic;
pub mod error;

use std::collections::HashMap;

use details::rpc::AnchoringRpc;
use details::btc;
use details::btc::transactions::{AnchoringTx, FundingTx};
use local_storage::AnchoringNodeConfig;
use blockchain::consensus_storage::AnchoringConfig;
use blockchain::dto::MsgAnchoringSignature;

/// An internal anchoring service handler. Can be used to manage the service.
pub struct AnchoringHandler {
    #[doc(hidden)]
    pub client: Option<AnchoringRpc>,
    #[doc(hidden)]
    pub node: AnchoringNodeConfig,
    #[doc(hidden)]
    pub proposal_tx: Option<AnchoringTx>,
    #[cfg(feature="sandbox_tests")]
    #[doc(hidden)]
    pub errors: Vec<error::Error>
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
    Anchoring { cfg: AnchoringConfig },
    Transition {
        from: AnchoringConfig,
        to: AnchoringConfig,
    },
    Recovering { cfg: AnchoringConfig },
    Auditing { cfg: AnchoringConfig },
    Broken,
}

#[doc(hidden)]
pub enum LectKind {
    Anchoring(AnchoringTx),
    Funding(FundingTx),
    None,
}

#[doc(hidden)]
/// The function extracts signatures from messages and order them by inputs.
pub fn collect_signatures<'a, I>(proposal: &AnchoringTx,
                                 common: &AnchoringConfig,
                                 msgs: I)
                                 -> Option<HashMap<u32, Vec<btc::Signature>>>
    where I: Iterator<Item = &'a MsgAnchoringSignature>
{
    let mut signatures = HashMap::new();
    for input in proposal.inputs() {
        signatures.insert(input, vec![None; common.validators.len()]);
    }

    for msg in msgs {
        let input = msg.input();
        let validator = msg.validator() as usize;

        let mut signatures_by_input = signatures.get_mut(&input).unwrap();
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

        trace!("signatures for input={}, count={}, majority_count={}",
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
