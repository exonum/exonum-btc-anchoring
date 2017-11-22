use std::ops::Deref;

use bitcoin::blockdata::script::Script;
use bitcoin::blockdata::transaction::SigHashType;
use bitcoin::network::constants::Network;

use exonum::messages::Message;
use exonum::helpers::{Height, ValidatorId};
use exonum::encoding::serialize::HexValue;
use exonum::crypto::Hash;

use blockchain::dto::{MsgAnchoringSignature, MsgAnchoringUpdateLatest};
use details::btc::transactions::{verify_tx_input, AnchoringTx, FundingTx, TransactionBuilder};
use super::AnchoringTestKit;
use super::helpers::*;

// We anchor first block
// problems: None
// result: success
#[test]
fn test_anchoring_first_block_simple() {
    let mut testkit = AnchoringTestKit::default();
    anchor_first_block(&mut testkit);
}

// We wait until `funding_tx` have got enough confirmations.
// problems: None
// result: success
#[test]
fn test_anchoring_funding_tx_waiting() {
    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();
    let funding_tx = testkit.current_funding_tx();

    requests.expect(vec![confirmations_request(&funding_tx, 0)]);
    testkit.create_block();
    // Resend funding_tx if we lost it
    requests.expect(resend_raw_transaction_requests(&funding_tx));
    testkit.create_block();

    requests.expect(vec![confirmations_request(&funding_tx, 0)]);
    testkit.create_block();
}


// We anchor first block and receive lect
// problems: None
// result: success
#[test]
fn test_anchoring_update_lect_normal() {
    let mut testkit = AnchoringTestKit::default();
    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);
}

// We anchor first block and receive lect with different but correct signatures
// problems: lect with a different signature set
// result: success with a new lect
#[test]
fn test_anchoring_update_lect_different() {
    let mut testkit = AnchoringTestKit::default();
    anchor_first_block_lect_different(&mut testkit);
}

// We anchor first block and lose anchoring transaction
// problems: anchoring transaction is lost
// result: we have lost anchoring transaction
#[test]
fn test_anchoring_first_block_lect_lost() {
    let mut testkit = AnchoringTestKit::default();
    anchor_first_block_lect_lost(&mut testkit);
}

// We anchor second block after successfuly anchored first
// problems: none
// result: success
#[test]
fn test_anchoring_second_block_normal() {
    let mut testkit = AnchoringTestKit::default();
    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);
    anchor_second_block_normal(&mut testkit);
}

// We anchor second block after successfuly anchored first with additional funds
// problems: none
// result: success
#[test]
fn test_anchoring_second_block_additional_funds() {
    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();
    let anchoring_addr = testkit.current_addr();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let height = testkit.next_anchoring_height();
    testkit.create_blocks_until(height);

    let funds = testkit.current_funding_tx();
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_string()]],
            response: [
                listunspent_entry(&mut testkit.latest_anchored_tx(), &anchoring_addr, 1),
                listunspent_entry(&funds, &anchoring_addr, 75)
            ]
        },
        get_transaction_request(
            &mut testkit.latest_anchored_tx()
        ),
        get_transaction_request(&funds),
    ]);
    testkit.create_block();

    let block_hash = testkit.block_hash_on_height(Height(10));
    let (_, signatures) = testkit.gen_anchoring_tx_with_signatures(
        Height(10),
        block_hash,
        &[funds],
        None,
        &anchoring_addr,
    );
    let signatures = signatures.into_iter().map(to_box).collect::<Vec<_>>();

    testkit.mempool().contains_key(&signatures[0].hash());
    testkit.mempool().contains_key(&signatures[1].hash());

    let anchored_tx = &mut testkit.latest_anchored_tx();
    requests.expect(send_raw_transaction_requests(anchored_tx));

    testkit.create_block_with_transactions(signatures);
    let lect = gen_service_tx_lect(&mut testkit, ValidatorId(0), anchored_tx, 2);
    testkit.mempool().contains_key(&lect.hash());
}

