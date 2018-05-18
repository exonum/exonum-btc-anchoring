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

use exonum::blockchain::{ExecutionResult, Transaction};
use exonum::crypto::{Hash, PublicKey};
use exonum::helpers::ValidatorId;
use exonum::messages::Message;
use exonum::storage::Fork;

use btc_transaction_utils::InputSignatureRef;
use secp256k1::{self, Secp256k1};

use super::data_layout::TxInputId;
use BTC_ANCHORING_SERVICE_ID;

transactions! {
    pub Transactions {
        const SERVICE_ID = BTC_ANCHORING_SERVICE_ID;

        /// Exonum message with the signature for the new anchoring transaction.
        struct Signature {
            /// Public key of validator.
            from: &PublicKey,
            /// Public key index in anchoring public keys list.
            validator: ValidatorId,
            /// Transaction identifier.
            txid: &Hash,
            /// Signed input.
            input: u32,
            /// Signature content.
            content: &[u8]
        }
    }
}

impl Signature {
    pub fn input_id(&self) -> TxInputId {
        TxInputId {
            txid: *self.txid(),
            input: self.input(),
        }
    }

    pub fn input_signature(
        &self,
        context: &Secp256k1,
    ) -> Result<InputSignatureRef, secp256k1::Error> {
        InputSignatureRef::from_bytes(context, self.content())
    }
}

impl Transaction for Signature {
    fn verify(&self) -> bool {
        let context = Secp256k1::without_caps();
        self.input_signature(&context).is_ok() && self.verify_signature(self.from())
    }

    fn execute(&self, _fork: &mut Fork) -> ExecutionResult {
        unimplemented!();
    }
}
