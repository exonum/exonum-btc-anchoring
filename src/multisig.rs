use bitcoin::blockdata::script::Script;
use bitcoin::blockdata::transaction::SigHashType;
use secp256k1::key::{PublicKey, SecretKey};
use secp256k1::{Secp256k1, Message, Signature};

use transactions::RawBitcoinTx;
pub use btc::RedeemScript;

pub fn sign_input(tx: &RawBitcoinTx,
                  input: usize,
                  subscript: &Script,
                  sec_key: &SecretKey)
                  -> Vec<u8> {
    let sighash = tx.signature_hash(input, subscript, SigHashType::All.as_u32());
    // Make signature
    let context = Secp256k1::new();
    let msg = Message::from_slice(&sighash[..]).unwrap();
    let sign = context.sign(&msg, sec_key).unwrap();
    // Serialize signature
    let mut sign_data = sign.serialize_der(&context);
    sign_data.push(SigHashType::All.as_u32() as u8);
    sign_data
}

pub fn verify_input(tx: &RawBitcoinTx,
                    input: usize,
                    subscript: &Script,
                    pub_key: &PublicKey,
                    signature: &[u8])
                    -> bool {
    let sighash = tx.signature_hash(input, subscript, SigHashType::All.as_u32());
    let msg = Message::from_slice(&sighash[..]).unwrap();

    let context = Secp256k1::new();
    if let Ok(sign) = Signature::from_der_lax(&context, signature) {
        context.verify(&msg, &sign, pub_key).is_ok()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    extern crate blockchain_explorer;

    use bitcoin::util::base58::FromBase58;
    use bitcoin::network::constants::Network;
    use bitcoin::util::address::Privkey;
    use secp256k1::key::PublicKey as RawPublicKey;
    use secp256k1::Secp256k1;

    use exonum::crypto::HexValue;

    use HexValueEx;
    use transactions::BitcoinTx;
    use btc::PublicKey;
    use super::{RedeemScript, sign_input, verify_input};

    #[test]
    fn test_redeem_script_from_pubkeys() {
        let redeem_script_hex = "5321027db7837e51888e94c094703030d162c682c8dba312210f44ff440fbd5e5c24732102bdd272891c9e4dfc3962b1fdffd5a59732019816f9db4833634dbdaf01a401a52103280883dc31ccaee34218819aaa245480c35a33acd91283586ff6d1284ed681e52103e2bc790a6e32bf5a766919ff55b1f9e9914e13aed84f502c0e4171976e19deb054ae";
        let keys = ["027db7837e51888e94c094703030d162c682c8dba312210f44ff440fbd5e5c2473",
                    "02bdd272891c9e4dfc3962b1fdffd5a59732019816f9db4833634dbdaf01a401a5",
                    "03280883dc31ccaee34218819aaa245480c35a33acd91283586ff6d1284ed681e5",
                    "03e2bc790a6e32bf5a766919ff55b1f9e9914e13aed84f502c0e4171976e19deb0"]
            .into_iter()
            .map(|x| PublicKey::from_hex(x).unwrap())
            .collect::<Vec<_>>();

        let redeem_script = RedeemScript::from_pubkeys(&keys, 3);
        assert_eq!(redeem_script.to_hex(), redeem_script_hex);
        assert_eq!(redeem_script.to_address(Network::Testnet),
                   "2N1mHzwKTmjnC7JjqeGFBRKYE4WDTjTfop1");
        assert_eq!(RedeemScript::from_hex(redeem_script_hex).unwrap(),
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

        let priv_key =
            Privkey::from_base58check("cVC9eJN5peJemWn1byyWcWDevg6xLNXtACjHJWmrR5ynsCu8mkQE")
                .unwrap();
        let pub_key = {
            let context = Secp256k1::new();
            RawPublicKey::from_secret_key(&context, priv_key.secret_key()).unwrap()
        };

        let redeem_script = RedeemScript::from_hex("5321027db7837e51888e94c094703030d162c682c8dba312210f44ff440fbd5e5c24732102bdd272891c9e4dfc3962b1fdffd5a59732019816f9db4833634dbdaf01a401a52103280883dc31ccaee34218819aaa245480c35a33acd91283586ff6d1284ed681e52103e2bc790a6e32bf5a766919ff55b1f9e9914e13aed84f502c0e4171976e19deb054ae").unwrap();
        let actual_signature = sign_input(&unsigned_tx, 0, &redeem_script, priv_key.secret_key());

        assert_eq!(actual_signature.to_hex(),
                   "304502210092f1fd6367677ef63dfddfb69cb3644ab10a7c497e5cd391e1d36284dca6a570022021dc2132349afafb9273600698d806f6d5f55756fcc058fba4e49c066116124e01");
        assert!(verify_input(&unsigned_tx,
                             0,
                             &redeem_script,
                             &pub_key,
                             actual_signature.as_ref()));
    }

    #[test]
    fn test_redeem_script_pubkey() {
        let redeem_script = RedeemScript::from_hex("55210351d8beec8ef4faef9a299640f2f2c8427b4c5ec655da3bdf9c78bb02debce7052103c39016fa9182f84d367d382b561a3db2154041926e4e461607a903ce2b78dbf72103cba17beba839abbc377f8ff8a908199d544ef821509a45ec3b5684e733e4d73b2102014c953a69d452a8c385d1c68e985d697d04f79bf0ddb11e2852e40b9bb880a4210389cbc7829f40deff4acef55babf7dc486a805ad0f4533e665dee4dd6d38157a32103c60e0aeb3d87b05f49341aa88a347237ab2cff3e91a78d23880080d05f8f08e756ae").unwrap();

        assert_eq!(redeem_script.script_pubkey(Network::Testnet).to_hex(),
                   "a914544fa2db1f36b091bbee603c0bc7675fe34655ff87");
    }
}
