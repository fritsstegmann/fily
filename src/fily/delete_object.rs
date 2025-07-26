use std::sync::Arc;

use axum::extract::Path;
use axum::response::IntoResponse;
use axum::Extension;
use hyper::StatusCode;

use super::metadata::delete_metadata;
use super::path_security::construct_safe_path;
use super::s3_app_error::S3AppError;
use super::Config;

pub async fn handle(
    config: Extension<Arc<Config>>,
    Path((bucket, file)): Path<(String, String)>,
) -> Result<impl IntoResponse, S3AppError> {
    // Check if bucket exists first
    let bucket_path = std::path::Path::new(&config.location).join(&bucket);
    if !bucket_path.exists() {
        return Err(S3AppError::no_such_bucket(&bucket));
    }
    
    // Use secure path construction to prevent path traversal attacks
    let storage_root = std::path::Path::new(&config.location);
    let path = match construct_safe_path(storage_root, &bucket, &file) {
        Ok(p) => p,
        Err(e) => {
            return Err(S3AppError::with_message(
                super::s3_app_error::S3ErrorCode::InvalidArgument,
                format!("Invalid bucket or object name: {}", e)
            ));
        }
    };
    
    match tokio::fs::remove_file(path).await {
        Ok(_) => {
            // Also clean up metadata
            let storage_path = std::path::Path::new(&config.location);
            if let Err(e) = delete_metadata(storage_path, &bucket, &file).await {
                tracing::warn!("Failed to delete metadata for {}/{}: {}", bucket, file, e);
                // Continue despite metadata cleanup failure
            }
            Ok(StatusCode::NO_CONTENT)
        },
        Err(e) => {
            match e.kind() {
                std::io::ErrorKind::NotFound => Err(S3AppError::no_such_key(&bucket, &file)),
                std::io::ErrorKind::PermissionDenied => Err(S3AppError::access_denied(&format!("/{}/{}", bucket, file))),
                _ => Err(S3AppError::internal_error(&e.to_string())),
            }
        },
    }
}
