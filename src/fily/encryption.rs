pub mod key_manager;
pub mod traits;
pub mod xchacha20poly1305;

pub use key_manager::KeyManager;
pub use traits::{Encryptor, EncryptionError};
pub use xchacha20poly1305::XChaCha20Poly1305Encryptor;

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose};

    #[test]
    fn test_xchacha20poly1305_encryption_roundtrip() {
        let key_bytes = [1u8; 32];
        let key_b64 = general_purpose::STANDARD.encode(&key_bytes);
        
        let key_manager = KeyManager::from_base64(&key_b64).unwrap();
        let encryptor = XChaCha20Poly1305Encryptor::new(key_manager);
        
        let plaintext = b"Hello, encrypted world!";
        let associated_data = b"test/file.txt";
        
        let encrypted = encryptor.encrypt(plaintext, associated_data).unwrap();
        assert!(encrypted.len() > plaintext.len());
        
        let decrypted = encryptor.decrypt(&encrypted, associated_data).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_key_derivation() {
        let key_manager = KeyManager::new([2u8; 32]);
        
        let key1 = key_manager.derive_key(b"context1").unwrap();
        let key2 = key_manager.derive_key(b"context2").unwrap();
        let key3 = key_manager.derive_key(b"context1").unwrap();
        
        assert_ne!(key1, key2);
        assert_eq!(key1, key3);
    }

    #[test]
    fn test_invalid_base64_key() {
        let result = KeyManager::from_base64("invalid-base64!");
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_key_size() {
        let short_key = general_purpose::STANDARD.encode(&[1u8; 16]);
        let result = KeyManager::from_base64(&short_key);
        assert!(result.is_err());
    }
}