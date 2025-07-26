use std::sync::Arc;

use axum::extract::Path;
use axum::response::IntoResponse;
use axum::Extension;
use hyper::StatusCode;
use tracing::{info, error};

use super::s3_app_error::S3AppError;
use super::Config;

async fn is_bucket_empty(bucket_path: &std::path::Path) -> std::io::Result<bool> {
    let mut entries = tokio::fs::read_dir(bucket_path).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        
        // Skip metadata directory
        if file_name_str != ".fily-metadata" {
            return Ok(false);
        }
    }
    
    Ok(true)
}

pub async fn handle(
    config: Extension<Arc<Config>>, 
    Path(bucket): Path<String>
) -> Result<impl IntoResponse, S3AppError> {
    info!("Deleting bucket: {}", bucket);

    let bucket_path = format!("{}/{}", config.location, bucket);
    let path = std::path::Path::new(&bucket_path);
    
    // Check if bucket exists
    if !path.exists() {
        return Err(S3AppError::no_such_bucket(&bucket));
    }
    
    // Check if bucket is empty
    match is_bucket_empty(path).await {
        Ok(false) => {
            info!("Bucket {} is not empty, cannot delete", bucket);
            return Err(S3AppError::bucket_not_empty(&bucket));
        }
        Ok(true) => {
            // Bucket is empty, proceed with deletion
        }
        Err(e) => {
            error!("Failed to check if bucket {} is empty: {}", bucket, e);
            return Err(S3AppError::internal_error(&format!(
                "Failed to check bucket contents: {}", e
            )));
        }
    }

    // Delete the bucket directory
    match tokio::fs::remove_dir_all(&bucket_path).await {
        Ok(_) => {
            info!("Successfully deleted bucket: {}", bucket);
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            error!("Failed to delete bucket {}: {}", bucket, e);
            Err(S3AppError::internal_error(&format!(
                "Failed to delete bucket: {}", e
            )))
        }
    }
}