// We anchor second block after successfuly anchored first
// problems: second anchoring tx is lost
// result: we have lost anchoring tx
#[test]
fn test_anchoring_second_block_lect_lost() {
    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();
    let anchoring_addr = testkit.current_addr();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let prev_anchored_tx = testkit.latest_anchored_tx().clone();
    let prev_tx_signatures = testkit.latest_anchored_tx_signatures();

    anchor_second_block_normal(&mut testkit);
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);

    requests.expect(vec![
        request! {
        method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_string()]],
            response: [
                listunspent_entry(&prev_anchored_tx, &anchoring_addr, 0)
            ]
        },
        get_transaction_request(&prev_anchored_tx),
    ]);

    testkit.create_block();

    let txs = (0..4)
        .map(ValidatorId)
        .map(|id| {
            gen_service_tx_lect(&mut testkit, id, &prev_anchored_tx, 3)
        })
        .map(to_box)
        .collect::<Vec<_>>();
    testkit.mempool().contains_key(&txs[0].hash());

    // Trying to resend lost lect tx
    requests.expect(vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_string()]],
            response: [
                listunspent_entry(&prev_anchored_tx, &anchoring_addr, 0)
            ]
        },
        get_transaction_request(&prev_anchored_tx),
    ]);
    testkit.create_block_with_transactions(txs);
    testkit.set_latest_anchored_tx(Some((prev_anchored_tx, prev_tx_signatures)));
}

// We find lect, whose prev_hash is not known
// problems: prev_hash is unknown
// result: we unroll chain to funding_tx up to funding_tx and update lect
#[test]
fn test_anchoring_find_lect_chain_normal() {
    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();
    anchor_first_block(&mut testkit);

    // Just add few heights
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);

    let anchoring_addr = testkit.current_addr();

    let prev_anchored_tx = {
        let mut txs = Vec::new();
        for height in 0..3 {
            let height = Height(height);
            let hash = testkit.block_hash_on_height(height);
            testkit.gen_anchoring_tx_with_signatures(height, hash, &[], None, &anchoring_addr);
            let tx = testkit.latest_anchored_tx().clone();
            let lects = (1..4)
                .map(ValidatorId)
                .map(|id| {
                    let keypair = testkit.validator(ValidatorId(0)).service_keypair();
                    MsgAnchoringUpdateLatest::new(
                        keypair.0,
                        id,
                        tx.clone().into(),
                        lects_count(&testkit, id),
                        keypair.1,
                    )
                })
                .collect::<Vec<_>>();
            force_commit_lects(&mut testkit, lects);
            txs.push(tx);
        }
        // Get n - 1 transaction
        txs.into_iter().rev().nth(1).unwrap()
    };
    let current_anchored_tx = testkit.latest_anchored_tx();
    assert_eq!(current_anchored_tx.prev_hash(), prev_anchored_tx.id());

    let request = vec![
        request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_string()]],
            response: [
                listunspent_entry(&current_anchored_tx, &anchoring_addr, 0)
            ]
        },
        get_transaction_request(&current_anchored_tx),
        get_transaction_request(&prev_anchored_tx),
    ];
    requests.expect(request);
    testkit.create_block();

    let lect = gen_service_tx_lect(&mut testkit, ValidatorId(0), &current_anchored_tx, 2);
    testkit.mempool().contains_key(&lect.hash());
}

// We find lect, whose prev_hash is not known
// problems: prev_hash is unknown, chain has wrong prev_hashes
// result: we unroll chain to funding_tx up to weird tx and discard lect
#[test]
fn test_anchoring_find_lect_chain_wrong() {
    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();
    anchor_first_block(&mut testkit);

    // Just add few heights
    let height = testkit.next_check_lect_height();
    testkit.create_blocks_until(height);

    let anchoring_addr = testkit.current_addr();
    let anchored_txs = {
        let mut tx = AnchoringTx::from_hex(
            "0100000001c13d4c739390c799344fa89fb701add04e5ccaf3d580\
             e4d4379c4b897e3a2266000000006b483045022100ff88211040a8a95a42ca8520749c1b2b4024ce07b3ed\
             1b51da8bb90ef77dbe5d022034b34ef638d23ef0ea532e2c84a8816cb32021112d4bcf1457b4e2c149d1b8\
             3f01210250749a68b12a93c2cca6f86a9a9c9ba37f5191e85334c340856209a17cca349afeffffff024042\
             0f000000000017a914180d8e6b0ad7f63177e943752c278294709425bd872908da0b000000001976a914de\
             e9f9433b3f2d24cbd833f83a41e4c1235efa3f88acd6ac1000",
        ).unwrap();
        let mut txs = vec![tx.clone()];
        for height in 1..4 {
            let height = Height(height);
            let hash = testkit.block_hash_on_height(height);
            tx = TransactionBuilder::with_prev_tx(&tx, 0)
                .fee(100)
                .payload(height, hash)
                .send_to(anchoring_addr.clone())
                .into_transaction()
                .unwrap();
            txs.push(tx.clone());
        }
        testkit.set_latest_anchored_tx(Some((tx, vec![])));
        txs.into_iter().take(2).collect::<Vec<_>>()
    };
    let current_anchored_tx = anchored_txs.last().unwrap();

    let request = {
        let mut request = Vec::new();

        request.push(request! {
            method: "listunspent",
            params: [0, 9_999_999, [&anchoring_addr.to_string()]],
            response: [
                listunspent_entry(current_anchored_tx, &anchoring_addr, 0)
            ]
        });
        for tx in anchored_txs.iter().rev() {
            request.push(get_transaction_request(tx));
        }
        request
    };
    requests.expect(request);
    testkit.create_block();
}

