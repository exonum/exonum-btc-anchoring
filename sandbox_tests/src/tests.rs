use serde_json::value::ToJson;

use exonum::messages::Message;

use sandbox::sandbox_tests_helper::{SandboxState, add_one_height_with_transactions};

use anchoring_service::sandbox::{SandboxClient, Request};
use anchoring_service::AnchoringTx;
use anchoring_service::HexValue;

use super::{RpcError, anchoring_sandbox, gen_sandbox_anchoring_config, gen_service_tx_lect,
            anchor_genesis_block, anchor_update_lect_normal, TransactionBuilder};

#[test]
fn test_rpc_getnewaddress() {
    let client = SandboxClient::default();
    client.expect(vec![request! {
                           method: "getnewaddress",
                           params: ["maintain"],
                           response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
                       }]);
    let addr = client.getnewaddress("maintain").unwrap();
    assert_eq!(addr, "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY");
}

#[test]
#[should_panic(expected = "expected response for method=getnewaddress")]
fn test_rpc_expected_request() {
    let client = SandboxClient::default();
    client.getnewaddress("useroid").unwrap();
}

#[test]
#[should_panic(expected = "assertion failed")]
fn test_rpc_wrong_request() {
    let client = SandboxClient::default();
    client.expect(vec![request! {
                           method: "getnewaddress",
                           params: ["maintain"],
                           response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
                       }]);
    client.getnewaddress("useroid").unwrap();
}

#[test]
#[should_panic(expected = "assertion failed")]
fn test_rpc_uneexpected_request() {
    let client = SandboxClient::default();
    client.expect(vec![request! {
                           method: "getnewaddress",
                           params: ["maintain"],
                           response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
                       },
                       request! {
                           method: "getnewaddress",
                           params: ["maintain2"],
                           response: "mmoXxKhBwnhtFiAMvxJ82CKCBia751mzfY"
                       }]);
    client.getnewaddress("useroid").unwrap();
    client.expect(vec![request! {
                           method: "getnewaddress",
                           params: ["maintain"],
                           response: "mmoXxKhAwnhtFiAMvxJ82CKCBia751mzfY"
                       }]);
}

#[test]
fn test_rpc_validateaddress() {
    let client = SandboxClient::default();
    client.expect(vec![request! {
                           method: "validateaddress",
                           params: ["n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb"],
                           response: {
                               "account":"node_0","address":"n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb","hdkeypath":"m/0'/0'/1023'","hdmasterkeyid":"e2aabb596d105e11c1838c0b6bede91e1f2a95ee","iscompressed":true,"ismine":true,"isscript":false,"isvalid":true,"iswatchonly":false,"pubkey":"0394a06ac465776c110cb43d530663d7e7df5684013075988917f02ff007edd364","scriptPubKey":"76a914e7588549f0c4149e7949cd7ea933cfcdde45f8c888ac"
                           }
                       }]);
    client.validateaddress("n2cCRtaXxRAbmWYhH9sZUBBwqZc8mMV8tb").unwrap();
}

#[test]
fn test_generate_anchoring_config() {
    let mut client = SandboxClient::default();
    gen_sandbox_anchoring_config(&mut client);
}

#[test]
fn test_anchoring_sandbox() {
    anchoring_sandbox();
}

#[test]
fn test_anchoring_genesis_block() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchorign_state) = anchoring_sandbox();
    let sandbox_state = SandboxState::new();
    anchor_genesis_block(&sandbox, &client, &sandbox_state, &mut anchorign_state);
}

#[test]
fn test_anchoring_update_lect_normal() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchorign_state) = anchoring_sandbox();
    let sandbox_state = SandboxState::new();
    anchor_update_lect_normal(&sandbox, &client, &sandbox_state, &mut anchorign_state);
}

