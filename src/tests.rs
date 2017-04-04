extern crate blockchain_explorer;
extern crate rand;

use std::env;

use std::collections::HashMap;

use rand::Rng;
use bitcoin::network::constants::Network;
use bitcoin::util::base58::{FromBase58, ToBase58};
use bitcoin::util::address::Privkey as RawPrivateKey;
use bitcoin::blockdata::transaction::SigHashType;
use secp256k1::key::PublicKey as RawPublicKey;
use secp256k1::Secp256k1;

use exonum::crypto::{Hash, hash, HexValue};
use exonum::storage::StorageValue;

use client::AnchoringRpc;
use transactions::{AnchoringTx, FundingTx, sign_tx_input, verify_tx_input};
use service::config::AnchoringRpcConfig;
use transactions::{TransactionBuilder, BitcoinTx, TxKind};
use btc;
use btc::HexValueEx;

fn anchoring_client() -> AnchoringRpc {
    let rpc = AnchoringRpcConfig {
        host: env::var("ANCHORING_RELAY_HOST")
            .expect("Env variable ANCHORING_RELAY_HOST needs to be setted")
            .parse()
            .unwrap(),
        username: env::var("ANCHORING_USER").ok(),
        password: env::var("ANCHORING_PASSWORD").ok(),
    };
    AnchoringRpc::new(rpc)
}

fn dummy_anchoring_tx(redeem_script: &btc::RedeemScript) -> AnchoringTx {
    let addr = btc::Address::from_script(redeem_script, Network::Testnet);
    let input_tx = AnchoringTx::from_hex("01000000019aaf09d7e73a5f9ab394f1358bfb3dbde7b15b983d715f5c98f369a3f0a288a70000000000ffffffff02b80b00000000000017a914f18eb74087f751109cc9052befd4177a52c9a30a8700000000000000002c6a2a012800000000000000007fab6f66a0f7a747c820cd01fa30d7bdebd26b91c6e03f742abac0b3108134d900000000").unwrap();
    TransactionBuilder::with_prev_tx(&input_tx, 0)
        .fee(1000)
        .payload(0, Hash::zero())
        .send_to(addr)
        .into_transaction()
        .unwrap()
}

fn gen_anchoring_keys(count: usize) -> (Vec<btc::PublicKey>, Vec<btc::PrivateKey>) {
    let mut validators = Vec::new();
    let mut priv_keys = Vec::new();
    for _ in 0..count {
        let (pub_key, priv_key) = btc::gen_btc_keypair(Network::Testnet);
        validators.push(pub_key);
        priv_keys.push(priv_key);
    }
    (validators, priv_keys)
}

fn make_signatures(redeem_script: &btc::RedeemScript,
                   proposal: &AnchoringTx,
                   inputs: &[u32],
                   priv_keys: &[btc::PrivateKey])
                   -> HashMap<u32, Vec<btc::Signature>> {
    let majority_count = (priv_keys.len() as u8) * 2 / 3 + 1;

    let mut signatures = inputs
        .iter()
        .map(|input| (*input, vec![None; priv_keys.len()]))
        .collect::<Vec<_>>();
    let mut priv_keys = priv_keys.iter().enumerate().collect::<Vec<_>>();
    rand::thread_rng().shuffle(&mut priv_keys);

    for (input_idx, input) in inputs.iter().enumerate() {
        let priv_keys_iter = priv_keys.iter().take(majority_count as usize);
        for &(id, priv_key) in priv_keys_iter {
            let sign = proposal.sign_input(redeem_script, *input, priv_key);
            signatures[input_idx].1[id] = Some(sign);
        }
    }

    signatures
        .iter()
        .map(|signs| {
            let input = signs.0;
            let signs = signs
                .1
                .iter()
                .filter_map(|x| x.clone())
                .take(majority_count as usize)
                .collect::<Vec<_>>();
            (input, signs)
        })
        .collect::<HashMap<_, _>>()
}

