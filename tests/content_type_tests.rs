use tempfile::TempDir;
use hyper::HeaderMap;

use fily::fily::metadata::{ObjectMetadata, detect_content_type, extract_user_metadata, save_metadata, load_metadata};

#[tokio::test]
async fn test_content_type_detection() {
    // Test various file extensions
    assert_eq!(detect_content_type("test.txt"), "text/plain");
    assert_eq!(detect_content_type("test.json"), "application/json");
    assert_eq!(detect_content_type("test.jpg"), "image/jpeg");
    assert_eq!(detect_content_type("test.png"), "image/png");
    assert_eq!(detect_content_type("test.html"), "text/html");
    assert_eq!(detect_content_type("test.css"), "text/css");
    assert_eq!(detect_content_type("test.js"), "text/javascript");
    assert_eq!(detect_content_type("test.pdf"), "application/pdf");
    assert_eq!(detect_content_type("unknown_file"), "application/octet-stream");
}

#[tokio::test]
async fn test_user_metadata_extraction() {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", "text/plain".parse().unwrap());
    headers.insert("x-amz-meta-author", "test-user".parse().unwrap());
    headers.insert("x-amz-meta-version", "1.0".parse().unwrap());
    headers.insert("x-amz-meta-custom-field", "custom-value".parse().unwrap());
    headers.insert("authorization", "AWS4-HMAC-SHA256 test".parse().unwrap());
    
    let user_metadata = extract_user_metadata(&headers);
    
    assert_eq!(user_metadata.len(), 3);
    assert_eq!(user_metadata.get("author"), Some(&"test-user".to_string()));
    assert_eq!(user_metadata.get("version"), Some(&"1.0".to_string()));
    assert_eq!(user_metadata.get("custom-field"), Some(&"custom-value".to_string()));
    
    // Ensure non-metadata headers are not included
    assert!(!user_metadata.contains_key("content-type"));
    assert!(!user_metadata.contains_key("authorization"));
}

#[tokio::test]
async fn test_metadata_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path();
    
    // Create metadata with content-type and user metadata
    let mut metadata = ObjectMetadata::new(
        Some("application/json".to_string()),
        2048,
        "\"abc123def\"".to_string(),
        "data.json",
    );
    metadata.add_user_metadata("creator".to_string(), "test-system".to_string());
    metadata.add_user_metadata("purpose".to_string(), "testing".to_string());
    
    // Save metadata
    save_metadata(storage_path, "test-bucket", "path/to/data.json", &metadata)
        .await
        .unwrap();
    
    // Load metadata and verify
    let loaded = load_metadata(storage_path, "test-bucket", "path/to/data.json")
        .await
        .unwrap()
        .unwrap();
    
    assert_eq!(loaded.content_type, "application/json");
    assert_eq!(loaded.content_length, 2048);
    assert_eq!(loaded.etag, "\"abc123def\"");
    assert_eq!(loaded.user_metadata.len(), 2);
    assert_eq!(loaded.user_metadata.get("creator"), Some(&"test-system".to_string()));
    assert_eq!(loaded.user_metadata.get("purpose"), Some(&"testing".to_string()));
    
    // Verify the metadata file structure
    let metadata_file = storage_path
        .join("test-bucket")
        .join(".fily-metadata")
        .join("path_to_data.json.json");
    assert!(metadata_file.exists());
}

#[tokio::test]
async fn test_metadata_fallback_behavior() {
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path();
    
    // Try to load metadata for non-existent object
    let result = load_metadata(storage_path, "nonexistent-bucket", "nonexistent-object").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_content_type_without_explicit_type() {
    // Test ObjectMetadata creation without explicit content-type
    let metadata = ObjectMetadata::new(
        None,
        1024,
        "\"test123\"".to_string(),
        "document.pdf",
    );
    
    // Should detect PDF content type
    assert_eq!(metadata.content_type, "application/pdf");
    
    // Test with unknown extension
    let metadata2 = ObjectMetadata::new(
        None,
        512,
        "\"test456\"".to_string(),
        "unknown.unknown_extension_12345",
    );
    
    // Should fallback to octet-stream
    assert_eq!(metadata2.content_type, "application/octet-stream");
}

#[tokio::test]
async fn test_content_type_override() {
    // Test that explicit content-type overrides file extension detection
    let metadata = ObjectMetadata::new(
        Some("text/custom".to_string()),
        1024,
        "\"test123\"".to_string(),
        "document.pdf", // Extension suggests PDF
    );
    
    // Should use explicit content-type, not detected
    assert_eq!(metadata.content_type, "text/custom");
}

#[test]
fn test_special_characters_in_metadata() {
    let mut headers = HeaderMap::new();
    // Use ASCII-safe characters that are valid in HTTP headers
    headers.insert("x-amz-meta-project", "test-project-2024".parse().unwrap());
    headers.insert("x-amz-meta-symbols", "test_value_123".parse().unwrap());
    
    let user_metadata = extract_user_metadata(&headers);
    
    assert_eq!(user_metadata.get("project"), Some(&"test-project-2024".to_string()));
    assert_eq!(user_metadata.get("symbols"), Some(&"test_value_123".to_string()));
}