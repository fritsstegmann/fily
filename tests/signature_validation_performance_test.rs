use fily::fily::metadata::{save_metadata, load_metadata, ObjectMetadata};
use std::time::Instant;
use tempfile::TempDir;
use sha2::{Sha256, Digest};
use std::collections::HashMap;

#[tokio::test]
async fn test_metadata_hash_caching_performance() {
    // Create a large body to simulate performance impact
    let large_body = vec![0u8; 5 * 1024 * 1024]; // 5MB
    let body_hash = hex::encode(Sha256::digest(&large_body));
    
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path();
    let bucket = "test-bucket";
    let object = "large-file.bin";
    
    // Create metadata with cached hash
    let metadata = ObjectMetadata {
        content_type: "application/octet-stream".to_string(),
        content_length: large_body.len() as u64,
        etag: "\"test-etag\"".to_string(),
        last_modified: "Mon, 01 Jan 2024 00:00:00 GMT".to_string(),
        user_metadata: HashMap::new(),
        content_sha256: Some(body_hash.clone()),
    };
    
    // Save metadata
    save_metadata(storage_path, bucket, object, &metadata).await.unwrap();
    
    // Test performance: Loading hash from metadata vs computing from body
    let start_cached = Instant::now();
    let loaded_metadata = load_metadata(storage_path, bucket, object).await.unwrap().unwrap();
    let cached_hash = loaded_metadata.get_content_sha256().unwrap();
    let cached_duration = start_cached.elapsed();
    
    let start_computed = Instant::now();
    let computed_hash = hex::encode(Sha256::digest(&large_body));
    let computed_duration = start_computed.elapsed();
    
    // Hashes should be identical
    assert_eq!(cached_hash, &computed_hash);
    
    // Cached version should be significantly faster for large bodies
    println!("Cached hash duration: {:?}", cached_duration);
    println!("Computed hash duration: {:?}", computed_duration);
    
    if computed_duration.as_nanos() > 0 {
        println!("Performance improvement: {:.2}x faster", 
                 computed_duration.as_nanos() as f64 / cached_duration.as_nanos() as f64);
    }
    
    // Cached should be faster (though the difference might be small for file I/O)
    assert!(cached_duration <= computed_duration);
}

#[tokio::test]
async fn test_content_hash_storage_and_retrieval() {
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path();
    let bucket = "test-bucket";
    let object = "test-file.txt";
    let body_data = b"Hello, World! This is test content.";
    let expected_hash = hex::encode(Sha256::digest(body_data));
    
    // Create metadata with content hash (simulating PUT operation)
    let metadata = ObjectMetadata::with_content_sha256(
        Some("text/plain".to_string()),
        body_data.len() as u64,
        "\"test-etag\"".to_string(),
        object,
        expected_hash.clone(),
    );
    
    // Save metadata (simulating what PUT operation does)
    save_metadata(storage_path, bucket, object, &metadata).await.unwrap();
    
    // Verify that metadata can be loaded with content hash
    let loaded_metadata = load_metadata(storage_path, bucket, object).await.unwrap().unwrap();
    let stored_hash = loaded_metadata.get_content_sha256().unwrap();
    
    assert_eq!(stored_hash, &expected_hash);
    println!("Successfully stored and retrieved content hash: {}", stored_hash);
}