// We received lect message with correct content
// problems: None
// result: we appect it
#[test]
fn test_anchoring_lect_correct_validator() {
    let mut testkit = AnchoringTestKit::default();
    anchor_first_block(&mut testkit);

    let msg_lect = {
        let latest_anchored_tx = testkit.latest_anchored_tx();
        gen_service_tx_lect_wrong(
            &mut testkit,
            ValidatorId(0),
            ValidatorId(0),
            &latest_anchored_tx,
            2,
        )
    };
    // Commit `msg_lect` into blockchain
    testkit.create_block_with_transactions(txvec![msg_lect]);
    // Ensure that service accepts it
    let lects_after = dump_lects(&testkit, ValidatorId(0));
    assert_eq!(
        lects_after.last().unwrap(),
        testkit.latest_anchored_tx().deref()
    );
}

// We received lect message with different validator id
// problems: None
// result: we ignore it
#[test]
fn test_anchoring_lect_wrong_validator() {
    let mut testkit = AnchoringTestKit::default();
    anchor_first_block(&mut testkit);

    let msg_lect_wrong = {
        let latest_anchored_tx = testkit.latest_anchored_tx();
        gen_service_tx_lect_wrong(
            &mut testkit,
            ValidatorId(2),
            ValidatorId(0),
            &latest_anchored_tx,
            2,
        )
    };

    let lects_before = dump_lects(&mut testkit, ValidatorId(0));
    // Commit `msg_lect_wrong` into blockchain
    testkit.create_block_with_transactions(txvec![msg_lect_wrong]);
    // Ensure that service ignore it
    let lects_after = dump_lects(&mut testkit, ValidatorId(0));
    assert_eq!(lects_after, lects_before);
}

// We received lect message with nonexistent validator id
// problems: None
// result: we ignore it
#[test]
fn test_anchoring_lect_nonexistent_validator() {
    let mut testkit = AnchoringTestKit::default();
    anchor_first_block(&mut testkit);

    let msg_lect_wrong = {
        let latest_anchored_tx = testkit.latest_anchored_tx();
        gen_service_tx_lect_wrong(
            &mut testkit,
            ValidatorId(2),
            ValidatorId(1000),
            &latest_anchored_tx,
            2,
        )
    };

    let lects_before = dump_lects(&mut testkit, ValidatorId(2));
    // Commit `msg_lect_wrong` into blockchain
    testkit.create_block_with_transactions(txvec![msg_lect_wrong]);
    // Ensure that service ignore it
    let lects_after = dump_lects(&mut testkit, ValidatorId(2));
    assert_eq!(lects_after, lects_before);
}

// We received signature message with wrong sign
// problems: None
// result: we ignore it
#[test]
fn test_anchoring_signature_wrong_validator() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let signatures = testkit.latest_anchored_tx_signatures();
    let tx = {
        let mut tx = testkit.latest_anchored_tx().clone();
        tx.0.input[0].script_sig = Script::new();
        tx
    };

    let validator_1 = ValidatorId(1);
    let msg_signature_wrong = {
        let keypair = testkit.validator(validator_1).service_keypair();
        MsgAnchoringSignature::new(
            keypair.0,
            validator_1,
            tx.clone(),
            0,
            signatures[0].signature(),
            keypair.1,
        )
    };

    let signs_before = dump_signatures(&mut testkit, &tx.id());
    // Commit `msg_signature_wrong` into blockchain
    testkit.create_block_with_transactions(txvec![msg_signature_wrong]);
    // Ensure that service ignore it
    let signs_after = dump_signatures(&mut testkit, &tx.id());
    assert_eq!(signs_before, signs_after);
}

