//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use bitcoin::blockdata::transaction;

#[derive(Debug, Clone, From, Into, PartialEq)]
pub struct Transaction(pub transaction::Transaction);

impl_wrapper_for_bitcoin_type! { Transaction }

#[cfg(test)]
mod tests {
    use exonum::encoding::serialize::{FromHex};
    use exonum::storage::StorageValue;

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

}
