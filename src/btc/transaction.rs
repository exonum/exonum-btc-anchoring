//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use exonum::crypto::Hash;
use exonum::helpers::Height;

use bitcoin::blockdata::script::Script;
use bitcoin::blockdata::transaction::{self, TxIn, TxOut};
use btc_transaction_utils::multisig::RedeemScript;

use super::{Payload, PayloadBuilder};

#[derive(Debug, Clone, From, Into, PartialEq)]
pub struct Transaction(pub transaction::Transaction);

impl_wrapper_for_bitcoin_type! { Transaction }

impl AsRef<transaction::Transaction> for Transaction {
    fn as_ref(&self) -> &transaction::Transaction {
        &self.0
    }
}

impl Transaction {
    pub fn id(&self) -> Hash {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&self.0.txid()[..]);
        bytes.reverse();
        Hash::new(bytes)
    }

    pub fn prev_tx_id(&self) -> Hash {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&self.0.input[0].prev_hash[..]);
        bytes.reverse();
        Hash::new(bytes)
    }

    pub fn find_out(&self, script_pubkey: &Script) -> Option<(usize, &TxOut)> {
        self.0
            .output
            .iter()
            .enumerate()
            .find(|out| &out.1.script_pubkey == script_pubkey)
    }

    pub fn anchoring_payload(&self) -> Option<Payload> {
        let out = self.0.output.get(1)?;
        Payload::from_script(&out.script_pubkey)
    }

    pub fn anchoring_metadata(&self) -> Option<(&Script, Payload)> {
        let payload = self.anchoring_payload()?;
        let script_pubkey = self.0.output.get(0).map(|out| &out.script_pubkey)?;
        Some((script_pubkey, payload))
    }

    pub fn unspent_value(&self) -> Option<u64> {
        self.0.output.get(0).map(|out| out.value)
    }
}

#[derive(Debug)]
pub struct BtcAnchoringTransactionBuilder {
    script_pubkey: Script,
    transit_to: Option<Script>,
    prev_tx: Option<Transaction>,
    recovery_tx: Option<Hash>,
    additional_funds: Vec<(usize, Transaction)>,
    fee: Option<u64>,
    payload: Option<(Height, Hash)>,
}

#[derive(Debug, Copy, Clone, PartialEq, Display, Fail)]
pub enum BuilderError {
    #[display(
        fmt = "Insufficient funds to construct a new anchoring transaction,\
               total fee is {}, total balance is {}",
        _0,
        _1
    )]
    InsufficientFunds { total_fee: u64, balance: u64 },
    #[display(fmt = "At least one input should be provided.")]
    NoInputs,
    #[display(fmt = "Output address in a previous anchoring transaction is not suitable.")]
    UnsuitableOutput,
    #[display(fmt = "Funding transaction doesn't contains outputs to the anchoring address.")]
    UnsutableFundingTx,
}

impl BtcAnchoringTransactionBuilder {
    pub fn new(redeem_script: &RedeemScript) -> BtcAnchoringTransactionBuilder {
        BtcAnchoringTransactionBuilder {
            script_pubkey: redeem_script.as_ref().to_v0_p2wsh(),
            transit_to: None,
            prev_tx: None,
            recovery_tx: None,
            additional_funds: Vec::default(),
            fee: None,
            payload: None,
        }
    }

    pub fn transit_to(&mut self, script: Script) {
        self.transit_to = Some(script);
    }

    pub fn prev_tx(&mut self, tx: Transaction) -> Result<(), BuilderError> {
        if tx.anchoring_metadata().unwrap().0 != &self.script_pubkey {
            Err(BuilderError::UnsuitableOutput)
        } else {
            self.prev_tx = Some(tx);
            Ok(())
        }
    }

    pub fn recover(&mut self, last_tx: Hash) {
        self.recovery_tx = Some(last_tx);
    }

    pub fn additional_funds(&mut self, tx: Transaction) -> Result<(), BuilderError> {
        let out = tx.find_out(&self.script_pubkey)
            .ok_or_else(|| BuilderError::UnsutableFundingTx)?
            .0;
        self.additional_funds.push((out, tx));
        Ok(())
    }

    pub fn fee(&mut self, fee: u64) {
        self.fee = Some(fee);
    }

    pub fn payload(&mut self, block_height: Height, block_hash: Hash) {
        self.payload = Some((block_height, block_hash));
    }

