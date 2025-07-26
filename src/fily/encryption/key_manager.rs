use hkdf::Hkdf;
use sha2::Sha256;
use base64::{Engine as _, engine::general_purpose};
use super::traits::EncryptionError;

pub struct KeyManager {
    master_key: [u8; 32],
}

impl KeyManager {
    pub fn new(master_key: [u8; 32]) -> Self {
        Self { master_key }
    }

    pub fn from_base64(key_b64: &str) -> Result<Self, EncryptionError> {
        let key_bytes = general_purpose::STANDARD.decode(key_b64)
            .map_err(|e| EncryptionError::InvalidKey(format!("Base64 decode error: {}", e)))?;
        
        if key_bytes.len() != 32 {
            return Err(EncryptionError::InvalidKey(
                "Master key must be 32 bytes".to_string(),
            ));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);
        Ok(Self::new(key))
    }

    pub fn derive_key(&self, context: &[u8]) -> Result<[u8; 32], EncryptionError> {
        let hk = Hkdf::<Sha256>::new(None, &self.master_key);
        let mut derived_key = [0u8; 32];
        hk.expand(context, &mut derived_key)
            .map_err(|e| EncryptionError::InvalidKey(format!("Key derivation failed: {}", e)))?;
        Ok(derived_key)
    }

    pub fn derive_key_for_object(&self, bucket: &str, object: &str) -> Result<[u8; 32], EncryptionError> {
        let context = format!("fily-object:{}/{}", bucket, object);
        self.derive_key(context.as_bytes())
    }
}