// We received correct signature message with nonexistent id
// problems: None
// result: we add signature
#[test]
fn test_anchoring_signature_nonexistent_tx() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let (redeem_script, addr) = testkit.current_cfg().redeem_script();
    let block_hash = testkit.block_hash_on_height(Height::zero());
    let tx = TransactionBuilder::with_prev_tx(&mut testkit.latest_anchored_tx(), 0)
        .fee(100)
        .payload(Height::zero(), block_hash)
        .send_to(addr.clone())
        .into_transaction()
        .unwrap();
    let signature = tx.sign_input(&redeem_script, 0, &mut testkit.priv_keys(&addr)[1]);
    let validator_1 = ValidatorId(1);
    let msg_sign = {
        let keypair = testkit.validator(validator_1).service_keypair();
        MsgAnchoringSignature::new(
            keypair.0,
            validator_1,
            tx.clone(),
            0,
            signature.as_ref(),
            keypair.1,
        )
    };


    let signs_before = dump_signatures(&mut testkit, &tx.id());
    // Commit `msg_sign` into blockchain
    testkit.create_block_with_transactions(txvec![msg_sign.clone()]);
    // Ensure that service adds it
    let signs_after = dump_signatures(&testkit, &tx.id());
    assert!(signs_before.is_empty());
    assert_eq!(signs_after[0], msg_sign);
}

// We received correct signature message with incorrect payload
// problems: None
// result: we ignore it
#[test]
fn test_anchoring_signature_incorrect_payload() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let (redeem_script, addr) = testkit.current_cfg().redeem_script();
    let tx = TransactionBuilder::with_prev_tx(&mut testkit.latest_anchored_tx(), 0)
        .fee(100)
        .payload(Height::zero(), Hash::zero())
        .send_to(addr.clone())
        .into_transaction()
        .unwrap();
    let signature = tx.sign_input(&redeem_script, 0, &mut testkit.priv_keys(&addr)[1]);
    let validator_1 = ValidatorId(1);
    let msg_sign = {
        let keypair = testkit.validator(validator_1).service_keypair();
        MsgAnchoringSignature::new(
            keypair.0,
            validator_1,
            tx.clone(),
            0,
            signature.as_ref(),
            keypair.1,
        )
    };

    let signatures_before = dump_signatures(&testkit, &tx.id());
    // Commit `msg_sign` into blockchain
    testkit.create_block_with_transactions(txvec![msg_sign.clone()]);
    // Ensure that service ignores it
    let signatures_after = dump_signatures(&testkit, &tx.id());
    assert!(signatures_before.is_empty());
    assert!(signatures_after.is_empty());
}

// We received correct lect with the current funding_tx
// problems: None
// result: we add it
#[test]
fn test_anchoring_lect_funding_tx() {
    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let tx = testkit.current_funding_tx();
    let msg_lect = gen_service_tx_lect(&testkit, ValidatorId(0), &tx, 2);
    let lects_before = dump_lects(&testkit, ValidatorId(0));
    // Commit `msg_lect` into blockchain
    requests.expect(vec![confirmations_request(&tx, 50)]);
    testkit.create_block_with_transactions(txvec![msg_lect.clone()]);
    // Ensure that service accepts it
    let lects_after = dump_lects(&testkit, ValidatorId(0));
    assert_eq!(lects_before.len(), 2);
    assert_eq!(lects_after[2], tx.0);
}

// We received correct lect with the incorrect funding_tx
// problems: None
// result: we ignore it
#[test]
fn test_anchoring_lect_incorrect_funding_tx() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    // Create `p2sh` transaction for unknown multisig address
    // For details see link be470954627bfbde664b5adc2bbb98280e8491918cf4a678d16cab13e8a9865b
    // transaction in https://www.blocktrail.com/tBTC/tx
    let tx = FundingTx::from_hex(
        "02000000017ed7e5c5ebec6c7d3d012f543b0656216ccc3710f4272e0b663176\
         598a9271da010000006a47304402206746eab4ce2a720307686b3b255e8fed99dee6da01575d67394c7b7e0953\
         93a102206fbb34a95c139ad217c767bf2bc1db5521af2407ea12d47b3a9e9babfba58df9012102b3aac66108bf\
         a3eee4075c6adee8fd417ca535cfdbe1637be418b7ab92cc5346feffffff02a00f00000000000017a91424f0d3\
         4abfec4f4cb19d942202529b8653a0a58d870c170f0c000000001976a9147bb8844ee71cbd2bc735411f4e2997\
         1f697fed0a88ac81131100",
    ).unwrap();
    let msg_lect = gen_service_tx_lect(&mut testkit, ValidatorId(0), &tx, 2);
    let lects_before = dump_lects(&mut testkit, ValidatorId(0));
    // Commit `msg_lect` into blockchain
    testkit.create_block_with_transactions(txvec![msg_lect.clone()]);
    // Ensure that service ignores it
    let lects_after = dump_lects(&mut testkit, ValidatorId(0));
    assert_eq!(lects_before, lects_after);
}