    pub fn create(mut self) -> Result<(Transaction, Vec<Transaction>), BuilderError> {
        // Creates transaction inputs.
        let (input, input_transactions, balance) = {
            let mut input = Vec::new();
            let mut input_transactions = Vec::new();
            let mut balance = 0;

            let tx_iter = self.prev_tx
                .into_iter()
                .map(|tx| (0, tx))
                .chain(self.additional_funds.into_iter());
            for (out_index, tx) in tx_iter {
                let txin = TxIn {
                    prev_hash: tx.0.txid(),
                    prev_index: out_index as u32,
                    script_sig: Script::default(),
                    sequence: 0xFFFF_FFFF,
                    witness: Vec::default(),
                };
                balance += tx.0.output[out_index].value;
                input.push(txin);
                input_transactions.push(tx);
            }
            (input, input_transactions, balance)
        };
        // Computes payload script.
        let (block_height, block_hash) = self.payload.take().expect("Payload isn't set.");
        let payload_script = PayloadBuilder::new()
            .block_hash(block_hash)
            .block_height(block_height)
            .prev_tx_chain(self.recovery_tx)
            .into_script();
        let output = match self.transit_to {
            Some(script) => script,
            _ => self.script_pubkey,
        };

        // Creates unsigned transaction.
        let mut transaction = Transaction::from(transaction::Transaction {
            version: 2,
            lock_time: 0,
            input,
            output: vec![
                TxOut {
                    value: balance,
                    script_pubkey: output,
                },
                TxOut {
                    value: 0,
                    script_pubkey: payload_script,
                },
            ],
        });

        // Computes a total fee value.
        let size_in_bytes = {
            let bytes = ::bitcoin::network::serialize::serialize(&transaction.0).unwrap();
            bytes.len() as u64
        };
        let total_fee = self.fee.expect("Fee per byte isn't set.") * size_in_bytes;
        if total_fee > balance {
            return Err(BuilderError::InsufficientFunds { total_fee, balance });
        }
        // Sets the corresponding fee.
        transaction.0.output[0].value -= total_fee;
        Ok((transaction, input_transactions))
    }
}

#[cfg(test)]
mod tests {
    use super::{BtcAnchoringTransactionBuilder, BuilderError, Transaction};
    use bitcoin::blockdata::opcodes::All;
    use bitcoin::blockdata::script::{Builder, Script};
    use bitcoin::blockdata::transaction::{self, TxIn, TxOut};
    use bitcoin::network::constants::Network;
    use bitcoin::util::address::Address;
    use bitcoin::util::hash::Sha256dHash;
    use btc::PublicKey;
    use btc_transaction_utils::multisig::RedeemScriptBuilder;
    use exonum::crypto::CryptoHash;
    use exonum::crypto::Hash;
    use exonum::encoding::serialize::FromHex;
    use exonum::helpers::Height;
    use exonum::storage::StorageValue;
    use std::borrow::Cow;

    #[test]
    fn test_transaction_conversions() {
        let tx_hex = "01000000019aaf09d7e73a5f9ab394f1358bfb3dbde7b15b983d715f5c98f369a3f0a288a700\
        00000000ffffffff02b80b00000000000017a914f18eb74087f751109cc9052befd4177a52c9a30a8700000000\
        000000002c6a2a012800000000000000007fab6f66a0f7a747c820cd01fa30d7bdebd26b91c6e03f742abac0b3\
        108134d900000000";

        let tx = Transaction::from_hex(tx_hex).unwrap();
        assert_eq!(tx.to_string(), tx_hex);

        let bytes = tx.clone().into_bytes();
        let tx2 = Transaction::from_bytes(bytes.into());
        assert_eq!(tx2, tx);
    }

    #[test]
    fn test_segwit_txid() {
        let tx = Transaction::from_hex(
            "02000000000101a4fe140f92eb5fa5a4788b6271a22f07fa91cb2f8ac328cd0065bfc43adb16c90100000\
             01716001446decf32d70ee1fad5aa11d02158810316e6790bfeffffff02a08601000000000017a9147f14\
             23e3359d1754ae9427e313c1d9581f3f280a87e8e520070000000017a914b83c7a100c7ff491e5edb5f1d\
             fcd39e298e50f4b87024830450221008f9378080defdb2029f9c96e149e85e93d8fb860a1c06a7c988908\
             09077eec8b02206049967206a4bd35f8fa4c59a8cd9f46b08e48f794a6b325986b4e9227b9d8d30121037\
             f72563a0750831ab4fb762e01cfe368ddd412042be6b78af5ee5a9bd38d0ed093a81300",
        ).unwrap();
        let txid_hex = "6ed431718c73787ad92e6bcbd6ac7c8151e08dffeeebb6d9e5af2d25b6837d98";

        assert_eq!(tx.id().to_string(), txid_hex);
    }

