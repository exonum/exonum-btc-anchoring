extern crate exonum;
extern crate sandbox;
extern crate anchoring_service;
extern crate serde;
extern crate serde_json;
extern crate bitcoin;
extern crate bitcoinrpc;
extern crate secp256k1;
extern crate blockchain_explorer;
#[macro_use]
extern crate log;

use std::ops::Deref;

pub use bitcoinrpc::RpcError as JsonRpcError;
pub use bitcoinrpc::Error as RpcError;

use serde_json::value::ToJson;

use bitcoin::util::base58::FromBase58;
use bitcoin::util::address::Privkey;

use exonum::messages::Message;
use exonum::crypto::Hash;

use sandbox::sandbox_with_services;
use sandbox::sandbox::Sandbox;
use sandbox::timestamping::TimestampingService;
use sandbox::sandbox_tests_helper::{SandboxState, VALIDATOR_0, add_one_height_with_transactions};

use anchoring_service::sandbox::{SandboxClient, Request};
use anchoring_service::config::{generate_anchoring_config, AnchoringConfig, AnchoringNodeConfig};
use anchoring_service::{AnchoringService, TxAnchoringSignature, TxAnchoringUpdateLatest,
                        AnchoringTx, FundingTx, HexValue as HexValueEx, collect_signatures};
use anchoring_service::transactions::TransactionBuilder;
use anchoring_service::multisig::sign_input;

#[macro_use]
mod macros;
#[cfg(test)]
mod tests;

pub const ANCHORING_VALIDATOR: u32 = VALIDATOR_0;
pub const ANCHORING_FREQUENCY: u64 = 10;
pub const ANCHORING_FUNDS: u64 = 4000;
pub const CHECK_LECT_FREQUENCY: u64 = 6;

pub struct AnchoringSandboxState {
    pub genesis: AnchoringConfig,
    pub nodes: Vec<AnchoringNodeConfig>,
    pub latest_anchored_tx: Option<(AnchoringTx, Vec<TxAnchoringSignature>)>,
}

impl AnchoringSandboxState {
    pub fn latest_anchored_tx(&self) -> &AnchoringTx {
        &self.latest_anchored_tx.as_ref().unwrap().0
    }

    pub fn latest_anchored_tx_signatures(&self) -> &[TxAnchoringSignature] {
        self.latest_anchored_tx.as_ref().unwrap().1.as_ref()
    }

    pub fn gen_anchoring_tx_with_signatures(&mut self,
                                            sandbox: &Sandbox,
                                            height: u64,
                                            block_hash: Hash,
                                            funds: &[FundingTx],
                                            addr: &str,
                                            amount: u64)
                                            -> (AnchoringTx, Vec<TxAnchoringSignature>) {
        let (propose_tx, signed_tx, signs) = {
            let prev_tx = self.latest_anchored_tx
                .as_ref()
                .map(|x| x.0.deref())
                .unwrap_or(self.genesis.funding_tx.deref());

            let mut builder = TransactionBuilder::with_prev_tx(prev_tx, 0)
                .payload(height, block_hash)
                .send_to(addr, amount);
            for fund in funds {
                let out = fund.find_out(addr).unwrap();
                builder = builder.add_funds(fund, out);
            }

            let tx = builder.into_transaction();
            let signs = self.gen_anchoring_signatures(sandbox, &tx);
            let signed_tx = self.finalize_tx(tx.clone(), signs.as_ref());
            (tx, signed_tx, signs)
        };
        self.latest_anchored_tx = Some((signed_tx, signs.clone()));
        (propose_tx, signs)
    }

    pub fn finalize_tx(&self, tx: AnchoringTx, signs: &[TxAnchoringSignature]) -> AnchoringTx {
        let collected_signs = collect_signatures(&tx, &self.genesis, signs.iter()).unwrap();
        tx.finalize(&self.genesis.multisig().redeem_script, collected_signs)
            .unwrap()
    }

    pub fn gen_anchoring_signatures(&self,
                                    sandbox: &Sandbox,
                                    tx: &AnchoringTx)
                                    -> Vec<TxAnchoringSignature> {
        let multisig = self.genesis.multisig();
        let redeem_script = self.genesis.redeem_script();
        let priv_keys = self.nodes
            .iter()
            .map(|cfg| cfg.private_keys[&multisig.address].clone())
            .collect::<Vec<_>>();

        let mut signs = Vec::new();
        for (validator, key) in priv_keys.iter().enumerate() {
            let priv_key = Privkey::from_base58check(key).unwrap();
            for input in tx.inputs() {
                let signature =
                    sign_input(&tx.0, input as usize, &redeem_script, priv_key.secret_key());
                signs.push(TxAnchoringSignature::new(sandbox.p(validator),
                                                     validator as u32,
                                                     tx.clone(),
                                                     input,
                                                     &signature,
                                                     sandbox.s(validator)));
            }
        }
        signs
    }
}

