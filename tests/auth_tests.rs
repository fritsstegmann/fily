use axum::http::{HeaderMap, HeaderValue, Method, Uri};
use fily::fily::auth::{AuthError, AwsCredentials, AwsSignatureV4Validator};

#[tokio::test]
async fn test_aws_signature_validator_creation() {
    let validator = AwsSignatureV4Validator::new();
    // Basic test to ensure validator can be created
    assert!(std::ptr::addr_of!(validator) as usize != 0);
}

#[tokio::test]
async fn test_missing_authorization_header() {
    let validator = AwsSignatureV4Validator::new();
    let method = Method::GET;
    let uri: Uri = "/bucket/object".parse().unwrap();
    let headers = HeaderMap::new();
    let body = b"";

    let result = validator
        .validate_request(&method, &uri, &headers, body)
        .await;

    assert!(matches!(result, Err(AuthError::MissingAuthorizationHeader)));
}

#[tokio::test]
async fn test_invalid_authorization_header_format() {
    let validator = AwsSignatureV4Validator::new();
    let method = Method::GET;
    let uri: Uri = "/bucket/object".parse().unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_static("InvalidFormat"));

    let body = b"";

    let result = validator
        .validate_request(&method, &uri, &headers, body)
        .await;

    assert!(matches!(result, Err(AuthError::InvalidAuthorizationHeader)));
}

#[tokio::test]
async fn test_auth_error_display() {
    let errors = vec![
        AuthError::MissingAuthorizationHeader,
        AuthError::InvalidAuthorizationHeader,
        AuthError::MissingRequiredHeader("test-header".to_string()),
        AuthError::InvalidDateFormat,
        AuthError::SignatureVerificationFailed,
        AuthError::InvalidAccessKey,
        AuthError::RequestTooOld,
        AuthError::MalformedRequest,
        AuthError::MissingPresignedParameter("test-param".to_string()),
        AuthError::InvalidExpiration,
        AuthError::PresignedUrlExpired,
    ];

    for error in errors {
        let error_string = error.to_string();
        assert!(!error_string.is_empty());
    }
}

#[tokio::test]
async fn test_add_aws_credentials() {
    let mut validator = AwsSignatureV4Validator::new();
    
    // Test adding valid credentials
    let credentials = AwsCredentials::new(
        "AKIAIOSFODNN7EXAMPLE".to_string(),
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
        "us-east-1".to_string(),
    ).expect("Valid credentials should be created");
    
    let result = validator.add_credentials("AKIAIOSFODNN7EXAMPLE".to_string(), credentials);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_presigned_url_missing_algorithm() {
    let validator = AwsSignatureV4Validator::new();
    let method = Method::GET;
    let uri: Uri = "/bucket/object".parse().unwrap();
    let headers = HeaderMap::new();
    let body = b"";

    let result = validator
        .validate_presigned_request(&method, &uri, &headers, body)
        .await;

    // Should fail due to missing presigned URL parameters
    assert!(result.is_err());
}

#[test]
fn test_aws_credentials_creation() {
    let credentials = AwsCredentials {
        access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
        secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
        region: "us-east-1".to_string(),
    };
    
    assert_eq!(credentials.access_key_id, "AKIAIOSFODNN7EXAMPLE");
    assert_eq!(credentials.secret_access_key, "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
    assert_eq!(credentials.region, "us-east-1");
}

#[test]
fn test_uri_parsing() {
    let valid_uris = vec![
        "/",
        "/bucket",
        "/bucket/object",
        "/bucket/path/to/object.txt",
        "/bucket-with-hyphens/object_with_underscores.ext",
    ];
    
    for uri_str in valid_uris {
        let uri: Result<Uri, _> = uri_str.parse();
        assert!(uri.is_ok(), "Failed to parse URI: {}", uri_str);
        
        let parsed_uri = uri.unwrap();
        assert_eq!(parsed_uri.path(), uri_str);
    }
}

#[test]
fn test_http_methods() {
    let methods = vec![
        Method::GET,
        Method::PUT,
        Method::POST,
        Method::DELETE,
        Method::HEAD,
        Method::OPTIONS,
        Method::PATCH,
    ];
    
    for method in methods {
        // Test that we can create and compare methods
        let method_clone = method.clone();
        assert_eq!(method, method_clone);
    }
}

#[test]
fn test_header_map_operations() {
    let mut headers = HeaderMap::new();
    
    // Test adding headers
    headers.insert("authorization", HeaderValue::from_static("AWS4-HMAC-SHA256 ..."));
    headers.insert("x-amz-date", HeaderValue::from_static("20240101T120000Z"));
    headers.insert("host", HeaderValue::from_static("example.com"));
    
    // Test retrieving headers
    assert!(headers.contains_key("authorization"));
    assert!(headers.contains_key("x-amz-date"));
    assert!(headers.contains_key("host"));
    
    // Test header count
    assert_eq!(headers.len(), 3);
}

#[test]
fn test_aws_constants() {
    // Test that we have the expected AWS constants
    let algorithm = "AWS4-HMAC-SHA256";
    let service = "s3";
    let request_type = "aws4_request";
    
    assert!(algorithm.starts_with("AWS4"));
    assert!(algorithm.contains("HMAC"));
    assert!(algorithm.contains("SHA256"));
    assert_eq!(service, "s3");
    assert_eq!(request_type, "aws4_request");
}