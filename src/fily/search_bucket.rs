use std::sync::Arc;

use axum::response::IntoResponse;
use axum::Extension;
use hyper::StatusCode;

use super::s3_app_error::S3AppError;
use super::Config;

pub async fn handle(_: Extension<Arc<Config>>) -> Result<impl IntoResponse, S3AppError> {
    Ok(StatusCode::OK)
}