fn send_anchoring_tx(client: &AnchoringRpc,
                     redeem_script: &btc::RedeemScript,
                     to: &btc::Address,
                     block_height: u64,
                     block_hash: Hash,
                     priv_keys: &[btc::PrivateKey],
                     anchoring_tx: AnchoringTx,
                     additional_funds: &[FundingTx],
                     fee: u64)
                     -> AnchoringTx {
    let tx = {
        let mut builder = TransactionBuilder::with_prev_tx(&anchoring_tx, 0)
            .fee(fee)
            .payload(block_height, block_hash)
            .send_to(to.clone());
        for funding_tx in additional_funds {
            let out = funding_tx.find_out(to).unwrap();
            builder = builder.add_funds(funding_tx, out);
        }
        builder.into_transaction().unwrap()
    };
    debug!("Proposal anchoring_tx={:#?}, txid={}", tx, tx.txid());

    let inputs = tx.inputs().collect::<Vec<_>>();
    let signatures = make_signatures(redeem_script, &tx, inputs.as_slice(), priv_keys);
    let tx = tx.send(client, redeem_script, signatures).unwrap();
    assert_eq!(tx.payload(), (block_height, block_hash));

    debug!("Sended anchoring_tx={:#?}, txid={}", tx, tx.txid());
    let lect_tx = client
        .unspent_transactions(to)
        .unwrap()
        .first()
        .unwrap()
        .clone();
    assert_eq!(lect_tx.0, tx.0);
    tx
}

#[test]
fn test_anchoring_txid() {
    let tx = AnchoringTx::from_hex("010000000195a4472606ae658f1b9cbebd43f440def00c94341a3515024855a1da8d80932800000000fd3d020047304402204e11d63db849f253095e1e0a400f2f0c01894083e97bfaef644b1407b9fe5c4102207cc99ca986dfd99230e6641564d1f70009c5ec9a37da815c4e024c3ba837c01301483045022100d32536daa6e13989ebc7c908c27a0608517d5d967c8b6069dc047faa01e2a096022030f9c46738d9b701dd944ce3e31af9898b9266460b2de6ff3319f2a8c51f7b430147304402206b8e4491e3b98861ba06cf64e78f425cc553110535310f56f71dcd37de590b7f022051f0fa53cb74a1c73247224180cf026b61b7959d587ab6365dd19a279d14cf45014830450221009fa024c767d8004eef882c6cffe9602f781c60d1a7c629d58576e3de41833a5b02206d3b8dc86d052e112305e1fb32f61de77236f057523e22d58d82cbe37222e8fa01483045022100f1784c5e321fb2753fe725381d6f922d3f0edb94ff2eef52063f9c812489f61802202bec2903af6a5405db484ac73ab844707382f39a0b286a0453f2ed41d217c89e014ccf5521027b3e1c603ead09953bd0a8bd13a7a4830a1446289969220b96515dd1745e06f521026b64f403914e43b7ebe9aa23017eb75eef1bc74469f8b1fa342e622565ab28db2103503745e14331dac53528e666f1abab2c6b6e28767539a2827fe080bb475ec25021030a2ff505279a0e58cc3951ada56bcf323955550d1b993c4cb1b7e94a672b31252102ebb5a22d5ec3c2bc36ab7e104553a89c69684a4dfb3c8ea8fe2cb785c63425872102d9fea63c62d7cafcd4a3d20d77e06cf80cb25f3277ffce27d99c98f439323cee56aeffffffff02000000000000000017a914ab6db56dbd716114594a0d3f072ec447f6d8fc698700000000000000002c6a2a0128020000000000000062467691cf583d4fa78b18fafaf9801f505e0ef03baf0603fd4b0cd004cd1e7500000000").unwrap();

    let txid_hex = "0e4167aeb4769de5ad8d64d1b2342330c2b6aadc0ed9ad0d26ae8eafb18d9c87";
    let txid = btc::TxId::from_hex(txid_hex).unwrap();
    let txid2 = tx.id();

    assert_eq!(txid2.to_hex(), txid_hex);
    assert_eq!(txid2, txid);
}

#[test]
fn test_anchoring_tx_storage_value() {
    let hex = "010000000148f4ae90d8c514a739f17dbbd405442171b09f1044183080b23b6557ce82c0990100000000ffffffff0240899500000000001976a914b85133a96a5cadf6cddcfb1d17c79f42c3bbc9dd88ac00000000000000002e6a2c6a2a6a28020000000000000062467691cf583d4fa78b18fafaf9801f505e0ef03baf0603fd4b0cd004cd1e7500000000";
    let tx = AnchoringTx::from_hex(hex).unwrap();
    let data = tx.clone().serialize();
    let tx2: AnchoringTx = AnchoringTx::deserialize(data);

    assert_eq!(tx2, tx);
}