    #[test]
    fn test_anchoring_tx_metadata() {
        let tx: Transaction = Transaction::from_hex(
            "01000000000101348ead2317da8c6ae12305af07e33b8c0320c9319f21007a704e44f32e7a75500000000\
             000ffffffff0250ec0e00000000002200200f2602a87bbdb59fdf4babfffd568ef39a85cf2f08858c8847\
             d70f27894b68840000000000000000326a3045584f4e554d0100085200000000000085f467f2bad583dbb\
             08f84a47e817d8293fb8c70d033604f441f53a6cc092f180500473044022003334a325c7c382aca17c9c0\
             790d3e2a48fbc99fcc34eb6f73ac4386fcca290602206508789e66f246fc496784df97b0b9e895ae93948\
             cf6a3a1ca2603d02a462c050148304502210081cadbe7c21e9e030b8ed9e3d084595833351284ce26d79d\
             ad889ffdab73bfc402205dd359f19b6871f3e21b9b9c2a57aabd2ce64a0631a136fe4028dabd96fa89a20\
             147304402200dc7a83d7064f74e2e7a90fdfab6b41ab8663b8151ae4e313bf29ee9c7c9f38e022043ca73\
             74050f1f3e321fe686f6858f94b8d8d130e73b61a74e6200f75452edf30169532103f0c44bc5cd2747ac3\
             4380e84ac4e78fac684848d32198bac5985d116c74ae6862103d9d4eb85dad869dc54a65f77a7e81eea0e\
             a5d81129928d6d5b6dcb7b57c8991b21033ea315ab975c6424740c305db3f07b62b1121e27d3052b9a30d\
             b56a8b504713c53ae00000000",
        ).unwrap();
        let (script_pubkey, payload) = tx.anchoring_metadata().unwrap();

        assert_eq!(payload.block_height, Height(21000));
        assert_eq!(
            payload.block_hash,
            "85f467f2bad583dbb08f84a47e817d8293fb8c70d033604f441f53a6cc092f18"
                .parse::<Hash>()
                .unwrap()
        );
        assert_eq!(payload.prev_tx_chain, None);
        assert_eq!(
            Address::p2wsh(script_pubkey, Network::Testnet).to_string(),
            "tb1qgjg3s5u93cuvf5y8pc2aw259gf7spj7x3a4k09lc6a4gtnhg8l0su4axp4"
        );
    }

    proptest! {
        #[test]
        fn test_transaction_exonum_field(input_num in 1usize..4,
                                         prev_index in 1u32..10,
                                         output_num in 1usize..4,
                                         value in 1u64..1_000_000_000,
                                         ref s in "\\PC*") {
            let input = (0..input_num).map(|_| {
                // just random hash
                let prev_hash = Sha256dHash::from_data(s.as_bytes());
                TxIn {
                    prev_hash,
                    prev_index,
                    script_sig: Script::default(),
                    sequence: 0xFFFFFFFF,
                    witness: Vec::default(),
                }
            }).collect::<Vec<_>>();

            let output = (0..output_num).map(|_| {
                TxOut {
                    value,
                    script_pubkey: Builder::new()
                        .push_opcode(All::OP_RETURN)
                        .push_slice(s.as_bytes())
                        .into_script(),
                }
            }).collect::<Vec<_>>();

            let transaction = Transaction::from(transaction::Transaction {
                version: 2,
                lock_time: 0,
                input,
                output,
            });

            let bytes = transaction.clone().into_bytes();
            assert_eq!(
                transaction,
                <Transaction as StorageValue>::from_bytes(Cow::Borrowed(&bytes))
            );
        }
    }

