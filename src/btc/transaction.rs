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
}

#[derive(Debug)]
pub struct AnchoringTransactionBuilder {
    script_pubkey: Script,
    prev_tx: Option<Transaction>,
    additional_funds: Vec<(usize, Transaction)>,
    fee: Option<u64>,
    payload: Option<(Height, Hash)>,
}

#[derive(Debug, Copy, Clone, PartialEq, Display, Fail)]
pub enum BuilderError {
    #[display(fmt = "Insufficient funds to construct a new anchoring transaction,\
                     total fee is {}, total balance is {}",
              _0, _1)]
    InsufficientFunds { total_fee: u64, balance: u64 },
}

impl AnchoringTransactionBuilder {
    pub fn new(redeem_script: RedeemScript) -> AnchoringTransactionBuilder {
        AnchoringTransactionBuilder {
            script_pubkey: redeem_script.as_ref().to_v0_p2wsh(),
            prev_tx: None,
            additional_funds: Vec::default(),
            fee: None,
            payload: None,
        }
    }

    pub fn prev_tx(&mut self, tx: Transaction) -> &mut Self {
        assert_eq!(
            tx.anchoring_metadata().unwrap().0,
            &self.script_pubkey,
            "Output address in a previous anchoring transaction is not suitable."
        );

        self.prev_tx = Some(tx);
        self
    }

    pub fn fee(&mut self, fee: u64) -> &mut Self {
        self.fee = Some(fee);
        self
    }

    pub fn payload(&mut self, block_height: Height, block_hash: Hash) -> &mut Self {
        self.payload = Some((block_height, block_hash));
        self
    }

    pub fn additional_funds(&mut self, tx: Transaction) -> &mut Self {
        let out = tx.find_out(&self.script_pubkey)
            .expect("Funding transaction doesn't contains outputs to the anchoring address.")
            .0;

        self.additional_funds.push((out, tx));
        self
    }

    pub fn create(mut self) -> Result<Transaction, BuilderError> {
        // Collects inputs.
        let (input, balance) = {
            let mut balance = 0;
            let input = self.prev_tx
                .into_iter()
                .map(|tx| (0, tx))
                .chain(self.additional_funds.into_iter())
                .map(|(out_index, tx)| {
                    let out = &tx.0.output[out_index];
                    balance += out.value;
                    TxIn {
                        prev_hash: tx.0.txid(),
                        prev_index: out_index as u32,
                        script_sig: Script::default(),
                        sequence: 0xFFFFFFFF,
                        witness: Vec::default(),
                    }
                })
                .collect::<Vec<_>>();
            (input, balance)
        };
        // Computes payload script.
        let (block_height, block_hash) = self.payload.take().expect("Payload isn't set.");
        let payload_script = PayloadBuilder::new()
            .block_hash(block_hash)
            .block_height(block_height)
            .into_script();
        // Creates unsigned transaction.
        let mut transaction = Transaction::from(transaction::Transaction {
            version: 2,
            lock_time: 0,
            input,
            output: vec![
                TxOut {
                    value: balance,
                    script_pubkey: self.script_pubkey,
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
        Ok(transaction)
    }
}

#[cfg(test)]
mod tests {
    use exonum::crypto::Hash;
    use exonum::encoding::serialize::FromHex;
    use exonum::helpers::Height;
    use exonum::storage::StorageValue;

    use bitcoin::network::constants::Network;
    use bitcoin::util::address::Address;

    use super::Transaction;

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

    #[test]
    fn test_anchoring_transaction_builder_simple() {
        unimplemented!();
    }

    #[test]
    fn test_anchoring_transaction_builder_funds() {
        unimplemented!();
    }

    #[test]
    fn test_anchoring_transaction_builder_incorrect_prev_tx() {
        unimplemented!();
    }

    #[test]
    fn test_anchoring_transaction_builder_incorrect_funds() {
        unimplemented!();
    }
}
