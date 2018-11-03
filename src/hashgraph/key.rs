use ring::{error, rand, signature};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

pub struct Key {
    pub bytes: [u8; 85], // necessary to impl Clone
    pub key_pair: signature::Ed25519KeyPair,
}

impl Clone for Key {
    fn clone(&self) -> Key {
        let key_pair =
            signature::Ed25519KeyPair::from_pkcs8(untrusted::Input::from(&self.bytes)).unwrap();

        Key {
            bytes: self.bytes.clone(),
            key_pair,
        }
    }
}

impl Key {
    pub fn new_generate() -> Result<Key, error::Unspecified> {
        let rng = rand::SystemRandom::new();
        let pkcs8_bytes = signature::Ed25519KeyPair::generate_pkcs8(&rng)?;

        let key_pair = signature::Ed25519KeyPair::from_pkcs8(untrusted::Input::from(&pkcs8_bytes))?;

        Ok(Key {
            bytes: pkcs8_bytes,
            key_pair,
        })
    }

    pub fn get_pub(&self) -> Vec<u8> {
        self.key_pair.public_key_bytes().to_vec()
    }

    pub fn sign(self, msg: &[u8]) -> Vec<u8> {
        let sig = self.key_pair.sign(msg);

        let res = sig.as_ref();

        res.to_vec()
    }

    pub fn verify(pub_key: Vec<u8>, sig: Vec<u8>, msg: Vec<u8>) -> bool {
        let peer_public_key = untrusted::Input::from(pub_key.as_slice());
        let msg = untrusted::Input::from(msg.as_slice());
        let sig = untrusted::Input::from(sig.as_slice());

        if let Err(_) = signature::verify(&signature::ED25519, peer_public_key, msg, sig) {
            false
        } else {
            true
        }
    }

    pub fn pub_to_int(pub_key: Vec<u8>) -> u64 {
        let mut hasher = DefaultHasher::new();

        hasher.write(&pub_key);
        hasher.finish()
    }
}
