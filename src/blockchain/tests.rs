use exonum::crypto::{PublicKey, Signature, HexValue};

use bitcoin::blockdata::transaction::SigHashType;

use details::transactions::AnchoringTx;
use details::btc;
use blockchain::dto::MsgAnchoringSignature;

#[test]
fn test_sighash_type_all_in_msg_signature() {
    let tx = AnchoringTx::from_hex("01000000019aaf09d7e73a5f9ab394f1358bfb3dbde7b15b983d715f5c98f369a3f0a288a70000000000ffffffff02b80b00000000000017a914f18eb74087f751109cc9052befd4177a52c9a30a8700000000000000002c6a2a012800000000000000007fab6f66a0f7a747c820cd01fa30d7bdebd26b91c6e03f742abac0b3108134d900000000").unwrap();
    let btc_signature = btc::Signature::from_hex("3044022061d0bd408ec10f4f901c6d548151cc53031a3083f28dbcfc132319a162421d24022074f8a1c182088389bfae8646d9d99dea5b47db8f795d02efcc41ab4da0a8e11b01").unwrap();
    let msg = MsgAnchoringSignature::new_with_signature(&PublicKey::zero(),
                                                        0,
                                                        tx,
                                                        0,
                                                        &btc_signature,
                                                        &Signature::zero());

    assert!(msg.verify_content());
}

#[test]
fn test_sighash_type_single_in_msg_signature() {
    let tx = AnchoringTx::from_hex("01000000019aaf09d7e73a5f9ab394f1358bfb3dbde7b15b983d715f5c98f369a3f0a288a70000000000ffffffff02b80b00000000000017a914f18eb74087f751109cc9052befd4177a52c9a30a8700000000000000002c6a2a012800000000000000007fab6f66a0f7a747c820cd01fa30d7bdebd26b91c6e03f742abac0b3108134d900000000").unwrap();
    let mut btc_signature = btc::Signature::from_hex("3044022061d0bd408ec10f4f901c6d548151cc53031a3083f28dbcfc132319a162421d24022074f8a1c182088389bfae8646d9d99dea5b47db8f795d02efcc41ab4da0a8e11b01").unwrap();
    *btc_signature.last_mut().unwrap() = SigHashType::Single.as_u32() as u8;
    let msg = MsgAnchoringSignature::new_with_signature(&PublicKey::zero(),
                                                        0,
                                                        tx,
                                                        0,
                                                        &btc_signature,
                                                        &Signature::zero());

    assert!(!msg.verify_content());
}
