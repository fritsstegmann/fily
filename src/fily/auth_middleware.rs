use std::sync::Arc;

use std::task::{Context, Poll};

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::Response;
use http_body_util::BodyExt;
use tower::{Layer, Service};
use tracing::{error, info, warn};

use super::auth::{AuthError, AwsSignatureV4Validator};
use super::s3_app_error::S3Error;
use super::Config;

#[derive(Clone)]
pub struct AuthMiddleware<S> {
    inner: S,
    validator: Arc<AwsSignatureV4Validator>,
    config: Arc<Config>,
}

impl<S> AuthMiddleware<S> {
    pub fn new(inner: S, validator: Arc<AwsSignatureV4Validator>, config: Arc<Config>) -> Self {
        Self { inner, validator, config }
    }
}

impl<S> Service<Request> for AuthMiddleware<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let validator = self.validator.clone();
        let config = self.config.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Extract request components
            let method = req.method().clone();
            let uri = req.uri().clone();
            let headers = req.headers().clone();


            // Collect the body
            let (parts, body) = req.into_parts();
            let body_bytes = match body.collect().await {
                Ok(collected) => collected.to_bytes(),
                Err(e) => {
                    error!("Failed to collect request body: {}", e);
                    return Ok(create_error_response(
                        StatusCode::BAD_REQUEST,
                        "MalformedRequest",
                        "Failed to read request body",
                    ));
                }
            };

            // Check if this is a pre-signed URL request
            let is_presigned = uri.query().map_or(false, |q| {
                let has_algorithm = q.contains("X-Amz-Algorithm");
                let has_signature = q.contains("X-Amz-Signature");

                has_algorithm && has_signature
            });

            if is_presigned {
                info!(
                    "Detected pre-signed URL request for {} {}",
                    method,
                    uri.path()
                );
            }

            // Extract bucket and object from URI for cache optimization
            let (bucket, object) = parse_bucket_and_object_from_uri(&uri);
            
            // Validate the signature (header-based or query parameter-based)
            let auth_result = if is_presigned {
                validator
                    .validate_presigned_request(&method, &uri, &headers, &body_bytes)
                    .await
            } else {
                // Use optimized validation with cache lookup for regular requests
                let storage_path = std::path::Path::new(&config.location);
                validator
                    .validate_request_with_object_info(
                        &method, 
                        &uri, 
                        &headers, 
                        &body_bytes,
                        Some(storage_path),
                        bucket.as_deref(),
                        object.as_deref(),
                    )
                    .await
            };

            match auth_result {
                Ok(access_key_id) => {
                    info!(
                        "Successfully authenticated request for access key: {}",
                        access_key_id
                    );

                    // Reconstruct the request with the original body
                    let new_body = Body::from(body_bytes);
                    let new_req = Request::from_parts(parts, new_body);

                    // Continue with the request
                    inner.call(new_req).await
                }
                Err(auth_error) => {
                    warn!("Authentication failed: {}", auth_error);

                    let (status_code, error_code, message): (StatusCode, &str, String) = match auth_error {
                        AuthError::MissingAuthorizationHeader => (
                            StatusCode::UNAUTHORIZED,
                            "MissingSecurityHeader",
                            "Your request was missing a required header.".to_string(),
                        ),
                        AuthError::InvalidAuthorizationHeader => (
                            StatusCode::UNAUTHORIZED,
                            "InvalidRequest",
                            "The authorization header is malformed.".to_string(),
                        ),
                        AuthError::MissingRequiredHeader(header) => (
                            StatusCode::BAD_REQUEST,
                            "MissingSecurityHeader",
                            format!("Your request was missing a required header: {}", header),
                        ),
                        AuthError::InvalidDateFormat => (
                            StatusCode::UNAUTHORIZED,
                            "InvalidRequest",
                            "The date header is malformed.".to_string(),
                        ),
                        AuthError::SignatureVerificationFailed => (
                            StatusCode::FORBIDDEN,
                            "SignatureDoesNotMatch",
                            "The request signature we calculated does not match the signature you provided.".to_string(),
                        ),
                        AuthError::InvalidAccessKey => (
                            StatusCode::FORBIDDEN,
                            "InvalidAccessKeyId",
                            "The AWS access key ID you provided does not exist in our records.".to_string(),
                        ),
                        AuthError::RequestTooOld => (
                            StatusCode::FORBIDDEN,
                            "RequestTimeTooSkewed",
                            "The difference between the request time and the current time is too large.".to_string(),
                        ),
                        AuthError::MalformedRequest => (
                            StatusCode::BAD_REQUEST,
                            "MalformedRequest",
                            "The request is malformed.".to_string(),
                        ),
                        AuthError::MissingPresignedParameter(param) => (
                            StatusCode::BAD_REQUEST,
                            "InvalidRequest",
                            format!("Pre-signed URL is missing required parameter: {}", param),
                        ),
                        AuthError::InvalidExpiration => (
                            StatusCode::BAD_REQUEST,
                            "InvalidRequest",
                            "Invalid expiration time for pre-signed URL.".to_string(),
                        ),
                        AuthError::PresignedUrlExpired => (
                            StatusCode::FORBIDDEN,
                            "AccessDenied",
                            "Pre-signed URL has expired.".to_string(),
                        ),
                        AuthError::InvalidAccessKeyIdFormat(msg) => (
                            StatusCode::BAD_REQUEST,
                            "InvalidAccessKeyId",
                            format!("Invalid access key ID format: {}", msg),
                        ),
                        AuthError::InvalidSecretAccessKeyFormat(msg) => (
                            StatusCode::BAD_REQUEST,
                            "InvalidSecretAccessKey", 
                            format!("Invalid secret access key format: {}", msg),
                        ),
                    };

                    Ok(create_error_response(status_code, error_code, &message))
                }
            }
        })
    }
}

