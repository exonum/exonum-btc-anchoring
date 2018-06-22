extern crate bitcoin;
extern crate exonum;
extern crate exonum_btc_anchoring;
extern crate exonum_testkit;
extern crate serde_json;

extern crate btc_transaction_utils;

#[cfg(feature = "rpc_tests")]
mod rpc_tests {
    use exonum::blockchain::TransactionErrorType;
    use exonum::crypto::Hash;
    use exonum::helpers::Height;
    use exonum_btc_anchoring::{blockchain::transactions::ErrorKind, config::GlobalConfig,
                               rpc::BtcRelay, test_data::AnchoringTestKit,
                               BTC_ANCHORING_SERVICE_NAME};

    fn check_tx_error(tk: &AnchoringTestKit, tx_hash: Hash, e: ErrorKind) {
        let explorer = tk.explorer();
        let tx_info = explorer.transaction(&tx_hash).unwrap();
        let tx_status = tx_info.as_committed().unwrap().status();

        assert!(tx_status.is_err());

        match tx_status.err().unwrap().error_type() {
            TransactionErrorType::Code(x) => assert_eq!(x, e as u8),
            _ => panic!("should be error code"),
        }
    }

    #[test]
    fn simple() {
        let validators_num = 4;
        let mut anchoring_testkit = AnchoringTestKit::new_with_testnet(validators_num, 70000, 4);

        assert!(anchoring_testkit.last_anchoring_tx().is_none());

        let signatures = anchoring_testkit.create_signature_tx_for_validators(2);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(4));

        let tx0 = anchoring_testkit.last_anchoring_tx();
        assert!(tx0.is_some());
        let tx0 = tx0.unwrap();
        let tx0_meta = tx0.anchoring_metadata().unwrap();
        assert!(tx0_meta.1.block_height == Height(0));

