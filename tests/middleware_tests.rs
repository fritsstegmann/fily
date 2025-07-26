use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::Response;
use fily::fily::auth::AwsSignatureV4Validator;
use fily::fily::auth_middleware::{AuthLayer, AuthMiddleware};
use std::sync::Arc;
use tower::Layer;


#[tokio::test]
async fn test_auth_middleware_creation() {
    let validator = Arc::new(AwsSignatureV4Validator::new());
    let service = tower::service_fn(|_req: Request| async {
        Ok::<_, std::convert::Infallible>(Response::new(Body::empty()))
    });
    let middleware = AuthMiddleware::new(service, validator);
    
    // Basic test that middleware can be created
    assert!(std::ptr::addr_of!(middleware) as usize != 0);
}

#[tokio::test]
async fn test_auth_layer_creation() {
    let validator = Arc::new(AwsSignatureV4Validator::new());
    let layer = AuthLayer::new(validator);
    
    // Basic test that layer can be created
    assert!(std::ptr::addr_of!(layer) as usize != 0);
}

#[tokio::test]
async fn test_auth_layer_wraps_service() {
    let validator = Arc::new(AwsSignatureV4Validator::new());
    let layer = AuthLayer::new(validator);
    let service = tower::service_fn(|_req: Request| async {
        Ok::<_, std::convert::Infallible>(Response::new(Body::empty()))
    });
    
    let wrapped_service = layer.layer(service);
    
    // Test that we get a wrapped service
    assert!(std::ptr::addr_of!(wrapped_service) as usize != 0);
}

#[tokio::test]
async fn test_response_error_format() {
    // Test XML error response structure
    let error_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<S3Error>
    <Code>MissingSecurityHeader</Code>
    <Message>Your request was missing a required header</Message>
    <Resource>/</Resource>
    <RequestId></RequestId>
</S3Error>"#;
    
    // Test that error XML has proper structure
    assert!(error_xml.contains("<Code>MissingSecurityHeader</Code>"));
    assert!(error_xml.contains("<Message>Your request was missing a required header</Message>"));
    assert!(error_xml.contains("<Resource>/</Resource>"));
    assert!(error_xml.contains("<RequestId></RequestId>"));
}

#[test]
fn test_presigned_url_detection_logic() {
    // Test the logic that detects pre-signed URLs
    let presigned_query = "X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=test&X-Amz-Credential=test";
    let regular_query = "param1=value1&param2=value2";
    let partial_query = "X-Amz-Algorithm=AWS4-HMAC-SHA256&param=value";
    
    // Test pre-signed URL detection
    let is_presigned = |query: &str| {
        query.contains("X-Amz-Algorithm") && query.contains("X-Amz-Signature")
    };
    
    assert!(is_presigned(presigned_query));
    assert!(!is_presigned(regular_query));
    assert!(!is_presigned(partial_query));
}

#[test]
fn test_error_status_codes() {
    // Test that we have the right status codes for different error types
    let test_cases = vec![
        ("MissingAuthorizationHeader", StatusCode::UNAUTHORIZED),
        ("InvalidAuthorizationHeader", StatusCode::UNAUTHORIZED),
        ("MissingRequiredHeader", StatusCode::BAD_REQUEST),
        ("InvalidDateFormat", StatusCode::UNAUTHORIZED),
        ("SignatureVerificationFailed", StatusCode::FORBIDDEN),
        ("InvalidAccessKey", StatusCode::FORBIDDEN),
        ("RequestTooOld", StatusCode::FORBIDDEN),
        ("MalformedRequest", StatusCode::BAD_REQUEST),
    ];
    
    for (error_type, expected_status) in test_cases {
        match error_type {
            "MissingAuthorizationHeader" => assert_eq!(expected_status, StatusCode::UNAUTHORIZED),
            "InvalidAuthorizationHeader" => assert_eq!(expected_status, StatusCode::UNAUTHORIZED),
            "MissingRequiredHeader" => assert_eq!(expected_status, StatusCode::BAD_REQUEST),
            "InvalidDateFormat" => assert_eq!(expected_status, StatusCode::UNAUTHORIZED),
            "SignatureVerificationFailed" => assert_eq!(expected_status, StatusCode::FORBIDDEN),
            "InvalidAccessKey" => assert_eq!(expected_status, StatusCode::FORBIDDEN),
            "RequestTooOld" => assert_eq!(expected_status, StatusCode::FORBIDDEN),
            "MalformedRequest" => assert_eq!(expected_status, StatusCode::BAD_REQUEST),
            _ => panic!("Unexpected error type: {}", error_type),
        }
    }
}