#[derive(Clone)]
pub struct AuthLayer {
    validator: Arc<AwsSignatureV4Validator>,
    config: Arc<Config>,
}

impl AuthLayer {
    pub fn new(validator: Arc<AwsSignatureV4Validator>, config: Arc<Config>) -> Self {
        Self { validator, config }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthMiddleware::new(inner, self.validator.clone(), self.config.clone())
    }
}

fn parse_bucket_and_object_from_uri(uri: &hyper::Uri) -> (Option<String>, Option<String>) {
    let path = uri.path();
    
    // Remove leading slash and split by '/'
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    
    match parts.len() {
        0 => (None, None), // Root path
        1 => (Some(parts[0].to_string()), None), // Just bucket
        _ => {
            // Bucket + object path
            let bucket = parts[0].to_string();
            let object = parts[1..].join("/");
            (Some(bucket), Some(object))
        }
    }
}

fn create_error_response(status_code: StatusCode, error_code: &str, message: &str) -> Response {
    let s3_error = S3Error {
        code: error_code.to_string(),
        message: message.to_string(),
        resource: "/".to_string(),
        request_id: "".to_string(),
    };

    let error_body = quick_xml::se::to_string(&s3_error).unwrap_or_else(|_| {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>{}</Code>
    <Message>{}</Message>
    <Resource>/</Resource>
    <RequestId></RequestId>
</Error>"#,
            error_code, message
        )
    });

    Response::builder()
        .status(status_code)
        .header("Content-Type", "application/xml")
        .body(Body::from(error_body))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Method;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_auth_middleware_missing_auth_header() {
        let validator = Arc::new(AwsSignatureV4Validator::new());
        let config = Arc::new(Config {
            location: "./test_data".to_string(),
            address: "127.0.0.1".to_string(),
            port: "8333".to_string(),
            log_level: "info".to_string(),
            aws_credentials: vec![],
            encryption: None,
        });
        let layer = AuthLayer::new(validator, config);

        // Create a dummy service that just returns OK
        let service = tower::service_fn(|_req: Request| async {
            Ok::<_, std::convert::Infallible>(Response::new(Body::empty()))
        });

        let mut middleware = layer.layer(service);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let response = middleware.ready().await.unwrap().call(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
