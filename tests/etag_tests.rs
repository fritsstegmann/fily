use fily::fily::etag::generate_etag;

#[tokio::test]
async fn test_etag_generation_consistency() {
    let test_data = b"Hello, S3 world!";
    
    // Test that e-tag generation is consistent
    let etag1 = generate_etag(test_data);
    let etag2 = generate_etag(test_data);
    
    assert_eq!(etag1, etag2);
    assert!(etag1.starts_with('"'));
    assert!(etag1.ends_with('"'));
    
    // Test with different data produces different e-tags
    let different_data = b"Different content";
    let etag3 = generate_etag(different_data);
    
    assert_ne!(etag1, etag3);
}

#[tokio::test]
async fn test_etag_format() {
    let test_data = b"test";
    let etag = generate_etag(test_data);
    
    // E-tag should be quoted MD5 hash
    assert!(etag.starts_with('"'));
    assert!(etag.ends_with('"'));
    
    // Remove quotes and check if it's a valid hex string
    let hex_part = &etag[1..etag.len()-1];
    assert_eq!(hex_part.len(), 32); // MD5 is 32 hex characters
    assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn test_known_etag_values() {
    // Test known MD5 values
    let empty_data = b"";
    let empty_etag = generate_etag(empty_data);
    assert_eq!(empty_etag, "\"d41d8cd98f00b204e9800998ecf8427e\"");
    
    let hello_world = b"Hello, world!";
    let hello_etag = generate_etag(hello_world);
    assert_eq!(hello_etag, "\"6cd3556deb0da54bca060b4c39479839\"");
}