// We received correct lect with the incorrect anchoring payload
// problems: None
// result: we ignore it
#[test]
fn test_anchoring_lect_incorrect_anchoring_payload() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let tx = TransactionBuilder::with_prev_tx(&mut testkit.current_funding_tx(), 0)
        .fee(1000)
        .payload(Height::zero(), Hash::zero())
        .send_to(testkit.current_addr())
        .into_transaction()
        .unwrap();
    let msg_lect = gen_service_tx_lect(&testkit, ValidatorId(0), &tx, 2);
    let lects_before = dump_lects(&testkit, ValidatorId(0));
    // Commit `msg_lect` into blockchain
    testkit.create_block_with_transactions(txvec![msg_lect.clone()]);
    // Ensure that service ignores it
    let lects_after = dump_lects(&testkit, ValidatorId(0));
    assert_eq!(lects_before, lects_after);
}

// We received correct lect with the unknown prev_hash
// problems: None
// result: we ignore it
#[test]
fn test_anchoring_lect_unknown_prev_tx() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let tx = {
        let prev_tx = TransactionBuilder::with_prev_tx(&mut testkit.current_funding_tx(), 0)
            .fee(100)
            .payload(Height::zero(), Hash::zero())
            .send_to(testkit.current_addr())
            .into_transaction()
            .unwrap();

        TransactionBuilder::with_prev_tx(&prev_tx, 0)
            .fee(100)
            .payload(Height::zero(), testkit.block_hash_on_height(Height::zero()))
            .send_to(testkit.current_addr())
            .into_transaction()
            .unwrap()
    };

    let msg_lect = gen_service_tx_lect(&mut testkit, ValidatorId(0), &tx, 2);
    let lects_before = dump_lects(&mut testkit, ValidatorId(0));
    // Commit `msg_lect` into blockchain
    testkit.create_block_with_transactions(txvec![msg_lect.clone()]);
    // Ensure that service ignores it
    let lects_after = dump_lects(&mut testkit, ValidatorId(0));
    assert_eq!(lects_after, lects_before);
}

// We received signature message with wrong sign
// problems: None
// result: we ignore it
#[test]
fn test_anchoring_signature_nonexistent_validator() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let signatures = testkit.latest_anchored_tx_signatures();
    let tx = {
        let mut tx = testkit.latest_anchored_tx().clone();
        tx.0.input[0].script_sig = Script::new();
        tx
    };

    let validator_1 = ValidatorId(1);
    let msg_signature_wrong = {
        let keypair = testkit.validator(validator_1).service_keypair();
        MsgAnchoringSignature::new(
            keypair.0,
            ValidatorId(1000),
            tx.clone(),
            0,
            signatures[0].signature(),
            keypair.1,
        )
    };

    let signs_before = dump_signatures(&testkit, &tx.id());
    // Commit `msg_signature_wrong` into blockchain
    testkit.create_block_with_transactions(txvec![msg_signature_wrong]);
    // Ensure that service ignores it
    let signs_after = dump_signatures(&testkit, &tx.id());
    assert_eq!(signs_before, signs_after);
}

