use std::sync::Arc;

use axum::extract::Path;
use axum::response::IntoResponse;
use axum::Extension;
use bytes::Bytes;
use hyper::StatusCode;
use tracing::{debug, info, error};

use super::s3_app_error::S3AppError;
use super::Config;

fn is_valid_bucket_name(bucket: &str) -> bool {
    // S3 bucket naming rules (simplified)
    if bucket.len() < 3 || bucket.len() > 63 {
        return false;
    }
    
    // Must start and end with lowercase letter or number
    let first_char = bucket.chars().next().unwrap();
    let last_char = bucket.chars().last().unwrap();
    if !first_char.is_ascii_lowercase() && !first_char.is_ascii_digit() {
        return false;
    }
    if !last_char.is_ascii_lowercase() && !last_char.is_ascii_digit() {
        return false;
    }
    
    // Only lowercase letters, numbers, hyphens, and periods
    for c in bucket.chars() {
        if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-' && c != '.' {
            return false;
        }
    }
    
    // Cannot be formatted as IP address (simplified check)
    if bucket.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return false;
    }
    
    true
}

pub async fn handle(
    config: Extension<Arc<Config>>, 
    Path(bucket): Path<String>, 
    body: Bytes
) -> Result<impl IntoResponse, S3AppError> {
    info!("Creating bucket: {}", bucket);
    debug!("Request body: {:?}", body);

    // Validate bucket name
    if !is_valid_bucket_name(&bucket) {
        return Err(S3AppError::invalid_bucket_name(&bucket));
    }

    let bucket_path = format!("{}/{}", config.location, bucket);
    let path = std::path::Path::new(&bucket_path);
    
    // Check if bucket already exists
    if path.exists() {
        info!("Bucket {} already exists", bucket);
        return Err(S3AppError::bucket_already_exists(&bucket));
    }

    // Create the bucket directory
    match tokio::fs::create_dir_all(&bucket_path).await {
        Ok(_) => {
            info!("Successfully created bucket: {}", bucket);
            Ok(StatusCode::OK)
        }
        Err(e) => {
            error!("Failed to create bucket {}: {}", bucket, e);
            Err(S3AppError::internal_error(&format!(
                "Failed to create bucket: {}", e
            )))
        }
    }
}
