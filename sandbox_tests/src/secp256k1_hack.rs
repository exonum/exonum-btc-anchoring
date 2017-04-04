use std::mem;

use libc::c_void;
use byteorder::{LittleEndian, ByteOrder};
use bitcoin::blockdata::script::Script;
use bitcoin::blockdata::transaction::SigHashType;
use secp256k1::ffi;
use secp256k1::{ContextFlag, Secp256k1};
use secp256k1::{Message, Signature};
use secp256k1::key::SecretKey;
use secp256k1::key;
use secp256k1::Error;

use anchoring_btc_service::details::transactions::RawBitcoinTx;

/// The structure with the same memory representation as the `secp256k1::Secp256k1`.
#[derive(Clone, Copy)]
struct Context {
    pub ctx: *mut ffi::Context,
    pub caps: ContextFlag,
}

impl Context {
    /// Same as the 'secp256k1::Secp256k1::sign` but has a nonce argument.
    pub fn sign(&self, msg: &Message, sk: &key::SecretKey, nonce: u64) -> Result<Signature, Error> {
        if self.caps == ContextFlag::VerifyOnly || self.caps == ContextFlag::None {
            return Err(Error::IncapableContext);
        }

        let nonce_array = {
            let mut data = [0; 32];
            LittleEndian::write_u64(&mut data, nonce);
            data
        };

        let mut ret = unsafe { ffi::Signature::blank() };
        unsafe {
            // We can assume the return value because it's not possible to construct
            // an invalid signature from a valid `Message` and `SecretKey`
            assert_eq!(ffi::secp256k1_ecdsa_sign(self.ctx,
                                                 &mut ret,
                                                 msg.as_ptr(),
                                                 sk.as_ptr(),
                                                 ffi::secp256k1_nonce_function_rfc6979,
                                                 nonce_array.as_ptr() as *const c_void),
                       1);
        }
        Ok(Signature::from(ret))
    }
}

fn get_ffi_context(ctx: &mut Secp256k1) -> Context {
    unsafe {
        let ctx_ptr: *mut Context = mem::transmute(ctx as *mut Secp256k1);
        *ctx_ptr
    }
}

fn sign_with_nonce(ctx: &mut Secp256k1,
                   msg: &Message,
                   sk: &key::SecretKey,
                   nonce: u64)
                   -> Result<Signature, Error> {
    let ctx = get_ffi_context(ctx);
    ctx.sign(msg, sk, nonce)
}

pub fn sign_tx_input_with_nonce(tx: &RawBitcoinTx,
                                input: usize,
                                subscript: &Script,
                                sec_key: &SecretKey,
                                nonce: u64)
                                -> Vec<u8> {
    let sighash = tx.signature_hash(input, subscript, SigHashType::All.as_u32());
    // Make signature
    let mut context = Secp256k1::new();
    let msg = Message::from_slice(&sighash[..]).unwrap();
    let sign = sign_with_nonce(&mut context, &msg, sec_key, nonce).unwrap();
    // Serialize signature
    sign.serialize_der(&context)
}
