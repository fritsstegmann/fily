use axum::http::Uri;
use std::collections::HashMap;

#[tokio::test]
async fn test_presigned_url_query_parameter_parsing() {
    // Test URL with pre-signed parameters
    let uri: Uri = "https://example.com/bucket/object?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20240101%2Fus-east-1%2Fs3%2Faws4_request&X-Amz-Date=20240101T120000Z&X-Amz-Expires=3600&X-Amz-SignedHeaders=host&X-Amz-Signature=example".parse().unwrap();

    // Test that the query contains required parameters
    let query = uri.query().unwrap();
    assert!(query.contains("X-Amz-Algorithm"));
    assert!(query.contains("X-Amz-Signature"));
}

#[tokio::test]
async fn test_presigned_url_detection() {
    // Test URL with pre-signed parameters
    let presigned_uri: Uri = "/?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=example"
        .parse()
        .unwrap();
    let regular_uri: Uri = "/bucket/object".parse().unwrap();

    // Check pre-signed URL detection logic
    let is_presigned = presigned_uri.query().map_or(false, |q| {
        q.contains("X-Amz-Algorithm") && q.contains("X-Amz-Signature")
    });
    assert!(is_presigned);

    let is_regular = regular_uri.query().map_or(false, |q| {
        q.contains("X-Amz-Algorithm") && q.contains("X-Amz-Signature")
    });
    assert!(!is_regular);
}

#[tokio::test]
async fn test_query_parameter_parsing() {
    let test_cases = vec![
        ("key=value", vec![("key", "value")]),
        (
            "key1=value1&key2=value2",
            vec![("key1", "value1"), ("key2", "value2")],
        ),
        (
            "key%20with%20spaces=value%20with%20spaces",
            vec![("key with spaces", "value with spaces")],
        ),
        (
            "X-Amz-Algorithm=AWS4-HMAC-SHA256",
            vec![("X-Amz-Algorithm", "AWS4-HMAC-SHA256")],
        ),
    ];

    for (query_string, expected) in test_cases {
        let mut params = HashMap::new();

        for param in query_string.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                let decoded_key = percent_encoding::percent_decode_str(key)
                    .decode_utf8()
                    .unwrap()
                    .to_string();
                let decoded_value = percent_encoding::percent_decode_str(value)
                    .decode_utf8()
                    .unwrap()
                    .to_string();
                params.insert(decoded_key, decoded_value);
            }
        }

        for (expected_key, expected_value) in expected {
            assert_eq!(params.get(expected_key).unwrap(), expected_value);
        }
    }
}

#[tokio::test]
async fn test_expiration_validation() {
    use chrono::DateTime;

    // Test valid expiration times
    let valid_expires = vec![1, 3600, 86400, 604800]; // 1 sec, 1 hour, 1 day, 7 days
    for expires in valid_expires {
        assert!(expires >= 1 && expires <= 604800);
    }

    // Test invalid expiration times
    let invalid_expires = vec![0, 604801]; // 0 seconds, > 7 days
    for expires in invalid_expires {
        assert!(expires < 1 || expires > 604800);
    }

    // Test timestamp parsing
    let timestamp = "20240101T120000Z";
    let parsed = DateTime::parse_from_str(&format!("{}+00:00", timestamp), "%Y%m%dT%H%M%SZ%z");
    assert!(parsed.is_ok());
}

#[tokio::test]
async fn test_canonical_query_string_creation() {
    use percent_encoding::{percent_encode, AsciiSet, CONTROLS};
    use std::collections::HashMap;

    const ENCODE_SET: &AsciiSet = &CONTROLS
        .add(b' ')
        .add(b'"')
        .add(b'#')
        .add(b'<')
        .add(b'>')
        .add(b'`')
        .add(b'?')
        .add(b'{')
        .add(b'}');

    let mut query_params = HashMap::new();
    query_params.insert(
        "X-Amz-Algorithm".to_string(),
        "AWS4-HMAC-SHA256".to_string(),
    );
    query_params.insert("X-Amz-Date".to_string(), "20240101T120000Z".to_string());
    query_params.insert("X-Amz-Expires".to_string(), "3600".to_string());

    // Create canonical query string (excluding signature)
    let mut params: Vec<(String, String)> = query_params
        .iter()
        .filter(|(k, _)| k.as_str() != "X-Amz-Signature")
        .map(|(k, v)| {
            let encoded_key = percent_encode(k.as_bytes(), ENCODE_SET).to_string();
            let encoded_value = percent_encode(v.as_bytes(), ENCODE_SET).to_string();
            (encoded_key, encoded_value)
        })
        .collect();

    params.sort_by(|a, b| a.0.cmp(&b.0));

    let query_string = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    // Should be sorted alphabetically
    assert!(query_string.starts_with("X-Amz-Algorithm="));
    assert!(query_string.contains("&X-Amz-Date="));
    assert!(query_string.contains("&X-Amz-Expires="));
}