pub fn anchoring_sandbox() -> (Sandbox, SandboxClient, AnchoringSandboxState) {
    let mut client = SandboxClient::default();
    let (mut genesis, mut nodes) = gen_sandbox_anchoring_config(&mut client);

    // Change default anchoring configs
    genesis.frequency = ANCHORING_FREQUENCY;
    for node in &mut nodes {
        node.check_lect_frequency = CHECK_LECT_FREQUENCY;
    }

    client.expect(vec![request! {
            method: "importaddress",
            params: ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu", "multisig", false, false]
        }]);
    let service = AnchoringService::new(client.clone(),
                                        genesis.clone(),
                                        nodes[ANCHORING_VALIDATOR as usize].clone());
    let sandbox = sandbox_with_services(vec![Box::new(service),
                                             Box::new(TimestampingService::new())]);
    let info = AnchoringSandboxState {
        genesis: genesis,
        nodes: nodes,
        latest_anchored_tx: None,
    };
    (sandbox, client, info)
}

/// Anchor genesis block using funding tx
pub fn anchor_genesis_block(sandbox: &Sandbox,
                            client: &SandboxClient,
                            sandbox_state: &SandboxState,
                            anchoring_state: &mut AnchoringSandboxState) {
    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 999999, ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"]],
            response: [
                {
                    "txid": "a03b10b17fc8b86dd0b1b6ebcc3bc3c6dd4b7173302ef68628f5ed768dbd7049",
                    "vout": 0,
                    "address": "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 50,
                    "spendable": false,
                    "solvable": false
                }
            ]
        }]);

    let (_, signatures) =
        anchoring_state.gen_anchoring_tx_with_signatures(sandbox,
                                                         0,
                                                         sandbox.last_hash(),
                                                         &[],
                                                         "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                                                         3000);
    let anchored_tx = anchoring_state.latest_anchored_tx();
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    sandbox.broadcast(signatures[0].clone());
    client.expect(vec![// TODO add support for error response
                       Request {
                           method: "getrawtransaction",
                           params: vec![anchored_tx.txid().to_json(), 1.to_json()],
                           response: Err(RpcError::NoInformation("Unable to find tx".to_string())),
                       },
                       request! {
            method: "sendrawtransaction",
            params: [anchored_tx.to_hex()]
        }]);

    let signatures = signatures.into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures);

    let txs = [gen_service_tx_lect(sandbox, 0, &anchored_tx.to_hex()),
               gen_service_tx_lect(sandbox, 1, &anchored_tx.to_hex()),
               gen_service_tx_lect(sandbox, 2, &anchored_tx.to_hex()),
               gen_service_tx_lect(sandbox, 3, &anchored_tx.to_hex())];

    sandbox.broadcast(txs[0].raw().clone());
    let txs = txs.into_iter()
        .map(|x| x.raw())
        .cloned()
        .collect::<Vec<_>>();
    add_one_height_with_transactions(sandbox, sandbox_state, txs.as_ref());
}

pub fn anchor_update_lect_normal(sandbox: &Sandbox,
                                 client: &SandboxClient,
                                 sandbox_state: &SandboxState,
                                 anchorign_state: &mut AnchoringSandboxState) {
    anchor_genesis_block(sandbox, client, sandbox_state, anchorign_state);
    // Just add few heights
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"]],
            response: [
                {
                    "txid": "fea0a60f7146e7facf5bb382b80dafb762175bf0d4b6ac4e59c09cd4214d1491",
                    "vout": 0,
                    "address": "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 0,
                    "spendable": false,
                    "solvable": false
                }
            ]
        },
        request! {
            method: "getrawtransaction",
            params: ["fea0a60f7146e7facf5bb382b80dafb762175bf0d4b6ac4e59c09cd4214d1491", 1],
            response: {
                "hash":"fea0a60f7146e7facf5bb382b80dafb762175bf0d4b6ac4e59c09cd4214d1491","hex":"01000000014970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd670100483045022100e6ef3de83437c8dc33a8099394b7434dfb40c73631fc4b0378bd6fb98d8f42b002205635b265f2bfaa6efc5553a2b9e98c2eabdfad8e8de6cdb5d0d74e37f1e198520147304402203bb845566633b726e41322743677694c42b37a1a9953c5b0b44864d9b9205ca10220651b7012719871c36d0f89538304d3f358da12b02dab2b4d74f2981c8177b69601473044022052ad0d6c56aa6e971708f079073260856481aeee6a48b231bc07f43d6b02c77002203a957608e4fbb42b239dd99db4e243776cc55ed8644af21fa80fd9be77a59a60014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02b80b00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280000000000000000f1cb806d27e367f1cac835c22c8cc24c402a019e2d3ea82f7f841c308d830a9600000000","locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"41e2143f6f57b3f3e1ea093acc3e99922f7c3fef7fde65b579adc1db1a5648d3","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        }
        ]);
    add_one_height_with_transactions(sandbox, sandbox_state, &[]);
}