#[test]
fn test_redeem_script_from_pubkeys() {
    let redeem_script_hex = "5321027db7837e51888e94c094703030d162c682c8dba312210f44ff440fbd5e5c24732102bdd272891c9e4dfc3962b1fdffd5a59732019816f9db4833634dbdaf01a401a52103280883dc31ccaee34218819aaa245480c35a33acd91283586ff6d1284ed681e52103e2bc790a6e32bf5a766919ff55b1f9e9914e13aed84f502c0e4171976e19deb054ae";
    let keys = ["027db7837e51888e94c094703030d162c682c8dba312210f44ff440fbd5e5c2473",
                "02bdd272891c9e4dfc3962b1fdffd5a59732019816f9db4833634dbdaf01a401a5",
                "03280883dc31ccaee34218819aaa245480c35a33acd91283586ff6d1284ed681e5",
                "03e2bc790a6e32bf5a766919ff55b1f9e9914e13aed84f502c0e4171976e19deb0"]
            .into_iter()
            .map(|x| btc::PublicKey::from_hex(x).unwrap())
            .collect::<Vec<_>>();

    let redeem_script = btc::RedeemScript::from_pubkeys(&keys, 3);
    assert_eq!(redeem_script.to_hex(), redeem_script_hex);
    assert_eq!(redeem_script.to_address(Network::Testnet),
               "2N1mHzwKTmjnC7JjqeGFBRKYE4WDTjTfop1");
    assert_eq!(btc::RedeemScript::from_hex(redeem_script_hex).unwrap(),
               redeem_script);

    let compressed_redeem_script = redeem_script.compressed(Network::Testnet);
    assert_eq!(compressed_redeem_script.to_hex(),
               "5321027db7837e51888e94c094703030d162c682c8dba312210f44ff440fbd5e5c24732102bdd272891c9e4dfc3962b1fdffd5a59732019816f9db4833634dbdaf01a401a52103280883dc31ccaee34218819aaa245480c35a33acd91283586ff6d1284ed681e52103e2bc790a6e32bf5a766919ff55b1f9e9914e13aed84f502c0e4171976e19deb054ae");
    assert_eq!(compressed_redeem_script.compressed(Network::Testnet),
               compressed_redeem_script);
}

#[test]
fn test_sign_raw_transaction() {
    let unsigned_tx = BitcoinTx::from_hex("01000000015d1b8ba33a162d8f6e7c5707fbb557e726c32f30f77f2ba348a48c3c5d71ee0b0000000000ffffffff02b80b00000000000017a914889fc9c82819c7a728974ffa78cc884e3e9e68838700000000000000002c6a2a6a28020000000000000062467691cf583d4fa78b18fafaf9801f505e0ef03baf0603fd4b0cd004cd1e7500000000").unwrap();

    let priv_key = RawPrivateKey::from_base58check("cVC9eJN5peJemWn1byyWcWDevg6xLNXtACjHJWmrR5ynsCu8mkQE")
        .unwrap();
    let pub_key = {
        let context = Secp256k1::new();
        RawPublicKey::from_secret_key(&context, priv_key.secret_key()).unwrap()
    };

    let redeem_script = btc::RedeemScript::from_hex("5321027db7837e51888e94c094703030d162c682c8dba312210f44ff440fbd5e5c24732102bdd272891c9e4dfc3962b1fdffd5a59732019816f9db4833634dbdaf01a401a52103280883dc31ccaee34218819aaa245480c35a33acd91283586ff6d1284ed681e52103e2bc790a6e32bf5a766919ff55b1f9e9914e13aed84f502c0e4171976e19deb054ae").unwrap();
    let mut actual_signature =
        sign_tx_input(&unsigned_tx, 0, &redeem_script, priv_key.secret_key());
    actual_signature.push(SigHashType::All.as_u32() as u8);

    assert_eq!(actual_signature.to_hex(),
               "304502210092f1fd6367677ef63dfddfb69cb3644ab10a7c497e5cd391e1d36284dca6a570022021dc2132349afafb9273600698d806f6d5f55756fcc058fba4e49c066116124e01");
    assert!(verify_tx_input(&unsigned_tx,
                            0,
                            &redeem_script,
                            &pub_key,
                            &actual_signature[0..actual_signature.len() - 1]));
}

