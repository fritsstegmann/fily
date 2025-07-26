use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("Invalid nonce: {0}")]
    InvalidNonce(String),
}

pub trait Encryptor: Send + Sync {
    fn encrypt(&self, plaintext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, EncryptionError>;
    fn decrypt(&self, ciphertext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, EncryptionError>;
}