// We received signature message with correct input but different signature
// problems: None
// result: we ignore it
#[test]
fn test_anchoring_signature_input_with_different_correct_signature() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let signature_msgs = testkit.latest_anchored_tx_signatures();
    let tx = {
        let mut tx = testkit.latest_anchored_tx().clone();
        tx.0.input[0].script_sig = Script::new();
        tx
    };

    let msg_signature_different = {
        let cfg = testkit.current_cfg();
        let (redeem_script, addr) = cfg.redeem_script();
        let pub_key = &cfg.anchoring_keys[1];
        let priv_key = &mut testkit.priv_keys(&addr)[1];

        let mut different_signature =
            sign_tx_input_with_nonce(&tx, 0, &redeem_script, priv_key.secret_key(), 2);
        assert!(verify_tx_input(
            &tx,
            0,
            &redeem_script,
            pub_key,
            different_signature.as_ref(),
        ));

        different_signature.push(SigHashType::All.as_u32() as u8);
        assert_ne!(different_signature, signature_msgs[1].signature());

        let validator_1 = ValidatorId(1);
        let keypair = testkit.validator(validator_1).service_keypair();
        MsgAnchoringSignature::new(
            keypair.0,
            validator_1,
            tx.clone(),
            0,
            different_signature.as_ref(),
            keypair.1,
        )
    };
    assert_ne!(signature_msgs[1], msg_signature_different);

    let signs_before = dump_signatures(&mut testkit, &tx.id());
    // Commit `msg_signature_different` into blockchain
    testkit.create_block_with_transactions(txvec![msg_signature_different.clone()]);
    // Ensure that service ignores it
    let signs_after = dump_signatures(&mut testkit, &tx.id());
    assert_eq!(signs_before, signs_after);
}

// We received signature message with correct signature
// but signed by different validator
// problems: None
// result: we ignore it
#[test]
fn test_anchoring_signature_input_from_different_validator() {
    let mut testkit = AnchoringTestKit::default();
    let requests = testkit.requests();

    anchor_first_block_without_other_signatures(&mut testkit);

    let signatures = testkit.latest_anchored_tx_signatures();
    let tx = {
        let mut tx = testkit.latest_anchored_tx().clone();
        tx.0.input[0].script_sig = Script::new();
        tx
    };

    let msg_signature_wrong = {
        let validator_1 = ValidatorId(1);
        let keypair = testkit.validator(validator_1).service_keypair();
        MsgAnchoringSignature::new(
            keypair.0,
            ValidatorId(2),
            tx.clone(),
            0,
            signatures[2].signature(),
            keypair.1,
        )
    };

    let signs_before = dump_signatures(&mut testkit, &tx.id());
    // Commit `msg_signature_different` into blockchain
    requests.expect(vec![
        confirmations_request(
            &mut testkit.current_funding_tx(),
            50
        ),
    ]);
    testkit.create_block_with_transactions(txvec![msg_signature_wrong.clone()]);
    // Ensure that service ignores it
    let signs_after = dump_signatures(&mut testkit, &tx.id());
    assert_eq!(signs_before, signs_after);
}

// We received signature message for anchoring tx with unknown output_address
// problems: None
// result: we ignore it
#[test]
fn test_anchoring_signature_unknown_output_address() {
    let mut testkit = AnchoringTestKit::default();

    anchor_first_block(&mut testkit);
    anchor_first_block_lect_normal(&mut testkit);

    let tx = {
        let (_, addr) = {
            let mut anchoring_cfg = testkit.current_cfg().clone();
            anchoring_cfg.anchoring_keys.swap(1, 2);
            anchoring_cfg.redeem_script()
        };

        TransactionBuilder::with_prev_tx(&mut testkit.latest_anchored_tx(), 0)
            .fee(1000)
            .payload(Height::zero(), Hash::zero())
            .send_to(addr)
            .into_transaction()
            .unwrap()
    };
    let (redeem_script, addr) = testkit.current_cfg().redeem_script();
    let priv_key = &mut testkit.current_priv_keys()[0];
    let signature = tx.sign_input(&redeem_script, 0, priv_key);

    assert_ne!(tx.output_address(Network::Testnet), addr);
    assert!(tx.verify_input(
        &redeem_script,
        0,
        &mut testkit.current_cfg().anchoring_keys[0],
        &signature,
    ));

    let msg_signature_wrong = {
        let validator_0 = ValidatorId(0);
        let keypair = testkit.validator(validator_0).service_keypair();
        MsgAnchoringSignature::new(keypair.0, validator_0, tx.clone(), 0, &signature, keypair.1)
    };

    let signs_before = dump_signatures(&mut testkit, &tx.id());
    // Commit `msg_signature_wrong` into blockchain
    testkit.create_block_with_transactions(txvec![msg_signature_wrong]);
    // Ensure that service ignores it
    let signs_after = dump_signatures(&mut testkit, &tx.id());
    assert_eq!(signs_before, signs_after);
}