#[test]
fn test_anchoring_update_lect_different() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchorign_state) = anchoring_sandbox();
    let sandbox_state = SandboxState::new();
    anchor_genesis_block(&sandbox, &client, &sandbox_state, &mut anchorign_state);
    // Just add few heights
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    client.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9999999, ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"]],
            response: [
                {
                    "txid": "0ee74770757714703c33883ef2b7314137650e879fc91cd963fdc584eb3b5e7b",
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
            params: ["0ee74770757714703c33883ef2b7314137650e879fc91cd963fdc584eb3b5e7b", 1],
            response: {
                "hash":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","hex":"0100000001cdf7b60662e819fd226ff2fd1dc2dafad6886e5e5f68bbd5ce6f2c88e35c906900000000fd6701004830450221009c857104de4eb3472825c9e71b08d1191533644c1c554ec7e51b1464ffbbb317022051d2034757e3dcf7b971f0561aa47dd92d9b5ecc551e31677c756217b359f4e70147304402203e242499c36859c4fe2bf6ef69b50cf4cff335922ccaeab23e7343e1035031d2022035ff2edb175d6f1c37dced1f94b84f851ffe02ba86584c4f2bc8217cc1e30b7f0147304402201a778535852d02839cea6ee9f140adc8b2a5462d685147d66ad8a2ac71bf1fc302200716e7f39f32566ecd75c244ca604462ecfc42e81d13f7d4bb54c0c78c2d4be5014c8b53210362274ce74eab0ddb35a3abfab8d2b37f33767eb1c14dd26444ac83fd32a1e27d21039ab9b9d71406ec504ec7098d715ea63abe3d5b4de4cd88ca1b9961c21c3c65e0210393fd731b61f5316963558f4033b9365543262f00de8885e31edade0c88f70d87210281009788509d4b8c67c0196e6a954a19fe99d3d6cc37be0e98da5513d0ac617e54aeffffffff02000000000000000017a914130da2942c8efd16d05ccd3817b0b0a7165c16b28700000000000000002c6a2a6a28020000000000000062467691cf583d4fa78b18fafaf9801f505e0ef03baf0603fd4b0cd004cd1e7500000000","locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        }
        ]);

    let tx = gen_service_tx_lect(&sandbox,
                                 0,
                                 "0100000001cdf7b60662e819fd226ff2fd1dc2dafad6886e5e5f68bbd5ce6f2c88e35c906900000000fd6701004830450221009c857104de4eb3472825c9e71b08d1191533644c1c554ec7e51b1464ffbbb317022051d2034757e3dcf7b971f0561aa47dd92d9b5ecc551e31677c756217b359f4e70147304402203e242499c36859c4fe2bf6ef69b50cf4cff335922ccaeab23e7343e1035031d2022035ff2edb175d6f1c37dced1f94b84f851ffe02ba86584c4f2bc8217cc1e30b7f0147304402201a778535852d02839cea6ee9f140adc8b2a5462d685147d66ad8a2ac71bf1fc302200716e7f39f32566ecd75c244ca604462ecfc42e81d13f7d4bb54c0c78c2d4be5014c8b53210362274ce74eab0ddb35a3abfab8d2b37f33767eb1c14dd26444ac83fd32a1e27d21039ab9b9d71406ec504ec7098d715ea63abe3d5b4de4cd88ca1b9961c21c3c65e0210393fd731b61f5316963558f4033b9365543262f00de8885e31edade0c88f70d87210281009788509d4b8c67c0196e6a954a19fe99d3d6cc37be0e98da5513d0ac617e54aeffffffff02000000000000000017a914130da2942c8efd16d05ccd3817b0b0a7165c16b28700000000000000002c6a2a6a28020000000000000062467691cf583d4fa78b18fafaf9801f505e0ef03baf0603fd4b0cd004cd1e7500000000");
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[tx.raw().clone()]);
}

