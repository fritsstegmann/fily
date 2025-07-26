use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use quick_xml::se::to_string;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug)]
pub struct S3Error {
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "Resource")]
    pub resource: String,
    #[serde(rename = "RequestId")]
    pub request_id: String,
}

#[derive(Debug, Clone)]
pub enum S3ErrorCode {
    // Bucket errors
    BucketAlreadyExists,
    BucketAlreadyOwnedByYou,
    BucketNotEmpty,
    NoSuchBucket,
    InvalidBucketName,
    
    // Object errors
    NoSuchKey,
    InvalidObjectName,
    EntityTooLarge,
    EntityTooSmall,
    
    // Authentication errors
    AccessDenied,
    InvalidAccessKeyId,
    SignatureDoesNotMatch,
    TokenRefreshRequired,
    
    // Request errors
    BadRequest,
    InvalidArgument,
    InvalidRequest,
    MalformedXML,
    InvalidDigest,
    BadDigest,
    
    // Server errors
    InternalError,
    NotImplemented,
    ServiceUnavailable,
    SlowDown,
    
    // Multipart upload errors
    NoSuchUpload,
    InvalidPart,
    InvalidPartOrder,
    
    // Generic fallback
    AccountProblem,
}

impl S3ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            S3ErrorCode::BucketAlreadyExists => "BucketAlreadyExists",
            S3ErrorCode::BucketAlreadyOwnedByYou => "BucketAlreadyOwnedByYou",
            S3ErrorCode::BucketNotEmpty => "BucketNotEmpty",
            S3ErrorCode::NoSuchBucket => "NoSuchBucket",
            S3ErrorCode::InvalidBucketName => "InvalidBucketName",
            S3ErrorCode::NoSuchKey => "NoSuchKey",
            S3ErrorCode::InvalidObjectName => "InvalidObjectName",
            S3ErrorCode::EntityTooLarge => "EntityTooLarge",
            S3ErrorCode::EntityTooSmall => "EntityTooSmall",
            S3ErrorCode::AccessDenied => "AccessDenied",
            S3ErrorCode::InvalidAccessKeyId => "InvalidAccessKeyId",
            S3ErrorCode::SignatureDoesNotMatch => "SignatureDoesNotMatch",
            S3ErrorCode::TokenRefreshRequired => "TokenRefreshRequired",
            S3ErrorCode::BadRequest => "BadRequest",
            S3ErrorCode::InvalidArgument => "InvalidArgument",
            S3ErrorCode::InvalidRequest => "InvalidRequest",
            S3ErrorCode::MalformedXML => "MalformedXML",
            S3ErrorCode::InvalidDigest => "InvalidDigest",
            S3ErrorCode::BadDigest => "BadDigest",
            S3ErrorCode::InternalError => "InternalError",
            S3ErrorCode::NotImplemented => "NotImplemented",
            S3ErrorCode::ServiceUnavailable => "ServiceUnavailable",
            S3ErrorCode::SlowDown => "SlowDown",
            S3ErrorCode::NoSuchUpload => "NoSuchUpload",
            S3ErrorCode::InvalidPart => "InvalidPart",
            S3ErrorCode::InvalidPartOrder => "InvalidPartOrder",
            S3ErrorCode::AccountProblem => "AccountProblem",
        }
    }
    
    pub fn http_status(&self) -> StatusCode {
        match self {
            S3ErrorCode::BucketAlreadyExists => StatusCode::CONFLICT,
            S3ErrorCode::BucketAlreadyOwnedByYou => StatusCode::CONFLICT,
            S3ErrorCode::BucketNotEmpty => StatusCode::CONFLICT,
            S3ErrorCode::NoSuchBucket => StatusCode::NOT_FOUND,
            S3ErrorCode::InvalidBucketName => StatusCode::BAD_REQUEST,
            S3ErrorCode::NoSuchKey => StatusCode::NOT_FOUND,
            S3ErrorCode::InvalidObjectName => StatusCode::BAD_REQUEST,
            S3ErrorCode::EntityTooLarge => StatusCode::BAD_REQUEST,
            S3ErrorCode::EntityTooSmall => StatusCode::BAD_REQUEST,
            S3ErrorCode::AccessDenied => StatusCode::FORBIDDEN,
            S3ErrorCode::InvalidAccessKeyId => StatusCode::FORBIDDEN,
            S3ErrorCode::SignatureDoesNotMatch => StatusCode::FORBIDDEN,
            S3ErrorCode::TokenRefreshRequired => StatusCode::BAD_REQUEST,
            S3ErrorCode::BadRequest => StatusCode::BAD_REQUEST,
            S3ErrorCode::InvalidArgument => StatusCode::BAD_REQUEST,
            S3ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            S3ErrorCode::MalformedXML => StatusCode::BAD_REQUEST,
            S3ErrorCode::InvalidDigest => StatusCode::BAD_REQUEST,
            S3ErrorCode::BadDigest => StatusCode::BAD_REQUEST,
            S3ErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            S3ErrorCode::NotImplemented => StatusCode::NOT_IMPLEMENTED,
            S3ErrorCode::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            S3ErrorCode::SlowDown => StatusCode::SERVICE_UNAVAILABLE,
            S3ErrorCode::NoSuchUpload => StatusCode::NOT_FOUND,
            S3ErrorCode::InvalidPart => StatusCode::BAD_REQUEST,
            S3ErrorCode::InvalidPartOrder => StatusCode::BAD_REQUEST,
            S3ErrorCode::AccountProblem => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
    
    pub fn default_message(&self) -> &'static str {
        match self {
            S3ErrorCode::BucketAlreadyExists => "The requested bucket name is not available.",
            S3ErrorCode::BucketAlreadyOwnedByYou => "The bucket you tried to create already exists, and you own it.",
            S3ErrorCode::BucketNotEmpty => "The bucket you tried to delete is not empty.",
            S3ErrorCode::NoSuchBucket => "The specified bucket does not exist.",
            S3ErrorCode::InvalidBucketName => "The specified bucket is not valid.",
            S3ErrorCode::NoSuchKey => "The specified key does not exist.",
            S3ErrorCode::InvalidObjectName => "The specified object name is not valid.",
            S3ErrorCode::EntityTooLarge => "Your proposed upload size exceeds the maximum allowed object size.",
            S3ErrorCode::EntityTooSmall => "Your proposed upload is smaller than the minimum allowed object size.",
            S3ErrorCode::AccessDenied => "Access Denied",
            S3ErrorCode::InvalidAccessKeyId => "The AWS access key Id you provided does not exist in our records.",
            S3ErrorCode::SignatureDoesNotMatch => "The request signature we calculated does not match the signature you provided.",
            S3ErrorCode::TokenRefreshRequired => "The provided token must be refreshed.",
            S3ErrorCode::BadRequest => "Bad Request",
            S3ErrorCode::InvalidArgument => "Invalid Argument",
            S3ErrorCode::InvalidRequest => "Invalid Request",
            S3ErrorCode::MalformedXML => "The XML you provided was not well-formed or did not validate against our published schema.",
            S3ErrorCode::InvalidDigest => "The Content-MD5 you specified is not valid.",
            S3ErrorCode::BadDigest => "The Content-MD5 you specified did not match what we received.",
            S3ErrorCode::InternalError => "We encountered an internal error. Please try again.",
            S3ErrorCode::NotImplemented => "A header you provided implies functionality that is not implemented.",
            S3ErrorCode::ServiceUnavailable => "Reduce your request rate.",
            S3ErrorCode::SlowDown => "Slow Down",
            S3ErrorCode::NoSuchUpload => "The specified multipart upload does not exist.",
            S3ErrorCode::InvalidPart => "One or more of the specified parts could not be found.",
            S3ErrorCode::InvalidPartOrder => "The list of parts was not in ascending order.",
            S3ErrorCode::AccountProblem => "There is a problem with your AWS account that prevents the operation from completing successfully.",
        }
    }
}