    #[test]
    #[should_panic(expected = "segwit flag 10 not understood")]
    fn test_transaction_exonum_field_invalid_segwit_flag() {
        let hex_tx = "6600000000101b651818fe3855d0d5d74de1cf72b56503c16f808519440e842b6a\
                      dc2dd570c4930100000000feffffff02deaa7b0000000000160014923904449829\
                      cd865cdfb72abdba0806ce9e48911027000000000000220020e9bb049fdff8f8d3\
                      b33b7335978b1dbb268833a32a69906f9e500e4103151bef02483045022100ddc7\
                      eb1193529a8d0e48cf24f536d5fbb5de3b67d2f56c98190ea8585d58a156022075\
                      e33981f1a7d78ce2915402d4b9b38b8d5311e0aef2e3ccf9284d2ce602968d0121\
                      021d0478acd223fb9b2ad7485f06f12914a1b7effc78390a08c50bfe53b3b24815\
                      062c1400";

        let tx_raw = Vec::<u8>::from_hex(hex_tx).unwrap();
        let _ = <Transaction as StorageValue>::from_bytes(Cow::Borrowed(&tx_raw));
    }

    #[test]
    #[should_panic(expected = "failed to fill whole buffer")]
    fn test_transaction_exonum_field_invalid_length() {
        let hex_tx = "66000000000101b651818fe3855d0d5d74de1cf72b56503c16f808519440e842b6\
                      dc2dd570c4930100000000feffffff02deaa7b0000000000160014923904449829\
                      cd865cdfb72abdba0806ce9e48911027000000000000220020e9bb049fdff8f8d3\
                      b33b7335978b1dbb268833a32a69906f9e500e4103151bef02483045022100ddc7\
                      eb1193529a8d0e48cf24f536d5fbb5de3b67d2f56c98190ea8585d58a156022075\
                      021d0478acd223fb9b2ad7485f06f12914a1b7effc78390a08c50bfe53b3b24815\
                      062c1400";

        let tx_raw = Vec::<u8>::from_hex(hex_tx).unwrap();
        let _ = <Transaction as StorageValue>::from_bytes(Cow::Borrowed(&tx_raw));
    }

    #[test]
    fn test_anchoring_transaction_builder_simple() {
        let funding_tx: Transaction = Transaction::from_hex(
            "02000000000101b651818fe3855d0d5d74de1cf72b56503c16f808519440e842b6\
             dc2dd570c4930100000000feffffff02deaa7b0000000000160014923904449829\
             cd865cdfb72abdba0806ce9e48911027000000000000220020e9bb049fdff8f8d3\
             b33b7335978b1dbb268833a32a69906f9e500e4103151bef02483045022100ddc7\
             eb1193529a8d0e48cf24f536d5fbb5de3b67d2f56c98190ea8585d58a156022075\
             e33981f1a7d78ce2915402d4b9b38b8d5311e0aef2e3ccf9284d2ce602968d0121\
             021d0478acd223fb9b2ad7485f06f12914a1b7effc78390a08c50bfe53b3b24815\
             062c1400",
        ).unwrap();

        let keys = vec![
            "038b782f94d19f34536a96e12e0bad99e6f82c838fa16a4234572f5f132d95ba29",
            "020ae2216f42575c4196864eda0252c75c61273065f691b32be9a99cb2a3c9b4d1",
            "02536d5e1464b961562da57207e4a46edb7dade9b92aa29712ca8309c8aba5be5b",
        ].iter()
            .map(|h| PublicKey::from_hex(h).unwrap().0.clone())
            .collect::<Vec<_>>();

        let redeem_script = RedeemScriptBuilder::with_public_keys(keys)
            .to_script()
            .unwrap();

        let mut builder = BtcAnchoringTransactionBuilder::new(&redeem_script);
        builder.additional_funds(funding_tx.clone()).unwrap();
        builder.fee(1);
        builder.payload(Height::zero(), funding_tx.hash());
        let (tx, inputs) = builder.create().unwrap();

        assert_eq!(funding_tx, inputs[0]);
        assert_eq!(tx.0.version, 2);

        let inputs = tx.0.input;
        assert_eq!(inputs.len(), 1);

        let outputs = tx.0.output;
        assert_eq!(outputs.len(), 2);

        let out_0 = &outputs[0];
        let out_1 = &outputs[1];

        assert_ne!(out_0.value, 0);
        assert_eq!(out_1.value, 0);
    }