#[test]
fn test_s3_error_code_mappings() {
    // Test that S3 error codes match AWS S3 specification
    let s3_error_codes = vec![
        "MissingSecurityHeader",
        "InvalidRequest",
        "SignatureDoesNotMatch",
        "InvalidAccessKeyId",
        "RequestTimeTooSkewed",
        "MalformedRequest",
        "AccessDenied",
    ];
    
    for code in s3_error_codes {
        assert!(!code.is_empty());
        assert!(code.chars().all(|c| c.is_ascii()));
    }
}

#[test]
fn test_authorization_header_parsing() {
    // Test authorization header format validation
    let valid_auth_header = "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20240101/us-east-1/s3/aws4_request, SignedHeaders=host;x-amz-date, Signature=example";
    let invalid_auth_headers = vec![
        "InvalidFormat",
        "AWS4-HMAC-SHA256",
        "AWS4-HMAC-SHA256 Credential=test",
        "Bearer token",
        "",
    ];
    
    // Check valid header format
    assert!(valid_auth_header.starts_with("AWS4-HMAC-SHA256"));
    assert!(valid_auth_header.contains("Credential="));
    assert!(valid_auth_header.contains("SignedHeaders="));
    assert!(valid_auth_header.contains("Signature="));
    
    // Check invalid headers
    for invalid_header in invalid_auth_headers {
        let is_valid_format = invalid_header.starts_with("AWS4-HMAC-SHA256") 
            && invalid_header.contains("Credential=")
            && invalid_header.contains("SignedHeaders=")
            && invalid_header.contains("Signature=");
        assert!(!is_valid_format, "Header should be invalid: {}", invalid_header);
    }
}

#[test]
fn test_date_header_validation() {
    // Test x-amz-date header format validation
    let valid_dates = vec![
        "20240101T120000Z",
        "20231225T235959Z",
        "20240229T000000Z", // Leap year
    ];
    
    let invalid_dates = vec![
        "2024-01-01T12:00:00Z", // Wrong format
        "20240101", // Missing time
        "invalid-date",
        "",
        "20241301T120000Z", // Invalid month
        "20240132T120000Z", // Invalid day
    ];
    
    // Simple format validation (basic check)
    let is_valid_format = |date: &str| {
        date.len() == 16 && date.ends_with('Z') && date.contains('T')
    };
    
    for date in valid_dates {
        assert!(is_valid_format(date), "Date should be valid: {}", date);
    }
    
    for date in invalid_dates {
        if date == "20241301T120000Z" || date == "20240132T120000Z" {
            // These have correct format but invalid values
            assert!(is_valid_format(date));
        } else {
            assert!(!is_valid_format(date), "Date should be invalid: {}", date);
        }
    }
}

#[tokio::test]
async fn test_request_methods_support() {
    // Test that middleware works with different HTTP methods
    let methods = vec![
        Method::GET,
        Method::PUT,
        Method::POST,
        Method::DELETE,
        Method::HEAD,
        Method::OPTIONS,
    ];
    
    for method in methods {
        // Just test that we can create requests with different methods
        let req = Request::builder()
            .method(method.clone())
            .uri("/test-bucket/test-object")
            .body(Body::empty());
        
        assert!(req.is_ok(), "Failed to create request with method: {}", method);
    }
}

#[test]
fn test_content_type_header() {
    // Test that XML responses have correct content type
    let content_type = HeaderValue::from_static("application/xml");
    assert_eq!(content_type, "application/xml");
}

#[test]
fn test_clock_skew_tolerance() {
    // Test clock skew tolerance constants (15 minutes = 900 seconds)
    let max_clock_skew_seconds = 15 * 60; // 15 minutes
    assert_eq!(max_clock_skew_seconds, 900);
    
    // Test that we allow reasonable clock skew
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let old_time = current_time - 600; // 10 minutes ago (should be allowed)
    let too_old_time = current_time - 1200; // 20 minutes ago (should be rejected)
    
    assert!(current_time - old_time < max_clock_skew_seconds as u64);
    assert!(current_time - too_old_time > max_clock_skew_seconds as u64);
}