#[test]
fn test_anchoring_second_block_normal() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox();
    let sandbox_state = SandboxState::new();

    anchor_update_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 999999, ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"]],
            response: [
                {
                    "txid": "fea0a60f7146e7facf5bb382b80dafb762175bf0d4b6ac4e59c09cd4214d1491",
                    "vout": 0,
                    "address": "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 1,
                    "spendable": false,
                    "solvable": false
                }
            ]
        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    let tx = TransactionBuilder::with_prev_tx(&anchoring_state.latest_anchoring_tx(), 0)
        .payload(10, sandbox.last_hash())
        .send_to("2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu", 2000)
        .into_transaction();
    let signatures = anchoring_state.gen_anchoring_signatures(&sandbox, &tx);

    sandbox.broadcast(signatures[0].clone());

    client.expect(vec![
        request! {
            method: "getrawtransaction",
            params: ["5e89a4b281b9734ae318c4014ca1c7e92212452dc50d79bfd042ac4ee1ef4feb", 1],
            response: {
                "hash":"5e89a4b281b9734ae318c4014ca1c7e92212452dc50d79bfd042ac4ee1ef4feb","hex":"010000000120989cc8d83d75b870dbea4a8565e571996b4ec0a197bb7c6d1dc6f7e09f3fed00000000fd680100483045022100aeef1d5d99aece5adbadc9a06c736e1f70686996fe9316f3d7afaf64a7a8dbeb022036b39b812b4b091f722db83224c851263ee9fb8c80f82376700c5e4e0199b15c014730440220368dbca1415e666d9da7ee40b993cbe878cdb2a7c39be37e71b96c863e18b56102207ed4eabf78580d974a6767b1478f91e5741030b04501425fc5ad5714a03b8df2014830450221008c9d8f7b19c2a03e1c1257ee3b9cc2dcb08edd7f4c3c27a6cb68388a129fcc1f0220024b3589e51791074998255cb7c742dae4bfeb858cc2edebbc961e0608956fd7014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02d00700000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280a00000000000000164d236bbdb766e64cec57847e3a0509d4fc77fa9c17b7e61e48f7a3eaa8dbc900000000","locktime":1088682,"size":223,"txid":"4ae2de1782b19ddab252d88d570f60bc821bd745d031029a8b28f7427c8d0e93","version":1,"vin":[{"scriptSig":{"asm":"3044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a[ALL] 02c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876","hex":"473044022075b9f164d9fe44c348c7a18381314c3e6cf22c48e08bacc2ac6e145fd28f73800220448290b7c54ae465a34bb64a1427794428f7d99cc73204a5e501541d07b33e8a012102c5f412387bffcc44dec76b28b948bfd7483ec939858c4a65bace07794e97f876"},"sequence":429496729,"txid":"094d7f6acedd8eb4f836ff483157a97155373974ac0ba3278a60e7a0a5efd645","vout":0}],"vout":[{"n":0,"scriptPubKey":{"addresses":["2NDG2AbxE914amqvimARQF2JJBZ9vHDn3Ga"],"asm":"OP_HASH160 db891024f2aa265e3b1998617e8b18ed3b0495fc OP_EQUAL","hex":"a914db891024f2aa265e3b1998617e8b18ed3b0495fc87","reqSigs":1,"type":"scripthash"},"value":0.00004},{"n":1,"scriptPubKey":{"addresses":["mn1jSMdewrpxTDkg1N6brC7fpTNV9X2Cmq"],"asm":"OP_DUP OP_HASH160 474215d1e614a7d9dddbd853d9f139cff2e99e1a OP_EQUALVERIFY OP_CHECKSIG","hex":"76a914474215d1e614a7d9dddbd853d9f139cff2e99e1a88ac","reqSigs":1,"type":"pubkeyhash"},"value":1.00768693}],"vsize":223
                }
        }
    ]);

    let signatures = signatures.into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures);

    sandbox.broadcast(gen_service_tx_lect(&sandbox, 0, "010000000120989cc8d83d75b870dbea4a8565e571996b4ec0a197bb7c6d1dc6f7e09f3fed00000000fd680100483045022100aeef1d5d99aece5adbadc9a06c736e1f70686996fe9316f3d7afaf64a7a8dbeb022036b39b812b4b091f722db83224c851263ee9fb8c80f82376700c5e4e0199b15c014730440220368dbca1415e666d9da7ee40b993cbe878cdb2a7c39be37e71b96c863e18b56102207ed4eabf78580d974a6767b1478f91e5741030b04501425fc5ad5714a03b8df2014830450221008c9d8f7b19c2a03e1c1257ee3b9cc2dcb08edd7f4c3c27a6cb68388a129fcc1f0220024b3589e51791074998255cb7c742dae4bfeb858cc2edebbc961e0608956fd7014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02d00700000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280a00000000000000164d236bbdb766e64cec57847e3a0509d4fc77fa9c17b7e61e48f7a3eaa8dbc900000000"));
}