    #[test]
    fn test_anchoring_transaction_builder_funds() {
        let funding_tx0: Transaction = Transaction::from_hex(
            "02000000000101b651818fe3855d0d5d74de1cf72b56503c16f808519440e842b6\
             dc2dd570c4930100000000feffffff02deaa7b0000000000160014923904449829\
             cd865cdfb72abdba0806ce9e48911027000000000000220020e9bb049fdff8f8d3\
             b33b7335978b1dbb268833a32a69906f9e500e4103151bef02483045022100ddc7\
             eb1193529a8d0e48cf24f536d5fbb5de3b67d2f56c98190ea8585d58a156022075\
             e33981f1a7d78ce2915402d4b9b38b8d5311e0aef2e3ccf9284d2ce602968d0121\
             021d0478acd223fb9b2ad7485f06f12914a1b7effc78390a08c50bfe53b3b24815\
             062c1400",
        ).unwrap();
        let funding_tx1: Transaction = Transaction::from_hex(
            "020000000001018aa4065d472efc80d2a9f26bf0f77aabd5b8fcb45661de8a0161\
             cbcc6b5fef9e0000000000feffffff0235837b00000000001600143e9fd2829e66\
             868739ddbb8c397a3e35ae02a5151027000000000000220020e9bb049fdff8f8d3\
             b33b7335978b1dbb268833a32a69906f9e500e4103151bef0247304402201d2f3c\
             a3ec4c82071b825a44c5b8a7455e4e50caef07e988bbe46554846e445702205f1b\
             066bf6d747c06b3721ac878104e434e977e0e321191a0c860f05fb3bb319012103\
             b475c0164be599df74ea5d4b669fe1c439953e40eea2d4958d66698f26eeaa5f2a\
             2c1400",
        ).unwrap();
        let funding_tx2: Transaction = Transaction::from_hex(
            "0200000000010115c9acef986ba57a7fcf43c6cb60221b70af1da6d3ad6d1e2480\
             e55bc80c559c00000000171600147881a57eadd9361c497e2b1671da4ed1c0ac1e\
             44feffffff02a086010000000000220020e9bb049fdff8f8d3b33b7335978b1dbb\
             268833a32a69906f9e500e4103151bef406df6000000000016001424ff8bab4afa\
             feca816e4a8300e135045ce15f6b02483045022100f6b55f77ec53e339d150637a\
             76de5436165c27ea415a8175f5fdff634bf91cd402204252dbd3af0ba8a7490912\
             68491169dca4477515e2b3155de04ffacfa39f00d4012102ad0617b920ce3a7a48\
             1a10222344a7b338e7a13e8e725eb44a3a53354a90f9e32a2c1400",
        ).unwrap();

        let keys = vec![
            "038b782f94d19f34536a96e12e0bad99e6f82c838fa16a4234572f5f132d95ba29",
            "020ae2216f42575c4196864eda0252c75c61273065f691b32be9a99cb2a3c9b4d1",
            "02536d5e1464b961562da57207e4a46edb7dade9b92aa29712ca8309c8aba5be5b",
        ].iter()
            .map(|h| PublicKey::from_hex(h).unwrap().0.clone())
            .collect::<Vec<_>>();

        let redeem_script = RedeemScriptBuilder::with_public_keys(keys)
            .to_script()
            .unwrap();

        let mut builder = BtcAnchoringTransactionBuilder::new(&redeem_script);
        builder.additional_funds(funding_tx0.clone()).unwrap();
        builder.additional_funds(funding_tx1.clone()).unwrap();
        builder.additional_funds(funding_tx2.clone()).unwrap();
        builder.fee(1);
        builder.payload(Height::zero(), funding_tx0.hash());
        let (tx, inputs) = builder.create().unwrap();

        assert_eq!(inputs.len(), 3);
        let inputs = tx.0.input;
        assert_eq!(inputs.len(), 3);

        let outputs = tx.0.output;
        assert_eq!(outputs.len(), 2);

        let out_0 = &outputs[0];
        let out_1 = &outputs[1];

        assert_ne!(out_0.value, 0);
        assert_eq!(out_1.value, 0);
    }