// Enhanced S3AppError that supports specific error codes
pub struct S3AppError {
    pub code: S3ErrorCode,
    pub message: Option<String>,
    pub resource: Option<String>,
}

impl S3AppError {
    pub fn new(code: S3ErrorCode) -> Self {
        Self {
            code,
            message: None,
            resource: None,
        }
    }
    
    pub fn with_message(code: S3ErrorCode, message: String) -> Self {
        Self {  
            code,
            message: Some(message),
            resource: None,
        }
    }
    
    pub fn with_resource(code: S3ErrorCode, resource: String) -> Self {
        Self {
            code,
            message: None,
            resource: Some(resource),
        }
    }
    
    pub fn with_message_and_resource(code: S3ErrorCode, message: String, resource: String) -> Self {
        Self {
            code,
            message: Some(message),
            resource: Some(resource),
        }
    }
    
    // Convenience constructors for common errors
    pub fn no_such_bucket(bucket: &str) -> Self {
        Self::with_resource(S3ErrorCode::NoSuchBucket, format!("/{}", bucket))
    }
    
    pub fn no_such_key(bucket: &str, key: &str) -> Self {
        Self::with_resource(S3ErrorCode::NoSuchKey, format!("/{}/{}", bucket, key))
    }
    
    pub fn bucket_already_exists(bucket: &str) -> Self {
        Self::with_resource(S3ErrorCode::BucketAlreadyExists, format!("/{}", bucket))
    }
    