#[test]
fn test_anchoring_second_block_additional_funds() {
    let _ = ::blockchain_explorer::helpers::init_logger();

    let (sandbox, client, mut anchoring_state) = anchoring_sandbox();
    let sandbox_state = SandboxState::new();

    anchor_update_lect_normal(&sandbox, &client, &sandbox_state, &mut anchoring_state);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    client.expect(vec![request! {
            method: "listunspent",
            params: [0, 999999, ["2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu"]],
            response: [
                {
                    "txid": "fea0a60f7146e7facf5bb382b80dafb762175bf0d4b6ac4e59c09cd4214d1491",
                    "vout": 0,
                    "address": "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 1,
                    "spendable": false,
                    "solvable": false
                },
                {
                    "txid": "a03b10b17fc8b86dd0b1b6ebcc3bc3c6dd4b7173302ef68628f5ed768dbd7049",
                    "vout": 0,
                    "address": "2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu",
                    "account": "multisig",
                    "scriptPubKey": "a914499d997314d6e55e49293b50d8dfb78bb9c958ab87",
                    "amount": 0.00010000,
                    "confirmations": 75,
                    "spendable": false,
                    "solvable": false
                }
            ]
        }]);
    add_one_height_with_transactions(&sandbox, &sandbox_state, &[]);

    let tx = TransactionBuilder::with_prev_tx(&anchoring_state.latest_anchoring_tx(), 0)
        .payload(10, sandbox.last_hash())
        .add_funds(&anchoring_state.genesis.funding_tx, 0)
        .send_to("2NAkCcmVunAzQvKFgyQDbCApuKd9xwN6SRu", 6000)
        .into_transaction();
    let signatures = anchoring_state.gen_anchoring_signatures(&sandbox, &tx);

    sandbox.broadcast(signatures[0].clone());
    sandbox.broadcast(signatures[1].clone());

    client.expect(vec![
        Request {
            method: "getrawtransaction",
            params: vec!["d92ba7ae353d5271f1276efd39b0e73171eda171ee15b648f92b5591ee39be7e".to_json(), 1.to_json()],
            response: Err(RpcError::NoInformation("Unable to find tx".to_string()))
        },
        request! {
            method: "sendrawtransaction",
            params: ["010000000220989cc8d83d75b870dbea4a8565e571996b4ec0a197bb7c6d1dc6f7e09f3fed00000000fd680100483045022100d33ed28280fcb16349b55a5ba65618c1d31b82da7190189441d49f556db7d0d3022018a26b0ae8abcd368ad16c91b3e899a207c9e8f0226baadd700a651e38fb1ca801483045022100ddd62f2ce4aa6ed6acd96c3535e240fa7e64dd001d151f9c8ed1b5811cada94b02203bddd2b2da99f84ec99771f4b5189ce6f74797aea3843df395c11330623e6d3401473044022075a44341e0a23692594db9ed64e9fc5cf5f3d77fbe9c47fbc042c5ab2c9f582f0220280db663cad9f8262295ea4ada24cdd52168b0ced5126b4d9d348fae2f216a39014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff4970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd680100483045022100d135d5a93a6881c109297bb5e3419e58e2d122f05152363e4511823ce978dd8202202753a9fe702cd3733de6363725fc963c360042091c896f181d4241f9d9a44c5701483045022100fe16add05b7020e56c80b0e9b7e40c5bf8e1e3e81526387bf1578e826a7bb138022011377a05320c0c06f9bb881dee7561bab67bc7e064505eeb740d535322b19ea50147304402204d7d153d84e9789822e4a52218feb4749bf118e7a5ba59b1292e8be4737e69f902201ff6500d77fc90d0cc8a7f4cc289f4bc77b2919be114d0890a02f943f8d2e310014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02701700000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280a00000000000000164d236bbdb766e64cec57847e3a0509d4fc77fa9c17b7e61e48f7a3eaa8dbc900000000"]
        }
    ]);

    let signatures = signatures.into_iter()
        .map(|tx| tx.raw().clone())
        .collect::<Vec<_>>();
    add_one_height_with_transactions(&sandbox, &sandbox_state, &signatures);

    sandbox.broadcast(gen_service_tx_lect(&sandbox, 0, "010000000220989cc8d83d75b870dbea4a8565e571996b4ec0a197bb7c6d1dc6f7e09f3fed00000000fd680100483045022100d33ed28280fcb16349b55a5ba65618c1d31b82da7190189441d49f556db7d0d3022018a26b0ae8abcd368ad16c91b3e899a207c9e8f0226baadd700a651e38fb1ca801483045022100ddd62f2ce4aa6ed6acd96c3535e240fa7e64dd001d151f9c8ed1b5811cada94b02203bddd2b2da99f84ec99771f4b5189ce6f74797aea3843df395c11330623e6d3401473044022075a44341e0a23692594db9ed64e9fc5cf5f3d77fbe9c47fbc042c5ab2c9f582f0220280db663cad9f8262295ea4ada24cdd52168b0ced5126b4d9d348fae2f216a39014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff4970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd680100483045022100d135d5a93a6881c109297bb5e3419e58e2d122f05152363e4511823ce978dd8202202753a9fe702cd3733de6363725fc963c360042091c896f181d4241f9d9a44c5701483045022100fe16add05b7020e56c80b0e9b7e40c5bf8e1e3e81526387bf1578e826a7bb138022011377a05320c0c06f9bb881dee7561bab67bc7e064505eeb740d535322b19ea50147304402204d7d153d84e9789822e4a52218feb4749bf118e7a5ba59b1292e8be4737e69f902201ff6500d77fc90d0cc8a7f4cc289f4bc77b2919be114d0890a02f943f8d2e310014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02701700000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280a00000000000000164d236bbdb766e64cec57847e3a0509d4fc77fa9c17b7e61e48f7a3eaa8dbc900000000"));
}