    #[test]
    fn test_anchoring_transaction_builder_incorrect_prev_tx() {
        let funding_tx: Transaction = Transaction::from_hex(
            "02000000000101b651818fe3855d0d5d74de1cf72b56503c16f808519440e842b6\
             dc2dd570c4930100000000feffffff02deaa7b0000000000160014923904449829\
             cd865cdfb72abdba0806ce9e48911027000000000000220020e9bb049fdff8f8d3\
             b33b7335978b1dbb268833a32a69906f9e500e4103151bef02483045022100ddc7\
             eb1193529a8d0e48cf24f536d5fbb5de3b67d2f56c98190ea8585d58a156022075\
             e33981f1a7d78ce2915402d4b9b38b8d5311e0aef2e3ccf9284d2ce602968d0121\
             021d0478acd223fb9b2ad7485f06f12914a1b7effc78390a08c50bfe53b3b24815\
             062c1400",
        ).unwrap();

        let keys = vec![
            "038b782f94d19f34536a96e12e0bad99e6f82c838fa16a4234572f5f132d95ba29",
            "020ae2216f42575c4196864eda0252c75c61273065f691b32be9a99cb2a3c9b4d1",
            "02536d5e1464b961562da57207e4a46edb7dade9b92aa29712ca8309c8aba5be5b",
        ].iter()
            .map(|h| PublicKey::from_hex(h).unwrap().0.clone())
            .collect::<Vec<_>>();

        let prev_tx: Transaction = Transaction::from_hex(
            "01000000000101348ead2317da8c6ae12305af07e33b8c0320c9319f21007a704e44f32e7a75500000000\
             000ffffffff0250ec0e00000000002200200f2602a87bbdb59fdf4babfffd568ef39a85cf2f08858c8847\
             d70f27894b68840000000000000000326a3045584f4e554d0100085200000000000085f467f2bad583dbb\
             08f84a47e817d8293fb8c70d033604f441f53a6cc092f180500473044022003334a325c7c382aca17c9c0\
             790d3e2a48fbc99fcc34eb6f73ac4386fcca290602206508789e66f246fc496784df97b0b9e895ae93948\
             cf6a3a1ca2603d02a462c050148304502210081cadbe7c21e9e030b8ed9e3d084595833351284ce26d79d\
             ad889ffdab73bfc402205dd359f19b6871f3e21b9b9c2a57aabd2ce64a0631a136fe4028dabd96fa89a20\
             147304402200dc7a83d7064f74e2e7a90fdfab6b41ab8663b8151ae4e313bf29ee9c7c9f38e022043ca73\
             74050f1f3e321fe686f6858f94b8d8d130e73b61a74e6200f75452edf30169532103f0c44bc5cd2747ac3\
             4380e84ac4e78fac684848d32198bac5985d116c74ae6862103d9d4eb85dad869dc54a65f77a7e81eea0e\
             a5d81129928d6d5b6dcb7b57c8991b21033ea315ab975c6424740c305db3f07b62b1121e27d3052b9a30d\
             b56a8b504713c53ae00000000",
        ).unwrap();

        let redeem_script = RedeemScriptBuilder::with_public_keys(keys)
            .to_script()
            .unwrap();
        let mut builder = BtcAnchoringTransactionBuilder::new(&redeem_script);

        builder.additional_funds(funding_tx).unwrap();

        assert_matches!(
            builder.prev_tx(prev_tx).unwrap_err(),
            BuilderError::UnsuitableOutput
        );
    }

    #[test]
    fn test_anchoring_transaction_builder_incorrect_funds() {
        let funding_tx: Transaction = Transaction::from_hex(
            "020000000001015315c18b6a6893ec08d4a7175da494d5d856a8efc983ba2e8eed06c2211041f500000000\
             00feffffff028c5b7b0000000000160014d00d6944fb51eda26fb450809bd3388cf51462d4102700000000\
             0000220020c622abf40381f842e9a860c2353405a0c73534093a6fe9c661fd487c5bc36d99024730440220\
             5345da1affdc26eda5415c31a8fd9afd6280d42f411c43ec4e7a4f284d67da4102205039e7fe4a2cce1864\
             91f88d319e6001a45078c95b384f293669f00793e1e94b0121022b2bc757808dd27b3490b372764ff61872\
             1b11577d52837da3d0daac467df91c2f2c1400"
        ).unwrap();

        let keys = vec![
            "038b782f94d19f34536a96e12e0bad99e6f82c838fa16a4234572f5f132d95ba29",
            "020ae2216f42575c4196864eda0252c75c61273065f691b32be9a99cb2a3c9b4d1",
            "02536d5e1464b961562da57207e4a46edb7dade9b92aa29712ca8309c8aba5be5b",
        ].iter()
            .map(|h| PublicKey::from_hex(h).unwrap().0.clone())
            .collect::<Vec<_>>();

        let redeem_script = RedeemScriptBuilder::with_public_keys(keys)
            .to_script()
            .unwrap();

        let mut builder = BtcAnchoringTransactionBuilder::new(&redeem_script);
        assert_matches!(
            builder.additional_funds(funding_tx).unwrap_err(),
            BuilderError::UnsutableFundingTx
        );
    }
}
