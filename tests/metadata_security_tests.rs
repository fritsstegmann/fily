use fily::fily::metadata::{save_metadata, load_metadata, delete_metadata, ObjectMetadata};
use std::collections::HashMap;
use tempfile::TempDir;

#[tokio::test]
async fn test_metadata_path_injection_protection() {
    let temp_dir = TempDir::new().unwrap();
    let storage_root = temp_dir.path();

    let mut user_metadata = HashMap::new();
    user_metadata.insert("test-key".to_string(), "test-value".to_string());

    let metadata = ObjectMetadata {
        content_type: "text/plain".to_string(),
        content_length: 100,
        etag: "\"abc123\"".to_string(),
        last_modified: "Mon, 01 Jan 2024 00:00:00 GMT".to_string(),
        user_metadata,
        content_sha256: Some("abc123def456".to_string()),
    };

    // Test that path traversal attempts in object names are rejected
    let malicious_object_names = vec![
        "../../../etc/passwd",
        "..\\..\\..\\windows\\system32\\config\\sam",
        "test/../../../etc/passwd",
        "test\\..\\..\\..\\etc\\passwd",
        "..\0passwd",
        "test\0file.txt",
        "/etc/passwd",
        "C:\\Windows\\System32\\config\\sam",
    ];

    for malicious_name in malicious_object_names {
        println!("Testing malicious object name: {}", malicious_name);
        
        // All of these should fail with security violations
        let save_result = save_metadata(storage_root, "test-bucket", malicious_name, &metadata).await;
        assert!(save_result.is_err(), "save_metadata should reject malicious name: {}", malicious_name);
        
        let load_result = load_metadata(storage_root, "test-bucket", malicious_name).await;
        assert!(load_result.is_err(), "load_metadata should reject malicious name: {}", malicious_name);
        
        let delete_result = delete_metadata(storage_root, "test-bucket", malicious_name).await;
        assert!(delete_result.is_err(), "delete_metadata should reject malicious name: {}", malicious_name);
    }
}

#[tokio::test]
async fn test_metadata_bucket_name_injection_protection() {
    let temp_dir = TempDir::new().unwrap();
    let storage_root = temp_dir.path();

    let mut user_metadata = HashMap::new();
    user_metadata.insert("test-key".to_string(), "test-value".to_string());

    let metadata = ObjectMetadata {
        content_type: "text/plain".to_string(),
        content_length: 100,
        etag: "\"abc123\"".to_string(),
        last_modified: "Mon, 01 Jan 2024 00:00:00 GMT".to_string(),
        user_metadata,
        content_sha256: Some("abc123def456".to_string()),
    };

    // Test that path traversal attempts in bucket names are rejected
    let malicious_bucket_names = vec![
        "../../../etc",
        "..\\..\\..\\windows",
        "/etc",
        "C:\\Windows",
        "test/../etc",
        "bucket\0name",
        "",  // empty bucket name
        "ab", // too short
    ];

    for malicious_bucket in malicious_bucket_names {
        println!("Testing malicious bucket name: {}", malicious_bucket);
        
        // All of these should fail with security violations
        let save_result = save_metadata(storage_root, malicious_bucket, "test.txt", &metadata).await;
        assert!(save_result.is_err(), "save_metadata should reject malicious bucket: {}", malicious_bucket);
        
        let load_result = load_metadata(storage_root, malicious_bucket, "test.txt").await;
        assert!(load_result.is_err(), "load_metadata should reject malicious bucket: {}", malicious_bucket);
        
        let delete_result = delete_metadata(storage_root, malicious_bucket, "test.txt").await;
        assert!(delete_result.is_err(), "delete_metadata should reject malicious bucket: {}", malicious_bucket);
    }
}

#[tokio::test]
async fn test_metadata_secure_path_construction() {
    let temp_dir = TempDir::new().unwrap();
    let storage_root = temp_dir.path();

    let mut user_metadata = HashMap::new();
    user_metadata.insert("author".to_string(), "test-user".to_string());

    let metadata = ObjectMetadata {
        content_type: "application/json".to_string(),
        content_length: 256,
        etag: "\"def456\"".to_string(),
        last_modified: "Tue, 02 Jan 2024 12:00:00 GMT".to_string(),
        user_metadata,
        content_sha256: Some("def456abc123".to_string()),
    };

    // Test that valid names work correctly
    let valid_combinations = vec![
        ("test-bucket", "file.txt"),
        ("my-bucket-123", "path/to/file.json"),
        ("bucket.with.dots", "deep/nested/path/document.pdf"),
        ("test123", "file-with-dashes.txt"),
    ];

    for (bucket, object) in valid_combinations {
        println!("Testing valid combination: bucket='{}', object='{}'", bucket, object);
        
        // Save metadata
        let save_result = save_metadata(storage_root, bucket, object, &metadata).await;
        assert!(save_result.is_ok(), "save_metadata should succeed for valid names: {}/{}", bucket, object);
        
        // Load metadata
        let load_result = load_metadata(storage_root, bucket, object).await;
        assert!(load_result.is_ok(), "load_metadata should succeed for valid names: {}/{}", bucket, object);
        
        let loaded_metadata = load_result.unwrap().unwrap();
        assert_eq!(loaded_metadata.content_type, metadata.content_type);
        assert_eq!(loaded_metadata.etag, metadata.etag);
        
        // Delete metadata
        let delete_result = delete_metadata(storage_root, bucket, object).await;
        assert!(delete_result.is_ok(), "delete_metadata should succeed for valid names: {}/{}", bucket, object);
        
        // Verify deletion
        let load_after_delete = load_metadata(storage_root, bucket, object).await;
        assert!(load_after_delete.is_ok());
        assert!(load_after_delete.unwrap().is_none(), "Metadata should be None after deletion");
    }
}

#[tokio::test]
async fn test_metadata_file_isolation() {
    let temp_dir = TempDir::new().unwrap();
    let storage_root = temp_dir.path();

    let mut user_metadata = HashMap::new();
    user_metadata.insert("test".to_string(), "isolation".to_string());

    let metadata = ObjectMetadata {
        content_type: "text/plain".to_string(),
        content_length: 50,
        etag: "\"ghi789\"".to_string(),
        last_modified: "Wed, 03 Jan 2024 18:30:00 GMT".to_string(),
        user_metadata,
        content_sha256: Some("ghi789abc123".to_string()),
    };

    // Create metadata for a legitimate file
    let result = save_metadata(storage_root, "test-bucket", "legitimate-file.txt", &metadata).await;
    assert!(result.is_ok());

    // Verify that metadata files are created in the expected secure location
    let expected_metadata_dir = storage_root.join("test-bucket").join(".fily-metadata");
    assert!(expected_metadata_dir.exists(), "Metadata directory should exist");
    
    let expected_metadata_file = expected_metadata_dir.join("legitimate-file.txt.json");
    assert!(expected_metadata_file.exists(), "Metadata file should exist in secure location");

    // Verify that the metadata file cannot be accessed through path traversal
    // The secure implementation should prevent this, but let's verify
    let sensitive_paths = vec![
        storage_root.join("legitimate-file.txt.json"), // Direct access attempt
        storage_root.parent().unwrap().join("legitimate-file.txt.json"), // Parent directory attempt
    ];

    for sensitive_path in sensitive_paths {
        assert!(!sensitive_path.exists(), "Metadata should not be accessible at: {:?}", sensitive_path);
    }
}