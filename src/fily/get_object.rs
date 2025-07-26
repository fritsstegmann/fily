use std::sync::Arc;

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use hyper::{HeaderMap, StatusCode};

use super::encryption::{KeyManager, XChaCha20Poly1305Encryptor, Encryptor};
use super::etag::generate_etag;
use super::metadata::{load_metadata, detect_content_type};
use super::path_security::construct_safe_path;
use super::s3_app_error::S3AppError;
use super::Config;

pub async fn handle(
    config: Extension<Arc<Config>>,
    Path((bucket, file)): Path<(String, String)>,
) -> Result<Response, S3AppError> {
    // Check if bucket exists first
    let bucket_path = std::path::Path::new(&config.location).join(&bucket);
    if !bucket_path.exists() {
        return Err(S3AppError::no_such_bucket(&bucket));
    }
    
    match get_object(&config, &bucket, &file).await {
        Ok(contents) => {
            let mut headers = HeaderMap::new();
            
            // Load metadata to get stored content-type and other metadata
            let storage_path = std::path::Path::new(&config.location);
            let metadata = load_metadata(storage_path, &bucket, &file).await;
            
            let (etag, content_type) = match metadata {
                Ok(Some(meta)) => {
                    // Use stored metadata
                    (meta.etag, meta.content_type)
                }
                _ => {
                    // Fallback: generate etag and detect content-type
                    let etag = generate_etag(&contents);
                    let content_type = detect_content_type(&file);
                    (etag, content_type)
                }
            };
            
            headers.insert("etag", etag.parse().unwrap());
            headers.insert("content-type", content_type.parse().unwrap());
            headers.insert("content-length", contents.len().to_string().parse().unwrap());
            
            Ok((StatusCode::OK, headers, contents).into_response())
        },
        Err(e) => {
            // Convert specific IO errors to S3 errors
            if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
                match io_err.kind() {
                    std::io::ErrorKind::NotFound => Err(S3AppError::no_such_key(&bucket, &file)),
                    std::io::ErrorKind::PermissionDenied => Err(S3AppError::access_denied(&format!("/{}/{}", bucket, file))),
                    _ => Err(S3AppError::internal_error(&e.to_string())),
                }
            } else {
                Err(S3AppError::internal_error(&e.to_string()))
            }
        }
    }
}

async fn get_object(config: &Arc<Config>, bucket: &str, file: &str) -> anyhow::Result<Vec<u8>> {
    // Use secure path construction to prevent path traversal attacks
    let storage_root = std::path::Path::new(&config.location);
    let path = construct_safe_path(storage_root, bucket, file)
        .map_err(|e| anyhow::anyhow!("Path security violation: {}", e))?;

    let file_data = tokio::fs::read(&path).await?;

    let decrypted_data = if let Some(encryption_config) = &config.encryption {
        if encryption_config.enabled {
            if let Some(master_key_b64) = &encryption_config.master_key {
                let key_manager = KeyManager::from_base64(master_key_b64)
                    .map_err(|e| anyhow::anyhow!("Encryption key error: {}", e))?;
                let encryptor = XChaCha20Poly1305Encryptor::new(key_manager);
                
                let associated_data = format!("{}/{}", bucket, file);
                encryptor.decrypt(&file_data, associated_data.as_bytes())
                    .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?
            } else {
                return Err(anyhow::anyhow!(
                    "Encryption enabled but no master key provided"
                ));
            }
        } else {
            file_data
        }
    } else {
        file_data
    };

    Ok(decrypted_data)
}