        let signatures = anchoring_testkit.create_signature_tx_for_validators(2);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(8));

        let tx1 = anchoring_testkit.last_anchoring_tx();
        assert!(tx1.is_some());

        let tx1 = tx1.unwrap();
        let tx1_meta = tx1.anchoring_metadata().unwrap();

        assert!(tx0.id() == tx1.prev_tx_id());

        // script_pubkey should be the same
        assert!(tx0_meta.0 == tx1_meta.0);
        assert!(tx1_meta.1.block_height == Height(4));
    }

    #[test]
    fn additional_funding() {
        let validators_num = 4;
        let initial_sum = 50000;
        let mut anchoring_testkit =
            AnchoringTestKit::new_with_testnet(validators_num, initial_sum, 4);

        let signatures = anchoring_testkit.create_signature_tx_for_validators(2);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(4));

        let tx0 = anchoring_testkit.last_anchoring_tx();
        assert!(tx0.is_some());
        let tx0 = tx0.unwrap();
        assert!(tx0.0.input.len() == 1);

        let output_val0 = tx0.0.output.iter().map(|x| x.value).max().unwrap();
        assert!(output_val0 < initial_sum);

        //creating new funding tx
        let rpc_client = anchoring_testkit.rpc_client();
        let address = anchoring_testkit.anchoring_address();

        let new_funding_tx = rpc_client.send_to_address(&address, initial_sum).unwrap();
        let mut configuration_change_proposal = anchoring_testkit.configuration_change_proposal();
        let service_configuration = GlobalConfig {
            funding_transaction: Some(new_funding_tx),
            ..configuration_change_proposal.service_config(BTC_ANCHORING_SERVICE_NAME)
        };

        configuration_change_proposal
            .set_service_config(BTC_ANCHORING_SERVICE_NAME, service_configuration);
        configuration_change_proposal.set_actual_from(Height(6));
        anchoring_testkit.commit_configuration_change(configuration_change_proposal);
        anchoring_testkit.create_blocks_until(Height(6));

        let signatures = anchoring_testkit.create_signature_tx_for_validators(2);
        anchoring_testkit.create_block_with_transactions(signatures);

        let tx1 = anchoring_testkit.last_anchoring_tx().unwrap();
        let tx1_meta = tx1.anchoring_metadata().unwrap();
        assert!(tx1_meta.1.block_height == Height(4));

        assert!(tx1.0.input.len() == 2);

        let output_val1 = tx1.0.output.iter().map(|x| x.value).max().unwrap();
        assert!(output_val1 > output_val0);
        assert!(output_val1 > initial_sum);
    }

    #[test]
    fn address_changed() {
        let validators_num = 5;
        let mut anchoring_testkit = AnchoringTestKit::new_with_testnet(validators_num, 150000, 4);
        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(4));

        let tx0 = anchoring_testkit.last_anchoring_tx();
        assert!(tx0.is_some());
        let tx0 = tx0.unwrap();
        let tx0_meta = tx0.anchoring_metadata().unwrap();

        let signatures = anchoring_testkit.create_signature_tx_for_validators(4);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(6));

        // removing one of validators
        let mut configuration_change_proposal = anchoring_testkit.configuration_change_proposal();
        let mut validators = configuration_change_proposal.validators().to_vec();

        let _ = validators.pop().unwrap();
        configuration_change_proposal.set_validators(validators);

        let config: GlobalConfig =
            configuration_change_proposal.service_config(BTC_ANCHORING_SERVICE_NAME);

        let mut keys = config.public_keys.clone();
        let _ = keys.pop().unwrap();

        let service_configuration = GlobalConfig {
            public_keys: keys,
            ..config
        };
        configuration_change_proposal
            .set_service_config(BTC_ANCHORING_SERVICE_NAME, service_configuration);
        configuration_change_proposal.set_actual_from(Height(16));
        anchoring_testkit.commit_configuration_change(configuration_change_proposal);
        anchoring_testkit.create_blocks_until(Height(7));

        anchoring_testkit.renew_address();
        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(10));

        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(12));

        let tx_transition = anchoring_testkit.last_anchoring_tx().unwrap();

        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(16));

        let tx_same = anchoring_testkit.last_anchoring_tx().unwrap();
        // anchoring is paused till new config
        assert!(tx_transition == tx_same);

        anchoring_testkit.create_blocks_until(Height(17));
        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);

        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(20));

        let tx_changed = anchoring_testkit.last_anchoring_tx().unwrap();
        let tx_changed_meta = tx_changed.anchoring_metadata().unwrap();

        assert!(tx_transition != tx_changed);
        // script_pubkey should *not* be the same
        assert!(tx0_meta.0 != tx_changed_meta.0);
    }

    #[test]
    fn address_changed_and_new_funding_tx() {
        let validators_num = 5;
        let initial_sum = 150000;
        let mut anchoring_testkit =
            AnchoringTestKit::new_with_testnet(validators_num, initial_sum, 4);
        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(4));

        let tx0 = anchoring_testkit.last_anchoring_tx();
        assert!(tx0.is_some());
        let tx0 = tx0.unwrap();
        let tx0_meta = tx0.anchoring_metadata().unwrap();
        let output_val0 = tx0.0.output.iter().map(|x| x.value).max().unwrap();

        // removing one of validators
        let mut configuration_change_proposal = anchoring_testkit.configuration_change_proposal();
        let mut validators = configuration_change_proposal.validators().to_vec();

        let _ = validators.pop().unwrap();
        configuration_change_proposal.set_validators(validators);

        let config: GlobalConfig =
            configuration_change_proposal.service_config(BTC_ANCHORING_SERVICE_NAME);

        let mut keys = config.public_keys.clone();
        let _ = keys.pop().unwrap();

        let mut service_configuration = GlobalConfig {
            public_keys: keys,
            ..config
        };

        // additional funding
        let rpc_client = anchoring_testkit.rpc_client();
        let new_address = service_configuration.anchoring_address();

        let new_funding_tx = rpc_client
            .send_to_address(&new_address, initial_sum)
            .unwrap();

        service_configuration.funding_transaction = Some(new_funding_tx);

        configuration_change_proposal
            .set_service_config(BTC_ANCHORING_SERVICE_NAME, service_configuration);
        configuration_change_proposal.set_actual_from(Height(16));
        anchoring_testkit.commit_configuration_change(configuration_change_proposal);

        anchoring_testkit.create_blocks_until(Height(7));

        anchoring_testkit.renew_address();

        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(10));

        let tx_transition = anchoring_testkit.last_anchoring_tx().unwrap();

        //new funding transaction should not be consumed during creation of transition tx
        assert!(tx_transition.0.input.len() == 1);

        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(16));

        anchoring_testkit.create_blocks_until(Height(17));
        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);

        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(20));

        let tx_changed = anchoring_testkit.last_anchoring_tx().unwrap();
        let tx_changed_meta = tx_changed.anchoring_metadata().unwrap();
        let output_changed = tx_changed.0.output.iter().map(|x| x.value).max().unwrap();

        assert!(tx_transition != tx_changed);
        assert!(tx_changed.0.input.len() == 2);
        assert!(tx_changed.0.input.len() == 2);

        // script_pubkey should *not* be the same
        assert!(tx0_meta.0 != tx_changed_meta.0);

        assert!(output_changed > output_val0);
        assert!(output_changed > initial_sum);
    }

    #[test]
    #[should_panic(expected = "UnsuitableOutput")]
    fn insufficient_funds_during_address_change() {
        let validators_num = 5;
        // single tx fee is ~ 15000
        let initial_sum = 20000;
        let mut anchoring_testkit =
            AnchoringTestKit::new_with_testnet(validators_num, initial_sum, 4);
        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(4));

        let tx0 = anchoring_testkit.last_anchoring_tx();
        assert!(tx0.is_some());
        // removing one of validators
        let mut configuration_change_proposal = anchoring_testkit.configuration_change_proposal();
        let mut validators = configuration_change_proposal.validators().to_vec();

        let _ = validators.pop().unwrap();
        configuration_change_proposal.set_validators(validators);

        let config: GlobalConfig =
            configuration_change_proposal.service_config(BTC_ANCHORING_SERVICE_NAME);

        let mut keys = config.public_keys.clone();
        let _ = keys.pop().unwrap();

        let service_configuration = GlobalConfig {
            public_keys: keys,
            ..config
        };

        configuration_change_proposal
            .set_service_config(BTC_ANCHORING_SERVICE_NAME, service_configuration);

        configuration_change_proposal.set_actual_from(Height(16));
        anchoring_testkit.commit_configuration_change(configuration_change_proposal);
        anchoring_testkit.create_blocks_until(Height(7));

        anchoring_testkit.renew_address();
        anchoring_testkit.create_blocks_until(Height(20));

        let tx1 = anchoring_testkit.last_anchoring_tx();

        //no new transactions
        assert!(tx0 == tx1);

        // it should fail
        let _ = anchoring_testkit.create_signature_tx_for_validators(1);
    }

    #[test]
    fn signature_while_paused_in_transition() {
        let validators_num = 5;
        let initial_sum = 80000;
        let mut anchoring_testkit =
            AnchoringTestKit::new_with_testnet(validators_num, initial_sum, 4);

        let mut signatures = anchoring_testkit.create_signature_tx_for_validators(4);
        let leftover_signature = signatures.remove(0);

        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(4));

        let tx0 = anchoring_testkit.last_anchoring_tx();
        assert!(tx0.is_some());

        // removing one of validators
        let mut configuration_change_proposal = anchoring_testkit.configuration_change_proposal();
        let mut validators = configuration_change_proposal.validators().to_vec();

        let _ = validators.pop().unwrap();
        configuration_change_proposal.set_validators(validators);

        let config: GlobalConfig =
            configuration_change_proposal.service_config(BTC_ANCHORING_SERVICE_NAME);

        let mut keys = config.public_keys.clone();
        let _ = keys.pop().unwrap();

        let service_configuration = GlobalConfig {
            public_keys: keys,
            ..config
        };

        configuration_change_proposal
            .set_service_config(BTC_ANCHORING_SERVICE_NAME, service_configuration);

        configuration_change_proposal.set_actual_from(Height(16));

        anchoring_testkit.commit_configuration_change(configuration_change_proposal);
        anchoring_testkit.create_blocks_until(Height(7));
        anchoring_testkit.renew_address();

        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(12));

        let leftover_hash = leftover_signature.hash();
        anchoring_testkit.create_block_with_transactions(vec![leftover_signature]);

        check_tx_error(
            &anchoring_testkit,
            leftover_hash,
            ErrorKind::UnexpectedSignatureInTransitionState,
        );
    }

    #[test]
    fn wrong_singature_tx() {
        let validators_num = 4;
        let mut anchoring_testkit = AnchoringTestKit::new_with_testnet(validators_num, 70000, 4);

        assert!(anchoring_testkit.last_anchoring_tx().is_none());

        let mut signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        let leftover_signature = signatures.pop().unwrap();

        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(4));

        let tx0 = anchoring_testkit.last_anchoring_tx();
        assert!(tx0.is_some());
        let tx0 = tx0.unwrap();
        let tx0_meta = tx0.anchoring_metadata().unwrap();
        assert!(tx0_meta.1.block_height == Height(0));

        let signatures = anchoring_testkit.create_signature_tx_for_validators(2);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(8));

        // very slow node
        let leftover_hash = leftover_signature.hash();
        anchoring_testkit.create_block_with_transactions(vec![leftover_signature]);

        check_tx_error(
            &anchoring_testkit,
            leftover_hash,
            ErrorKind::UnexpectedSignature,
        );
    }

    #[test]
    fn broken_anchoring_recovery() {
        let validators_num = 5;
        // single tx fee is ~ 15000
        let initial_sum = 20000;
        let mut anchoring_testkit =
            AnchoringTestKit::new_with_testnet(validators_num, initial_sum, 4);
        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(4));

        let tx0 = anchoring_testkit.last_anchoring_tx();
        assert!(tx0.is_some());

        // removing one of validators
        let mut configuration_change_proposal = anchoring_testkit.configuration_change_proposal();
        let mut validators = configuration_change_proposal.validators().to_vec();

        let _ = validators.pop().unwrap();
        configuration_change_proposal.set_validators(validators);

        let config: GlobalConfig =
            configuration_change_proposal.service_config(BTC_ANCHORING_SERVICE_NAME);

        let mut keys = config.public_keys.clone();
        let _ = keys.pop().unwrap();

        let service_configuration = GlobalConfig {
            public_keys: keys,
            ..config
        };

        configuration_change_proposal
            .set_service_config(BTC_ANCHORING_SERVICE_NAME, service_configuration);

        configuration_change_proposal.set_actual_from(Height(16));
        anchoring_testkit.commit_configuration_change(configuration_change_proposal);
        anchoring_testkit.create_blocks_until(Height(7));

        anchoring_testkit.renew_address();
        anchoring_testkit.create_blocks_until(Height(20));

        let tx1 = anchoring_testkit.last_anchoring_tx();
        //no new transactions

        assert!(tx0 == tx1);
        //creating new funding tx
        let rpc_client = anchoring_testkit.rpc_client();
        let address = anchoring_testkit.anchoring_address();

        let new_funding_tx = rpc_client
            .send_to_address(&address, initial_sum * 3)
            .unwrap();
        let mut configuration_change_proposal = anchoring_testkit.configuration_change_proposal();
        let service_configuration = GlobalConfig {
            funding_transaction: Some(new_funding_tx),
            ..configuration_change_proposal.service_config(BTC_ANCHORING_SERVICE_NAME)
        };

        configuration_change_proposal
            .set_service_config(BTC_ANCHORING_SERVICE_NAME, service_configuration);
        configuration_change_proposal.set_actual_from(Height(24));

        anchoring_testkit.commit_configuration_change(configuration_change_proposal);
        anchoring_testkit.create_blocks_until(Height(26));

        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(28));

        let tx1 = anchoring_testkit.last_anchoring_tx().unwrap();

        assert!(tx1.anchoring_payload().unwrap().prev_tx_chain.is_some());
        assert_eq!(
            tx1.anchoring_payload().unwrap().prev_tx_chain.unwrap(),
            tx0.unwrap().id()
        );

        let signatures = anchoring_testkit.create_signature_tx_for_validators(3);
        anchoring_testkit.create_block_with_transactions(signatures);
        anchoring_testkit.create_blocks_until(Height(32));
        let tx2 = anchoring_testkit.last_anchoring_tx().unwrap();
        println!("---- {:#?}", tx2.anchoring_payload().unwrap());
    }

}