#[test]
fn test_redeem_script_pubkey() {
    let redeem_script = btc::RedeemScript::from_hex("55210351d8beec8ef4faef9a299640f2f2c8427b4c5ec655da3bdf9c78bb02debce7052103c39016fa9182f84d367d382b561a3db2154041926e4e461607a903ce2b78dbf72103cba17beba839abbc377f8ff8a908199d544ef821509a45ec3b5684e733e4d73b2102014c953a69d452a8c385d1c68e985d697d04f79bf0ddb11e2852e40b9bb880a4210389cbc7829f40deff4acef55babf7dc486a805ad0f4533e665dee4dd6d38157a32103c60e0aeb3d87b05f49341aa88a347237ab2cff3e91a78d23880080d05f8f08e756ae").unwrap();

    assert_eq!(redeem_script
                   .script_pubkey(btc::Network::Testnet)
                   .to_hex(),
               "a914544fa2db1f36b091bbee603c0bc7675fe34655ff87");
}

#[test]
fn test_anchoring_tx_sign() {
    let _ = blockchain_explorer::helpers::init_logger();

    let priv_keys = ["cVC9eJN5peJemWn1byyWcWDevg6xLNXtACjHJWmrR5ynsCu8mkQE",
                     "cMk66oMazTgquBVaBLHzDi8FMgAaRN3tSf6iZykf9bCh3D3FsLX1",
                     "cT2S5KgUQJ41G6RnakJ2XcofvoxK68L9B44hfFTnH4ddygaxi7rc",
                     "cRUKB8Nrhxwd5Rh6rcX3QK1h7FosYPw5uzEsuPpzLcDNErZCzSaj"]
            .iter()
            .map(|x| btc::PrivateKey::from_base58check(x).unwrap())
            .collect::<Vec<_>>();

    let pub_keys = ["03475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c",
                    "02a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0",
                    "0230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb49",
                    "036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e"]
            .iter()
            .map(|x| btc::PublicKey::from_hex(x).unwrap())
            .collect::<Vec<_>>();
    let redeem_script = btc::RedeemScript::from_pubkeys(pub_keys.iter(), 3)
        .compressed(Network::Testnet);

    let prev_tx = AnchoringTx::from_hex("01000000014970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd670100483045022100e6ef3de83437c8dc33a8099394b7434dfb40c73631fc4b0378bd6fb98d8f42b002205635b265f2bfaa6efc5553a2b9e98c2eabdfad8e8de6cdb5d0d74e37f1e198520147304402203bb845566633b726e41322743677694c42b37a1a9953c5b0b44864d9b9205ca10220651b7012719871c36d0f89538304d3f358da12b02dab2b4d74f2981c8177b69601473044022052ad0d6c56aa6e971708f079073260856481aeee6a48b231bc07f43d6b02c77002203a957608e4fbb42b239dd99db4e243776cc55ed8644af21fa80fd9be77a59a60014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02b80b00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280000000000000000f1cb806d27e367f1cac835c22c8cc24c402a019e2d3ea82f7f841c308d830a9600000000").unwrap();
    let funding_tx = FundingTx::from_hex("01000000019532a4022a22226a6f694c3f21216b2c9f5c1c79007eb7d3be06bc2f1f9e52fb000000006a47304402203661efd05ca422fad958b534dbad2e1c7db42bbd1e73e9b91f43a2f7be2f92040220740cf883273978358f25ca5dd5700cce5e65f4f0a0be2e1a1e19a8f168095400012102ae1b03b0f596be41a247080437a50f4d8e825b170770dcb4e5443a2eb2ecab2afeffffff02a00f00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678716e1ff05000000001976a91402f5d7475a10a9c24cea32575bd8993d3fabbfd388ac089e1000").unwrap();

    let tx = TransactionBuilder::with_prev_tx(&prev_tx, 0)
        .add_funds(&funding_tx, 0)
        .payload(10,
                 Hash::from_hex("164d236bbdb766e64cec57847e3a0509d4fc77fa9c17b7e61e48f7a3eaa8dbc9")
                     .unwrap())
        .fee(1000)
        .send_to(btc::Address::from_script(&redeem_script, Network::Testnet))
        .into_transaction()
        .unwrap();

    let mut signatures = HashMap::new();
    for input in tx.inputs() {
        let mut input_signs = Vec::new();
        for priv_key in &priv_keys {
            let sign = tx.sign_input(&redeem_script, input, priv_key);
            input_signs.push(sign);
        }
        signatures.insert(input, input_signs);
    }

    for (input, signs) in &signatures {
        for (id, signature) in signs.iter().enumerate() {
            assert!(tx.verify_input(&redeem_script, *input, &pub_keys[id], signature.as_ref()));
        }
    }
}

