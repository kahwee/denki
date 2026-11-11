use std::sync::atomic::{AtomicI32, Ordering};

use aes::Aes128;
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding};
use cbc::{Decryptor, Encryptor};
use subtle::ConstantTimeEq;

use super::crypto;

pub(super) struct KlapCipher {
    key: [u8; 16],
    iv: [u8; 12],
    seq: AtomicI32,
    sig: [u8; 28],
}

impl KlapCipher {
    pub fn new(local_seed: &[u8], remote_seed: &[u8], user_hash: &[u8]) -> anyhow::Result<Self> {
        let local_hash = &[local_seed, remote_seed, user_hash].concat();

        let (iv, seq) = Self::iv_derive(local_hash)?;

        Ok(Self {
            key: Self::key_derive(local_hash),
            iv,
            seq: AtomicI32::new(seq),
            sig: Self::sig_derive(local_hash),
        })
    }

    pub fn encrypt(&self, data: &str) -> anyhow::Result<(Vec<u8>, i32)> {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        let encryptor = Encryptor::<Aes128>::new_from_slices(&self.key, &self.iv_seq(seq))?;

        let cipher_bytes = encryptor.encrypt_padded_vec::<block_padding::Pkcs7>(data.as_bytes());

        let signature = crypto::sha256(
            &[
                self.sig.as_slice(),
                &seq.to_be_bytes(),
                cipher_bytes.as_slice(),
            ]
            .concat(),
        );

        let result = [&signature, cipher_bytes.as_slice()].concat();

        Ok((result, seq))
    }

    pub fn decrypt(&self, seq: i32, cipher_bytes: &[u8]) -> anyhow::Result<String> {
        let decryptor = Decryptor::<Aes128>::new_from_slices(&self.key, &self.iv_seq(seq))?;

        let ciphertext = cipher_bytes.get(32..).ok_or_else(|| {
            anyhow::anyhow!(
                "Response of {} bytes is shorter than its 32-byte signature",
                cipher_bytes.len()
            )
        })?;

        let expected =
            crypto::sha256(&[self.sig.as_slice(), &seq.to_be_bytes(), ciphertext].concat());
        if expected.ct_eq(&cipher_bytes[..32]).unwrap_u8() != 1 {
            anyhow::bail!("KLAP response signature mismatch");
        }

        let decrypted_bytes = decryptor
            .decrypt_padded_vec::<block_padding::Pkcs7>(ciphertext)
            .map_err(|e| anyhow::anyhow!("Decryption error: {:?}", e))?;
        let decrypted = std::str::from_utf8(&decrypted_bytes)?.to_string();

        Ok(decrypted)
    }
}

impl KlapCipher {
    fn key_derive(local_hash: &[u8]) -> [u8; 16] {
        let local_hash = &["lsk".as_bytes(), local_hash].concat();
        let hash = crypto::sha256(local_hash);
        let mut key = [0u8; 16];
        key.copy_from_slice(&hash[..16]);
        key
    }

    fn iv_derive(local_hash: &[u8]) -> anyhow::Result<([u8; 12], i32)> {
        let local_hash = &["iv".as_bytes(), local_hash].concat();
        let hash = crypto::sha256(local_hash);
        let mut iv = [0u8; 12];
        iv.copy_from_slice(&hash[..12]);
        let seq: [u8; 4] = hash[hash.len() - 4..].try_into()?;
        let seq = i32::from_be_bytes(seq);
        Ok((iv, seq))
    }

    fn sig_derive(local_hash: &[u8]) -> [u8; 28] {
        let local_hash = &["ldk".as_bytes(), local_hash].concat();
        let hash = crypto::sha256(local_hash);
        let mut sig = [0u8; 28];
        sig.copy_from_slice(&hash[..28]);
        sig
    }

    fn iv_seq(&self, seq: i32) -> [u8; 16] {
        let mut iv_seq = [0u8; 16];
        iv_seq[..12].copy_from_slice(&self.iv);
        iv_seq[12..].copy_from_slice(&seq.to_be_bytes());
        iv_seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decrypt_rejects_a_response_shorter_than_the_signature() {
        let cipher = KlapCipher::new(&[0; 16], &[0; 16], &[0; 32]).unwrap();

        assert!(cipher.decrypt(1, &[0; 31]).is_err());
    }
    #[test]
    fn verifies_signature_before_decryption_and_binds_sequence() {
        let cipher = KlapCipher::new(&[1; 16], &[2; 16], &[3; 32]).unwrap();
        let (packet, seq) = cipher.encrypt("fixture response").unwrap();
        assert_eq!(cipher.decrypt(seq, &packet).unwrap(), "fixture response");
        assert!(
            cipher
                .decrypt(seq.wrapping_add(1), &packet)
                .unwrap_err()
                .to_string()
                .contains("signature mismatch")
        );
        for index in [0, 31, 32, packet.len() - 1] {
            let mut tampered = packet.clone();
            tampered[index] ^= 1;
            assert!(
                cipher
                    .decrypt(seq, &tampered)
                    .unwrap_err()
                    .to_string()
                    .contains("signature mismatch")
            );
        }
    }
}
