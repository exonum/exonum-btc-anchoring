use secp256k1::Secp256k1;
use secp256k1::key;
use secp256k1::Error;

use super::types::{PublicKey, RawPublicKey};

impl PublicKey {
    pub fn from_secret_key(secp: &Secp256k1, sk: &key::SecretKey) -> Result<PublicKey, Error> {
        let raw = RawPublicKey::from_secret_key(secp, sk)?;
        Ok(PublicKey::from(raw))
    }

    pub fn to_bytes(&self) -> [u8; 33] {
        let ctx = Secp256k1::without_caps();
        let vec = self.0.serialize_vec(&ctx, true);
        let mut bytes = [0; 33];
        bytes.copy_from_slice(&vec);
        bytes
    }
}
