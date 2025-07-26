use std::sync::Arc;

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use bytes::Bytes;
use hyper::{HeaderMap, StatusCode};
use tracing::{debug, info, error, instrument};

use super::encryption::{Encryptor, KeyManager, XChaCha20Poly1305Encryptor};
use super::etag::generate_etag;
use super::metadata::{ObjectMetadata, extract_user_metadata, save_metadata};
use super::path_security::construct_safe_path;
use super::s3_app_error::S3AppError;
use super::Config;

#[instrument(
    name = "put_object",
    skip(config, headers, bytes),
    fields(
        bucket = %bucket,
        object = %file,
        content_length = bytes.len(),
        content_type = headers.get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("unknown"),
        has_encryption = config.encryption.as_ref().map(|e| e.enabled).unwrap_or(false)
    )
)]
pub async fn handle(
    config: Extension<Arc<Config>>,
    headers: HeaderMap,
    Path((bucket, file)): Path<(String, String)>,
    bytes: Bytes,
) -> anyhow::Result<Response, S3AppError> {
    info!("Starting PUT object operation for {}/{}", bucket, file);
    debug!("Request headers: {:?}", headers);
    debug!("Content length: {} bytes", bytes.len());

    // Use secure path construction to prevent path traversal attacks
    let storage_root = std::path::Path::new(&config.location);
    let path = match construct_safe_path(storage_root, &bucket, &file) {
        Ok(p) => p,
        Err(e) => {
            error!("Path security violation: {}", e);
            return Err(S3AppError::with_message(
                super::s3_app_error::S3ErrorCode::InvalidArgument,
                format!("Invalid bucket or object name: {}", e)
            ));
        }
    };
    
    debug!("Target file path: {}", path.display());
    
    let prefix = path.parent();
    match prefix {
        Some(prefix) => {
            debug!("Creating directory structure: {}", prefix.display());
            tokio::fs::create_dir_all(prefix).await
                .map_err(|e| {
                    error!("Failed to create directory structure {}: {}", prefix.display(), e);
                    anyhow::anyhow!("Directory creation failed: {}", e)
                })?;

            let data_to_write = if let Some(encryption_config) = &config.encryption {
                if encryption_config.enabled {
                    info!("Encryption is enabled, encrypting object data");
                    
                    if let Some(master_key_b64) = &encryption_config.master_key {
                        debug!("Initializing XChaCha20-Poly1305 encryptor");
                        let key_manager = KeyManager::from_base64(master_key_b64)
                            .map_err(|e| {
                                error!("Failed to initialize encryption key manager: {}", e);
                                anyhow::anyhow!("Encryption key error: {}", e)
                            })?;
                        let encryptor = XChaCha20Poly1305Encryptor::new(key_manager);

                        let associated_data = format!("{}/{}", bucket, file);
                        debug!("Using associated data for encryption: {}", associated_data);
                        
                        let encrypted_data = encryptor
                            .encrypt(bytes.as_ref(), associated_data.as_bytes())
                            .map_err(|e| {
                                error!("Encryption failed for {}/{}: {}", bucket, file, e);
                                anyhow::anyhow!("Encryption failed: {}", e)
                            })?;
                        
                        info!("Successfully encrypted object data (original: {} bytes, encrypted: {} bytes)", 
                              bytes.len(), encrypted_data.len());
                        encrypted_data
                    } else {
                        error!("Encryption is enabled but no master key provided in configuration");
                        return Err(S3AppError::internal_error(
                            "Encryption enabled but no master key provided"
                        ));
                    }
                } else {
                    debug!("Encryption is disabled, storing object data unencrypted");
                    bytes.to_vec()
                }
            } else {
                debug!("No encryption configuration found, storing object data unencrypted");
                bytes.to_vec()
            };

            debug!("Writing {} bytes to disk at {}", data_to_write.len(), path.display());
            tokio::fs::write(&path, &data_to_write).await
                .map_err(|e| {
                    error!("Failed to write object {}/{} to disk: {}", bucket, file, e);
                    anyhow::anyhow!("File write failed: {}", e)
                })?;
            
            // Generate e-tag for the original content (before encryption)
            let etag = generate_etag(bytes.as_ref());
            
            // Extract content-type from headers
            let content_type = headers
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            
            // Create and store metadata
            let mut metadata = ObjectMetadata::new(
                content_type.clone(),
                bytes.len() as u64,
                etag.clone(),
                &file,
            );
            
            // Add user metadata from x-amz-meta-* headers
            let user_metadata = extract_user_metadata(&headers);
            for (key, value) in user_metadata {
                metadata.add_user_metadata(key, value);
            }
            
            // Save metadata to disk
            let storage_path = std::path::Path::new(&config.location);
            if let Err(e) = save_metadata(storage_path, &bucket, &file, &metadata).await {
                error!("Failed to save metadata for {}/{}: {}", bucket, file, e);
                // Continue despite metadata save failure
            }
            
            let mut response_headers = HeaderMap::new();
            response_headers.insert("etag", etag.parse().unwrap());
            
            // Include content-type in response if provided
            if let Some(ct) = content_type {
                if let Ok(ct_value) = ct.parse() {
                    response_headers.insert("content-type", ct_value);
                }
            }
                
            info!("Successfully stored object {}/{} ({} bytes)", bucket, file, data_to_write.len());
            Ok((StatusCode::OK, response_headers, "").into_response())
        }
        None => {
            error!("Failed to determine parent directory for path: {}", path.display());
            Err(S3AppError::internal_error(&format!(
                "Failed getting parent path for: {}", path.display()
            )))
        }
    }
}
