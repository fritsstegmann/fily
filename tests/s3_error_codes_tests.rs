use axum::http::StatusCode;
use fily::fily::s3_app_error::{S3AppError, S3ErrorCode};

#[test]
fn test_s3_error_code_strings() {
    assert_eq!(S3ErrorCode::NoSuchBucket.as_str(), "NoSuchBucket");
    assert_eq!(S3ErrorCode::NoSuchKey.as_str(), "NoSuchKey");
    assert_eq!(S3ErrorCode::BucketAlreadyExists.as_str(), "BucketAlreadyExists");
    assert_eq!(S3ErrorCode::InvalidBucketName.as_str(), "InvalidBucketName");
    assert_eq!(S3ErrorCode::AccessDenied.as_str(), "AccessDenied");
    assert_eq!(S3ErrorCode::InternalError.as_str(), "InternalError");
}

#[test]
fn test_s3_error_http_status_codes() {
    assert_eq!(S3ErrorCode::NoSuchBucket.http_status(), StatusCode::NOT_FOUND);
    assert_eq!(S3ErrorCode::NoSuchKey.http_status(), StatusCode::NOT_FOUND);
    assert_eq!(S3ErrorCode::BucketAlreadyExists.http_status(), StatusCode::CONFLICT);
    assert_eq!(S3ErrorCode::InvalidBucketName.http_status(), StatusCode::BAD_REQUEST);
    assert_eq!(S3ErrorCode::AccessDenied.http_status(), StatusCode::FORBIDDEN);
    assert_eq!(S3ErrorCode::InternalError.http_status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(S3ErrorCode::BucketNotEmpty.http_status(), StatusCode::CONFLICT);
}

#[test]
fn test_s3_error_default_messages() {
    assert_eq!(
        S3ErrorCode::NoSuchBucket.default_message(),
        "The specified bucket does not exist."
    );
    assert_eq!(
        S3ErrorCode::NoSuchKey.default_message(),
        "The specified key does not exist."
    );
    assert_eq!(
        S3ErrorCode::BucketAlreadyExists.default_message(),
        "The requested bucket name is not available."
    );
    assert_eq!(
        S3ErrorCode::InvalidBucketName.default_message(),
        "The specified bucket is not valid."
    );
}

#[test]
fn test_s3_app_error_constructors() {
    let bucket_err = S3AppError::no_such_bucket("test-bucket");
    assert!(matches!(bucket_err.code, S3ErrorCode::NoSuchBucket));
    assert_eq!(bucket_err.resource, Some("/test-bucket".to_string()));

    let key_err = S3AppError::no_such_key("test-bucket", "test-key");
    assert!(matches!(key_err.code, S3ErrorCode::NoSuchKey));
    assert_eq!(key_err.resource, Some("/test-bucket/test-key".to_string()));

    let bucket_exists_err = S3AppError::bucket_already_exists("existing");
    assert!(matches!(bucket_exists_err.code, S3ErrorCode::BucketAlreadyExists));
    assert_eq!(bucket_exists_err.resource, Some("/existing".to_string()));

    let invalid_name_err = S3AppError::invalid_bucket_name("bad-name");
    assert!(matches!(invalid_name_err.code, S3ErrorCode::InvalidBucketName));
    assert_eq!(invalid_name_err.resource, Some("/bad-name".to_string()));
    assert!(invalid_name_err.message.is_some());

    let access_denied_err = S3AppError::access_denied("/test/resource");
    assert!(matches!(access_denied_err.code, S3ErrorCode::AccessDenied));
    assert_eq!(access_denied_err.resource, Some("/test/resource".to_string()));

    let internal_err = S3AppError::internal_error("Something went wrong");
    assert!(matches!(internal_err.code, S3ErrorCode::InternalError));
    assert_eq!(internal_err.message, Some("Something went wrong".to_string()));
}

#[test]
fn test_s3_app_error_builder_methods() {
    let err = S3AppError::new(S3ErrorCode::BadRequest);
    assert!(matches!(err.code, S3ErrorCode::BadRequest));
    assert!(err.message.is_none());
    assert!(err.resource.is_none());

    let err_with_msg = S3AppError::with_message(
        S3ErrorCode::InvalidArgument,
        "Custom message".to_string()
    );
    assert!(matches!(err_with_msg.code, S3ErrorCode::InvalidArgument));
    assert_eq!(err_with_msg.message, Some("Custom message".to_string()));
    assert!(err_with_msg.resource.is_none());

    let err_with_resource = S3AppError::with_resource(
        S3ErrorCode::NoSuchKey,
        "/bucket/key".to_string()
    );
    assert!(matches!(err_with_resource.code, S3ErrorCode::NoSuchKey));
    assert!(err_with_resource.message.is_none());
    assert_eq!(err_with_resource.resource, Some("/bucket/key".to_string()));

    let err_full = S3AppError::with_message_and_resource(
        S3ErrorCode::AccessDenied,
        "Access denied message".to_string(),
        "/protected/resource".to_string()
    );
    assert!(matches!(err_full.code, S3ErrorCode::AccessDenied));
    assert_eq!(err_full.message, Some("Access denied message".to_string()));
    assert_eq!(err_full.resource, Some("/protected/resource".to_string()));
}

#[test]
fn test_io_error_conversion() {
    let not_found_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let s3_err = S3AppError::from(not_found_err);
    assert!(matches!(s3_err.code, S3ErrorCode::NoSuchKey));

    let permission_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Permission denied");
    let s3_err = S3AppError::from(permission_err);
    assert!(matches!(s3_err.code, S3ErrorCode::AccessDenied));

    let other_err = std::io::Error::new(std::io::ErrorKind::Other, "Other error");
    let s3_err = S3AppError::from(other_err);
    assert!(matches!(s3_err.code, S3ErrorCode::InternalError));
}

#[test]
fn test_anyhow_error_conversion() {
    let anyhow_err = anyhow::anyhow!("Some error occurred");
    let s3_err = S3AppError::from(anyhow_err);
    assert!(matches!(s3_err.code, S3ErrorCode::InternalError));
    assert!(s3_err.message.is_some());
}

#[tokio::test]
async fn test_error_response_format() {
    use axum::response::IntoResponse;
    
    let error = S3AppError::no_such_bucket("test-bucket");
    let response = error.into_response();
    
    // Check status code
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    
    // Check content type header
    let content_type = response.headers().get("content-type");
    assert_eq!(content_type.unwrap().to_str().unwrap(), "application/xml");
    
    // Check that x-amz-request-id header exists
    let request_id = response.headers().get("x-amz-request-id");
    assert!(request_id.is_some());
    
    // Extract body and check XML structure
    let (_, body) = response.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    
    assert!(body_str.contains("<Code>NoSuchBucket</Code>"));
    assert!(body_str.contains("<Message>The specified bucket does not exist.</Message>"));
    assert!(body_str.contains("<Resource>/test-bucket</Resource>"));
    assert!(body_str.contains("<RequestId>"));
}

#[tokio::test]
async fn test_custom_error_message_in_response() {
    use axum::response::IntoResponse;
    
    let error = S3AppError::with_message_and_resource(
        S3ErrorCode::InvalidBucketName,
        "Custom error message".to_string(),
        "/custom-bucket".to_string()
    );
    let response = error.into_response();
    
    let (_, body) = response.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    
    assert!(body_str.contains("<Code>InvalidBucketName</Code>"));
    assert!(body_str.contains("<Message>Custom error message</Message>"));
    assert!(body_str.contains("<Resource>/custom-bucket</Resource>"));
}