    pub fn bucket_not_empty(bucket: &str) -> Self {
        Self::with_resource(S3ErrorCode::BucketNotEmpty, format!("/{}", bucket))
    }
    
    pub fn invalid_bucket_name(bucket: &str) -> Self {
        Self::with_message_and_resource(
            S3ErrorCode::InvalidBucketName,
            format!("Bucket name '{}' is not valid", bucket),
            format!("/{}", bucket)
        )
    }
    
    pub fn access_denied(resource: &str) -> Self {
        Self::with_resource(S3ErrorCode::AccessDenied, resource.to_string())
    }
    
    pub fn internal_error(message: &str) -> Self {
        Self::with_message(S3ErrorCode::InternalError, message.to_string())
    }
    
    pub fn not_implemented(feature: &str) -> Self {
        Self::with_message(
            S3ErrorCode::NotImplemented,
            format!("Feature '{}' is not implemented", feature)
        )
    }
}

// Tell axum how to convert `S3AppError` into a response.
impl IntoResponse for S3AppError {
    fn into_response(self) -> Response {
        let request_id = Uuid::new_v4().to_string();
        
        let err = S3Error {
            code: self.code.as_str().to_string(),
            message: self.message.unwrap_or_else(|| self.code.default_message().to_string()),
            resource: self.resource.unwrap_or_else(|| "/".to_string()),
            request_id,
        };

        let status_code = self.code.http_status();
        let xml_body = match to_string(&err) {
            Ok(xml) => xml,
            Err(_) => {
                // Fallback if XML serialization fails
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>{}</Code>
    <Message>{}</Message>
    <Resource>{}</Resource>
    <RequestId>{}</RequestId>
</Error>"#,
                    err.code, err.message, err.resource, err.request_id
                )
            }
        };
        
        let mut response = (status_code, xml_body).into_response();
        response.headers_mut().insert(
            "content-type",
            "application/xml".parse().unwrap()
        );
        response.headers_mut().insert(
            "x-amz-request-id", 
            err.request_id.parse().unwrap()
        );
        
        response
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, S3AppError>`. That way you don't need to do that manually.
impl From<anyhow::Error> for S3AppError {
    fn from(err: anyhow::Error) -> Self {
        // Convert anyhow errors to internal server errors by default
        Self::internal_error(&err.to_string())
    }
}

impl From<std::io::Error> for S3AppError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => Self::new(S3ErrorCode::NoSuchKey),
            std::io::ErrorKind::PermissionDenied => Self::new(S3ErrorCode::AccessDenied),
            _ => Self::internal_error(&err.to_string()),
        }
    }
}