#[test]
fn test_anchoring_tx_output_address() {
    let tx = AnchoringTx::from_hex("01000000014970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd670100483045022100e6ef3de83437c8dc33a8099394b7434dfb40c73631fc4b0378bd6fb98d8f42b002205635b265f2bfaa6efc5553a2b9e98c2eabdfad8e8de6cdb5d0d74e37f1e198520147304402203bb845566633b726e41322743677694c42b37a1a9953c5b0b44864d9b9205ca10220651b7012719871c36d0f89538304d3f358da12b02dab2b4d74f2981c8177b69601473044022052ad0d6c56aa6e971708f079073260856481aeee6a48b231bc07f43d6b02c77002203a957608e4fbb42b239dd99db4e243776cc55ed8644af21fa80fd9be77a59a60014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02b80b00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280000000000000000f1cb806d27e367f1cac835c22c8cc24c402a019e2d3ea82f7f841c308d830a9600000000").unwrap();

    let pub_keys = ["03475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c",
                    "02a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0",
                    "0230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb49",
                    "036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e"]
            .iter()
            .map(|x| btc::PublicKey::from_hex(x).unwrap())
            .collect::<Vec<_>>();
    let redeem_script = btc::RedeemScript::from_pubkeys(&pub_keys, 3).compressed(Network::Testnet);

    assert_eq!(tx.output_address(Network::Testnet).to_base58check(),
               redeem_script.to_address(Network::Testnet));
}

#[test]
fn test_anchoring_tx_prev_chain() {
    let prev_tx = AnchoringTx::from_hex("01000000014970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd670100483045022100e6ef3de83437c8dc33a8099394b7434dfb40c73631fc4b0378bd6fb98d8f42b002205635b265f2bfaa6efc5553a2b9e98c2eabdfad8e8de6cdb5d0d74e37f1e198520147304402203bb845566633b726e41322743677694c42b37a1a9953c5b0b44864d9b9205ca10220651b7012719871c36d0f89538304d3f358da12b02dab2b4d74f2981c8177b69601473044022052ad0d6c56aa6e971708f079073260856481aeee6a48b231bc07f43d6b02c77002203a957608e4fbb42b239dd99db4e243776cc55ed8644af21fa80fd9be77a59a60014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02b80b00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280000000000000000f1cb806d27e367f1cac835c22c8cc24c402a019e2d3ea82f7f841c308d830a9600000000").unwrap();
    let tx = TransactionBuilder::with_prev_tx(&prev_tx, 0)
        .fee(1000)
        .payload(0, Hash::default())
        .prev_tx_chain(Some(prev_tx.id()))
        .send_to(btc::Address::from_base58check("2N1mHzwKTmjnC7JjqeGFBRKYE4WDTjTfop1").unwrap())
        .into_transaction()
        .unwrap();

    assert_eq!(tx.prev_tx_chain(), Some(prev_tx.id()));
}

#[test]
fn test_tx_kind_funding() {
    let tx = BitcoinTx::from_hex("01000000019532a4022a22226a6f694c3f21216b2c9f5c1c79007eb7d3be06bc2f1f9e52fb000000006a47304402203661efd05ca422fad958b534dbad2e1c7db42bbd1e73e9b91f43a2f7be2f92040220740cf883273978358f25ca5dd5700cce5e65f4f0a0be2e1a1e19a8f168095400012102ae1b03b0f596be41a247080437a50f4d8e825b170770dcb4e5443a2eb2ecab2afeffffff02a00f00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678716e1ff05000000001976a91402f5d7475a10a9c24cea32575bd8993d3fabbfd388ac089e1000").unwrap();
    match TxKind::from(tx) {
        TxKind::FundingTx(_) => {}
        _ => panic!("Wrong tx kind!"),
    }
}

