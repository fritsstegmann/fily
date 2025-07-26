use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce
};
use rand::RngCore;
use super::traits::{Encryptor, EncryptionError};
use super::key_manager::KeyManager;

pub struct XChaCha20Poly1305Encryptor {
    key_manager: KeyManager,
}

impl XChaCha20Poly1305Encryptor {
    pub fn new(key_manager: KeyManager) -> Self {
        Self { key_manager }
    }

    fn generate_nonce() -> XNonce {
        let mut nonce_bytes = [0u8; 24];
        OsRng.fill_bytes(&mut nonce_bytes);
        *XNonce::from_slice(&nonce_bytes)
    }
}

impl Encryptor for XChaCha20Poly1305Encryptor {
    fn encrypt(&self, plaintext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let derived_key = self.key_manager.derive_key(associated_data)?;
        let cipher = XChaCha20Poly1305::new_from_slice(&derived_key)
            .map_err(|e| EncryptionError::InvalidKey(format!("Cipher creation failed: {}", e)))?;

        let nonce = Self::generate_nonce();
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| EncryptionError::EncryptionFailed(format!("Encryption failed: {}", e)))?;

        let mut result = Vec::with_capacity(24 + ciphertext.len());
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    fn decrypt(&self, encrypted_data: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        if encrypted_data.len() < 24 + 16 {
            return Err(EncryptionError::DecryptionFailed(
                "Encrypted data too short".to_string(),
            ));
        }

        let (nonce_bytes, ciphertext) = encrypted_data.split_at(24);
        let nonce = *XNonce::from_slice(nonce_bytes);

        let derived_key = self.key_manager.derive_key(associated_data)?;
        let cipher = XChaCha20Poly1305::new_from_slice(&derived_key)
            .map_err(|e| EncryptionError::InvalidKey(format!("Cipher creation failed: {}", e)))?;

        cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| EncryptionError::DecryptionFailed(format!("Decryption failed: {}", e)))
    }
}