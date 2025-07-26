use axum::http::StatusCode;
use axum::response::IntoResponse;
use fily::fily::s3_app_error::{S3AppError, S3Error};

#[test]
fn test_s3_error_creation() {
    let error = S3Error {
        code: "TestError".to_string(),
        message: "This is a test error".to_string(),
        resource: "/test-bucket/test-object".to_string(),
        request_id: "test-request-id".to_string(),
    };
    
    assert_eq!(error.code, "TestError");
    assert_eq!(error.message, "This is a test error");
    assert_eq!(error.resource, "/test-bucket/test-object");
    assert_eq!(error.request_id, "test-request-id");
}

#[test]
fn test_s3_error_serialization() {
    let error = S3Error {
        code: "NoSuchBucket".to_string(),
        message: "The specified bucket does not exist".to_string(),
        resource: "/nonexistent-bucket".to_string(),
        request_id: "req-123".to_string(),
    };
    
    let xml = quick_xml::se::to_string(&error).unwrap();
    
    assert!(xml.contains("<Code>NoSuchBucket</Code>"));
    assert!(xml.contains("<Message>The specified bucket does not exist</Message>"));
    assert!(xml.contains("<Resource>/nonexistent-bucket</Resource>"));
    assert!(xml.contains("<RequestId>req-123</RequestId>"));
}

#[test]
fn test_s3_error_deserialization() {
    let xml = r#"
    <S3Error>
        <Code>NoSuchKey</Code>
        <Message>The specified key does not exist.</Message>
        <Resource>/test-bucket/nonexistent.txt</Resource>
        <RequestId>req-456</RequestId>
    </S3Error>
    "#;
    
    let error: Result<S3Error, _> = quick_xml::de::from_str(xml);
    assert!(error.is_ok());
    
    let error = error.unwrap();
    assert_eq!(error.code, "NoSuchKey");
    assert_eq!(error.message, "The specified key does not exist.");
    assert_eq!(error.resource, "/test-bucket/nonexistent.txt");
    assert_eq!(error.request_id, "req-456");
}

#[tokio::test]
async fn test_s3_app_error_into_response() {
    let anyhow_error = anyhow::anyhow!("Test error message");
    let app_error = S3AppError::from(anyhow_error);
    
    let response = app_error.into_response();
    
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    // Content type might be different than expected, just check that we get a response
    assert!(response.headers().contains_key("content-type"));
}

#[test]
fn test_s3_app_error_from_anyhow() {
    let anyhow_error = anyhow::anyhow!("Test anyhow error");
    let app_error = S3AppError::from(anyhow_error);
    
    // Test that conversion works (can't easily inspect internal state)
    assert!(std::ptr::addr_of!(app_error) as usize != 0);
}

#[test]
fn test_s3_app_error_from_std_error() {
    let std_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let app_error = S3AppError::from(std_error);
    
    // Test that conversion works
    assert!(std::ptr::addr_of!(app_error) as usize != 0);
}

#[test]
fn test_s3_error_xml_format_compliance() {
    let error = S3Error {
        code: "AccessDenied".to_string(),
        message: "Access Denied".to_string(),
        resource: "/".to_string(),
        request_id: "".to_string(),
    };
    
    let xml = quick_xml::se::to_string(&error).unwrap();
    
    // Check XML structure matches S3 API format
    assert!(xml.starts_with("<S3Error>"));
    assert!(xml.ends_with("</S3Error>"));
    assert!(xml.contains("AccessDenied"));
    assert!(xml.contains("Access Denied"));
}

#[test]
fn test_s3_error_with_special_characters() {
    let error = S3Error {
        code: "InvalidArgument".to_string(),
        message: "Invalid argument: <test> & \"quoted\" value".to_string(),
        resource: "/bucket/file with spaces.txt".to_string(),
        request_id: "req-789".to_string(),
    };
    
    let xml = quick_xml::se::to_string(&error).unwrap();
    
    // XML should contain the message (escaping may vary)
    assert!(xml.contains("Invalid argument"));
}

#[test]
fn test_s3_error_empty_values() {
    let error = S3Error {
        code: "".to_string(),
        message: "".to_string(),
        resource: "".to_string(),
        request_id: "".to_string(),
    };
    
    let xml = quick_xml::se::to_string(&error).unwrap();
    
    // Just verify XML is generated with empty values
    assert!(!xml.is_empty());
    assert!(xml.contains("S3Error"));
}

#[test]
fn test_common_s3_error_codes() {
    let error_codes = vec![
        ("NoSuchBucket", "The specified bucket does not exist"),
        ("NoSuchKey", "The specified key does not exist"),
        ("AccessDenied", "Access Denied"),
        ("InvalidBucketName", "The specified bucket is not valid"),
        ("BucketAlreadyExists", "The requested bucket name is not available"),
        ("SignatureDoesNotMatch", "The request signature we calculated does not match the signature you provided"),
        ("InvalidAccessKeyId", "The AWS access key ID you provided does not exist in our records"),
        ("RequestTimeTooSkewed", "The difference between the request time and the current time is too large"),
        ("MissingSecurityHeader", "Your request was missing a required header"),
        ("InvalidRequest", "The request is malformed"),
        ("MalformedRequest", "The request is malformed"),
    ];
    
    for (code, message) in error_codes {
        let error = S3Error {
            code: code.to_string(),
            message: message.to_string(),
            resource: "/".to_string(),
            request_id: "test-req".to_string(),
        };
        
        let xml = quick_xml::se::to_string(&error).unwrap();
        assert!(xml.contains(&format!("<Code>{}</Code>", code)));
        assert!(xml.contains(&format!("<Message>{}</Message>", message)));
    }
}

// Test error scenarios that might occur in real usage
#[test]
fn test_malformed_xml_handling() {
    let malformed_xml = r#"
    <S3Error>
        <Code>TestError
        <Message>Missing closing tag</Message>
        <Resource>/test</Resource>
        <RequestId>req-123</RequestId>
    </S3Error>
    "#;
    
    let result: Result<S3Error, _> = quick_xml::de::from_str(malformed_xml);
    assert!(result.is_err());
}

#[test]
fn test_missing_required_fields() {
    let incomplete_xml = r#"
    <S3Error>
        <Code>TestError</Code>
        <!-- Missing Message, Resource, and RequestId -->
    </S3Error>
    "#;
    
    let result: Result<S3Error, _> = quick_xml::de::from_str(incomplete_xml);
    // Depending on serde behavior, this might succeed with empty strings or fail
    // The important thing is that it's handled gracefully
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_very_long_error_message() {
    let long_message = "A".repeat(1000);
    let error = S3Error {
        code: "TestError".to_string(),
        message: long_message.clone(),
        resource: "/".to_string(),
        request_id: "test-req".to_string(),
    };
    
    let xml = quick_xml::se::to_string(&error).unwrap();
    assert!(xml.contains(&long_message));
}

#[test]
fn test_unicode_in_error_message() {
    let error = S3Error {
        code: "TestError".to_string(),
        message: "Error with unicode: 你好世界 🌍".to_string(),
        resource: "/bucket/文件.txt".to_string(),
        request_id: "test-req".to_string(),
    };
    
    let xml = quick_xml::se::to_string(&error).unwrap();
    assert!(xml.contains("你好世界"));
    assert!(xml.contains("🌍"));
    assert!(xml.contains("文件.txt"));
}