#[test]
fn test_tx_kind_anchoring() {
    let tx = BitcoinTx::from_hex("01000000014970bd8d76edf52886f62e3073714bddc6c33bccebb6b1d06db8c87fb1103ba000000000fd670100483045022100e6ef3de83437c8dc33a8099394b7434dfb40c73631fc4b0378bd6fb98d8f42b002205635b265f2bfaa6efc5553a2b9e98c2eabdfad8e8de6cdb5d0d74e37f1e198520147304402203bb845566633b726e41322743677694c42b37a1a9953c5b0b44864d9b9205ca10220651b7012719871c36d0f89538304d3f358da12b02dab2b4d74f2981c8177b69601473044022052ad0d6c56aa6e971708f079073260856481aeee6a48b231bc07f43d6b02c77002203a957608e4fbb42b239dd99db4e243776cc55ed8644af21fa80fd9be77a59a60014c8b532103475ab0e9cfc6015927e662f6f8f088de12287cee1a3237aeb497d1763064690c2102a63948315dda66506faf4fecd54b085c08b13932a210fa5806e3691c69819aa0210230cb2805476bf984d2236b56ff5da548dfe116daf2982608d898d9ecb3dceb4921036e4777c8d19ccaa67334491e777f221d37fd85d5786a4e5214b281cf0133d65e54aeffffffff02b80b00000000000017a914bff50e89fa259d83f78f2e796f57283ca10d6e678700000000000000002c6a2a01280000000000000000f1cb806d27e367f1cac835c22c8cc24c402a019e2d3ea82f7f841c308d830a9600000000").unwrap();
    match TxKind::from(tx) {
        TxKind::Anchoring(_) => {}
        _ => panic!("Wrong tx kind!"),
    }
}

#[test]
fn test_tx_kind_other() {
    let tx = BitcoinTx::from_hex("0100000001cea827387bc0bb1b5e6afa6e6d557123e4432e47bad8c2d94214a9cd1e2e074b010000006a473044022034d463312dd75445ad078b1159a75c0b148388b36686b69da8aecca863e63dc3022071ef86a064bd15f11ec89059072bbd3e3d3bb6c5e9b10712e0e2dc6710520bb00121035e63a48d34250dbbcc58fdc0ab63b901769e71035e19e0eee1a87d433a96723afeffffff0296a6f80b000000001976a914b5d7055cfdacc803e5547b981faa693c5aaa813b88aca0860100000000001976a914f5548cb02bb197f071934a0ea3eeb5878cb59dff88ac03a21000").unwrap();
    match TxKind::from(tx) {
        TxKind::Other(_) => {}
        _ => panic!("Wrong tx kind!"),
    }
}

#[test]
fn test_tx_verify_sighash_type_correct() {
    let (pub_keys, priv_keys) = gen_anchoring_keys(4);
    let redeem_script = btc::RedeemScript::from_pubkeys(&pub_keys, 3).compressed(Network::Testnet);

    let tx = dummy_anchoring_tx(&redeem_script);
    let pub_key = &pub_keys[0];
    let btc_signature = tx.sign_input(&redeem_script, 0, &priv_keys[0]);

    assert_eq!(*btc_signature.last().unwrap(),
               SigHashType::All.as_u32() as u8);
    assert!(tx.verify_input(&redeem_script, 0, &pub_key, &btc_signature));
}

#[test]
fn test_tx_verify_incorrect_signature() {
    let (pub_keys, priv_keys) = gen_anchoring_keys(4);
    let redeem_script = btc::RedeemScript::from_pubkeys(&pub_keys, 3).compressed(Network::Testnet);

    let tx = dummy_anchoring_tx(&redeem_script);
    let pub_key = &pub_keys[0];
    let mut btc_signature = tx.sign_input(&redeem_script, 0, &priv_keys[0]);
    btc_signature[8] = btc_signature[8].wrapping_add(63);

    assert!(!tx.verify_input(&redeem_script, 0, &pub_key, &btc_signature));
}