/// Generates config for 4 validators and 10000 funds
pub fn gen_sandbox_anchoring_config(client: &mut SandboxClient)
                                    -> (AnchoringConfig, Vec<AnchoringNodeConfig>) {
    let requests = vec![
        request! {
            method: "getnewaddress",
            params: ["node_0"],
            response: "mwfpMzcF1b63RDM2ggzhwcBE8edsVfctUW"
        },
        request! {
            method: "validateaddress",
            params: ["mwfpMzcF1b63RDM2ggzhwcBE8edsVfctUW"],
            response: {
                "account":"node_0","address":"mwfpMzcF1b63RDM2ggzhwcBE8edsVfctUW","hdkeypath":"m/0'/0'/1611'","hdmasterkeyid":"e2aabb596d105e11c1838c0b6bede91e1f2a95ee","iscompressed":true,"ismine":true,"isscript":false,"isvalid":true,"iswatchonly":false,"pubkey":"03475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c","scriptPubKey":"76a914b12f2284d35eb69554f15950aa96935f43fab7a188ac"
            }
        },
        request! {
            method: "dumpprivkey",
            params: ["mwfpMzcF1b63RDM2ggzhwcBE8edsVfctUW"],
            response: "cVC9eJN5peJemWn1byyWcWDevg6xLNXtACjHJWmrR5ynsCu8mkQE"
        },
        request! {
            method: "getnewaddress",
            params: [
                "node_1"
            ],
            response: "mxaKfmSXj8JR8ZjfPvhi2GFPFbf7cM8tF1"
        },
        request! {
            method: "validateaddress",
            params: [
                "mxaKfmSXj8JR8ZjfPvhi2GFPFbf7cM8tF1"
            ],
            response: 
                {"account":"node_1","address":"mxaKfmSXj8JR8ZjfPvhi2GFPFbf7cM8tF1","hdkeypath":"m/0'/0'/1024'","hdmasterkeyid":"e2aabb596d105e11c1838c0b6bede91e1f2a95ee","iscompressed":true,"ismine":true,"isscript":false,"isvalid":true,"iswatchonly":false,"pubkey":"02a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0","scriptPubKey":"76a914d9e2a44fc0f8321aacc1a76fdffa036c0b4eb02e88ac"}
        },
        request! {
            method: "dumpprivkey",
            params: [
                "mxaKfmSXj8JR8ZjfPvhi2GFPFbf7cM8tF1"
            ],
            response: 
                "cMk66oMazTgquBVaBLHzDi8FMgAaRN3tSf6iZykf9bCh3D3FsLX1"
        },
        request! {
            method: "getnewaddress",
            params: [
                "node_2"
            ],
            response: 
                "mrasCaRhAxTbPpbbwaXQVg9Azyors9g3Zv"
        },
        request! {
            method: "validateaddress",
            params: [
                "mrasCaRhAxTbPpbbwaXQVg9Azyors9g3Zv"
            ],
            response: {
                "account":"node_2","address":"mrasCaRhAxTbPpbbwaXQVg9Azyors9g3Zv","hdkeypath":"m/0'/0'/1025'","hdmasterkeyid":"e2aabb596d105e11c1838c0b6bede91e1f2a95ee","iscompressed":true,"ismine":true,"isscript":false,"isvalid":true,"iswatchonly":false,"pubkey":"0230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb49","scriptPubKey":"76a914df9eb587014e39cda894ab357f43e268de6ace6588ac"
            }
        },
        request! {
            method: "dumpprivkey",
            params: [
                "mrasCaRhAxTbPpbbwaXQVg9Azyors9g3Zv"
            ],
            response: "cT2S5KgUQJ41G6RnakJ2XcofvoxK68L9B44hfFTnH4ddygaxi7rc"
        },
        request! {
            method: "getnewaddress",
            params: [
                "node_3"
            ],
            response: "mrXPgwhZezYj9tkMyZFGJdHrr1kQnK7aDu"
        },
        request! {
            method: "validateaddress",
            params: [
                "mrXPgwhZezYj9tkMyZFGJdHrr1kQnK7aDu"
            ],
            response: {
                "account":"node_3","address":"mrXPgwhZezYj9tkMyZFGJdHrr1kQnK7aDu","hdkeypath":"m/0'/0'/1026'","hdmasterkeyid":"e2aabb596d105e11c1838c0b6bede91e1f2a95ee","iscompressed":true,"ismine":true,"isscript":false,"isvalid":true,"iswatchonly":false,"pubkey":"036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e","scriptPubKey":"76a9141e9098bea4655360446daf63384eb20107c94e7588ac"
            }
        },
        request! {
            method: "dumpprivkey",
            params: [
                "mrXPgwhZezYj9tkMyZFGJdHrr1kQnK7aDu"
            ],
            response: "cRUKB8Nrhxwd5Rh6rcX3QK1h7FosYPw5uzEsuPpzLcDNErZCzSaj"
        },
        request! {
            method: "importaddress",
            params: ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu", "multisig", false, false]
        },
        request! {
            method: "sendtoaddress",
            params: [
                "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                "0.00004"
            ],
            response: "a03b10b17fc8b86dd0b1b6ebcc3bc3c6dd4b7173302ef68628f5ed768dbd7049"
        },
        request! {
            method: "getrawtransaction",
            params: [
                "a03b10b17fc8b86dd0b1b6ebcc3bc3c6dd4b7173302ef68628f5ed768dbd7049",
                1
            ],
            response: {
                "hash":"a03b10b17fc8b86dd0b1b6ebcc3bc3c6dd4b7173302ef68628f5ed768dbd7049","hex":"01000000019532a4022a22226a6f694c3f21216b2c9f5c1c79007eb7d3be06bc2f1f9e52fb000000006a47304402203661efd05ca422fad958b534dbad2e1c7db42bbd1e73e9b91f43a2f7be2f92040220740cf883273978358f25ca5dd5700cce5e65f4f0a0be2e1a1e19a8f168095400012102ae1b03b0f596be41a247080437a50f4d8e825b170770dcb4e5443a2eb2ecab2afeffffff02a00f00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678716e1ff05000000001976a91402f5d7475a10a9c24cea32575bd8993d3fabbfd388ac089e1000","locktime":1089032,"size":223,"txid":"a03b10b17fc8b86dd0b1b6ebcc3bc3c6dd4b7173302ef68628f5ed768dbd7049","version":1,"vin":[{"scriptSig":{"asm":"304402203661efd05ca422fad958b534dbad2e1c7db42bbd1e73e9b91f43a2f7be2f92040220740cf883273978358f25ca5dd5700cce5e65f4f0a0be2e1a1e19a8f168095400[ALL] 02ae1b03b0f596be41a247080437a50f4d8e825b170770dcb4e5443a2eb2ecab2a","hex":"47304402203661efd05ca422fad958b534dbad2e1c7db42bbd1e73e9b91f43a2f7be2f92040220740cf883273978358f25ca5dd5700cce5e65f4f0a0be2e1a1e19a8f168095400012102ae1b03b0f596be41a247080437a50f4d8e825b170770dcb4e5443a2eb2ecab2a"},"sequence":429496729,"txid":"fb529e1f2fbc06bed3b77e00791c5c9f2c6b21213f4c696f6a22222a02a43295","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"],"asm":"OP_HASH160 bff50e89fa259d83f78f2e796f57283ca10d6e67 OP_EQUAL","hex":"a914bff50e89fa259d83f78f2e796f57283ca10d6e6787","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mfnc9mL9APy38WT9gSWpY55wQAgbkp5MqJ"],"asm":"OP_DUP OP_HASH160 02f5d7475a10a9c24cea32575bd8993d3fabbfd3 OP_EQUALVERIFY OP_CHECKSIG","hex":"76a91402f5d7475a10a9c24cea32575bd8993d3fabbfd388ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00655382}],"vsize":223
            }
        },
    ];
    client.expect(requests);
    generate_anchoring_config(client, 4, ANCHORING_FUNDS)
}

pub fn gen_service_tx_lect(sandbox: &Sandbox,
                           validator: u32,
                           txhex: &str)
                           -> TxAnchoringUpdateLatest {
    let tx = AnchoringTx::from_hex(txhex).unwrap();
    TxAnchoringUpdateLatest::new(sandbox.p(validator as usize),
                                 validator,
                                 tx,
                                 sandbox.s(validator as usize))
}