#[test]
fn test_tx_verify_correct_signature_different() {
    let _ = blockchain_explorer::helpers::init_logger();

    let (pub_keys, priv_keys) = gen_anchoring_keys(4);
    let redeem_script = btc::RedeemScript::from_pubkeys(&pub_keys, 3).compressed(Network::Testnet);

    let tx = dummy_anchoring_tx(&redeem_script);
    let pub_key = &pub_keys[0];

    let btc_signature_1 = tx.sign_input(&redeem_script, 0, &priv_keys[0]);
    let mut btc_signature_2 = btc_signature_1.clone();
    btc_signature_2[1] = btc_signature_2[1].wrapping_add(1);

    debug!("{}", btc_signature_1.to_hex());
    debug!("{}", btc_signature_2.to_hex());

    assert!(btc_signature_1 != btc_signature_2);
    assert!(tx.verify_input(&redeem_script, 0, &pub_key, &btc_signature_1));
    assert!(!tx.verify_input(&redeem_script, 0, &pub_key, &btc_signature_2));
}

#[test]
fn test_tx_verify_sighash_type_wrong() {
    let (pub_keys, priv_keys) = gen_anchoring_keys(4);
    let redeem_script = btc::RedeemScript::from_pubkeys(&pub_keys, 3).compressed(Network::Testnet);

    let tx = dummy_anchoring_tx(&redeem_script);
    let pub_key = &pub_keys[0];
    let mut btc_signature = tx.sign_input(&redeem_script, 0, &priv_keys[0]);
    *btc_signature.last_mut().unwrap() = SigHashType::Single.as_u32() as u8;

    assert!(tx.verify_input(&redeem_script, 0, &pub_key, &btc_signature));
}

// rpc tests. Works through `rpc` by given env variables. See the `anchoring_client` method on top of this file.

#[test]
fn test_rpc_unspent_funding_tx() {
    let _ = blockchain_explorer::helpers::init_logger();

    let client = anchoring_client();

    let (validators, _) = gen_anchoring_keys(4);

    let majority_count = ::majority_count(4);
    let (_, address) = client
        .create_multisig_address(Network::Testnet, majority_count, validators.iter())
        .unwrap();

    let funding_tx = FundingTx::create(&client, &address, 1000).unwrap();
    let info = funding_tx.has_unspent_info(&client, &address).unwrap();
    assert!(info.is_some());
    debug!("{:#?}", info);
}

#[test]
fn test_rpc_anchoring_tx_chain() {
    let _ = blockchain_explorer::helpers::init_logger();

    let client = anchoring_client();

    let (validators, priv_keys) = gen_anchoring_keys(4);
    let majority_count = ::majority_count(4);
    let (redeem_script, addr) = client
        .create_multisig_address(Network::Testnet, majority_count, validators.iter())
        .unwrap();
    debug!("multisig_address={:#?}", redeem_script);

    let fee = 1000;
    let block_height = 2;
    let block_hash = hash(&[1, 3, 5]);

    // Make anchoring txs chain
    let total_funds = 4000;
    let mut utxo_tx = {
        let funding_tx = FundingTx::create(&client, &addr, total_funds).unwrap();
        let out = funding_tx.find_out(&addr).unwrap();
        debug!("funding_tx={:#?}", funding_tx);

        let tx = TransactionBuilder::with_prev_tx(&funding_tx, out)
            .payload(block_height, block_hash)
            .send_to(addr.clone())
            .fee(fee)
            .into_transaction()
            .unwrap();
        debug!("Proposal anchoring_tx={:#?}, txid={}", tx, tx.txid());

        let signatures = make_signatures(&redeem_script, &tx, &[0], &priv_keys);
        let tx = tx.send(&client, &redeem_script, signatures).unwrap();
        debug!("Sended anchoring_tx={:#?}, txid={}", tx, tx.txid());

        assert!(funding_tx
                    .has_unspent_info(&client, &addr)
                    .unwrap()
                    .is_none());
        let lect_tx = client
            .unspent_transactions(&addr)
            .unwrap()
            .first()
            .unwrap()
            .clone();
        assert_eq!(lect_tx.0, tx.0);
        tx
    };

    let utxos = client
        .listunspent(0, 9999999, &[addr.to_base58check().as_ref()])
        .unwrap();
    debug!("utxos={:#?}", utxos);

    // Send anchoring txs
    let mut out_funds = utxo_tx.amount();
    debug!("out_funds={}", out_funds);
    while out_funds >= fee {
        utxo_tx = send_anchoring_tx(&client,
                                    &redeem_script,
                                    &addr,
                                    block_height,
                                    block_hash,
                                    &priv_keys,
                                    utxo_tx,
                                    &[],
                                    fee);
        assert_eq!(utxo_tx.payload(), (block_height, block_hash));
        out_funds -= fee;
    }

    // Try to add funding input
    let funding_tx = FundingTx::create(&client, &addr, fee * 3).unwrap();
    utxo_tx = send_anchoring_tx(&client,
                                &redeem_script,
                                &addr,
                                block_height,
                                block_hash,
                                &priv_keys,
                                utxo_tx,
                                &[funding_tx],
                                fee);

    // Send to next addr
    let (validators2, priv_keys2) = gen_anchoring_keys(6);
    let majority_count2 = ::majority_count(6);
    let (redeem_script2, addr2) = client
        .create_multisig_address(Network::Testnet, majority_count2, validators2.iter())
        .unwrap();

    debug!("new_multisig_address={:#?}", redeem_script2);
    utxo_tx = send_anchoring_tx(&client,
                                &redeem_script,
                                &addr2,
                                block_height,
                                block_hash,
                                &priv_keys,
                                utxo_tx,
                                &[],
                                fee);

    send_anchoring_tx(&client,
                      &redeem_script2,
                      &addr2,
                      block_height,
                      block_hash,
                      &priv_keys2,
                      utxo_tx,
                      &[],
                      fee);
}

#[test]
#[should_panic(expected = "InsufficientFunds")]
fn test_rpc_anchoring_tx_chain_insufficient_funds() {
    let _ = blockchain_explorer::helpers::init_logger();

    let client = anchoring_client();

    let (validators, priv_keys) = gen_anchoring_keys(4);
    let majority_count = ::majority_count(4);
    let (redeem_script, addr) = client
        .create_multisig_address(Network::Testnet, majority_count, validators.iter())
        .unwrap();
    debug!("multisig_address={:#?}", redeem_script);

    let fee = 1000;
    let block_height = 2;
    let block_hash = hash(&[1, 3, 5]);

    // Make anchoring txs chain
    let total_funds = 4000;
    let mut utxo_tx = {
        let funding_tx = FundingTx::create(&client, &addr, total_funds).unwrap();
        let out = funding_tx.find_out(&addr).unwrap();
        debug!("funding_tx={:#?}", funding_tx);

        let tx = TransactionBuilder::with_prev_tx(&funding_tx, out)
            .payload(block_height, block_hash)
            .send_to(addr.clone())
            .fee(fee)
            .into_transaction()
            .unwrap();
        debug!("Proposal anchoring_tx={:#?}, txid={}", tx, tx.txid());

        let signatures = make_signatures(&redeem_script, &tx, &[0], &priv_keys);
        let tx = tx.send(&client, &redeem_script, signatures).unwrap();
        debug!("Sended anchoring_tx={:#?}, txid={}", tx, tx.txid());

        assert!(funding_tx
                    .has_unspent_info(&client, &addr)
                    .unwrap()
                    .is_none());
        let lect_tx = client
            .unspent_transactions(&addr)
            .unwrap()
            .first()
            .unwrap()
            .clone();
        assert_eq!(lect_tx.0, tx.0);
        tx
    };

    let utxos = client
        .listunspent(0, 9999999, &[addr.to_base58check().as_ref()])
        .unwrap();
    debug!("utxos={:#?}", utxos);

    // Send anchoring txs
    let mut out_funds = utxo_tx.amount();
    debug!("out_funds={}", out_funds);
    while out_funds >= fee {
        utxo_tx = send_anchoring_tx(&client,
                                    &redeem_script,
                                    &addr,
                                    block_height,
                                    block_hash,
                                    &priv_keys,
                                    utxo_tx,
                                    &[],
                                    fee);
        assert_eq!(utxo_tx.payload(), (block_height, block_hash));
        out_funds -= fee;
    }

    // Try to send tx without funds
    send_anchoring_tx(&client,
                      &redeem_script,
                      &addr,
                      block_height,
                      block_hash,
                      &priv_keys,
                      utxo_tx,
                      &[],
                      fee);
}