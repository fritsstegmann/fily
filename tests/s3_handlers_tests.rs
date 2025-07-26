use tempfile::TempDir;
use tokio::fs;

// Mock the S3 handler functions for testing
async fn get_object(location: &str, bucket: &str, file: &str) -> anyhow::Result<Vec<u8>> {
    let s = format!("{}/{}/{}", location, bucket, file);
    let path = std::path::Path::new(&s);
    let file_results = tokio::fs::read(&path).await?;
    Ok(file_results)
}

async fn put_object_to_disk(location: &str, bucket: &str, file: &str, content: &[u8]) -> anyhow::Result<()> {
    let s = format!("{}/{}/{}", location, bucket, file);
    let path = std::path::Path::new(&s);
    let prefix = path.parent();
    match prefix {
        Some(prefix) => {
            tokio::fs::create_dir_all(prefix).await?;
            tokio::fs::write(&path, content).await?;
            Ok(())
        }
        None => Err(anyhow::anyhow!("Could not get parent directory")),
    }
}

#[tokio::test]
async fn test_get_object_success() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    // Create test file
    let bucket = "test-bucket";
    let file = "test-file.txt";
    let test_content = b"Hello, World!";
    
    put_object_to_disk(location, bucket, file, test_content).await.unwrap();
    
    // Test getting the object
    let result = get_object(location, bucket, file).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), test_content);
}

#[tokio::test]
async fn test_get_object_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    let result = get_object(location, "nonexistent-bucket", "nonexistent-file.txt").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_put_object_success() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    let bucket = "test-bucket";
    let file = "test-file.txt";
    let test_content = b"Hello, World!";
    
    let result = put_object_to_disk(location, bucket, file, test_content).await;
    assert!(result.is_ok());
    
    // Verify file was created
    let s = format!("{}/{}/{}", location, bucket, file);
    let path = std::path::Path::new(&s);
    assert!(path.exists());
    
    let contents = fs::read(&path).await.unwrap();
    assert_eq!(contents, test_content);
}

#[tokio::test]
async fn test_put_object_creates_directory() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    let bucket = "deep/nested/bucket";
    let file = "test-file.txt";
    let test_content = b"Hello, World!";
    
    let result = put_object_to_disk(location, bucket, file, test_content).await;
    assert!(result.is_ok());
    
    // Verify directory structure was created
    let s = format!("{}/{}/{}", location, bucket, file);
    let path = std::path::Path::new(&s);
    assert!(path.exists());
    assert!(path.parent().unwrap().exists());
}

#[tokio::test]
async fn test_put_object_empty_file() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    let bucket = "test-bucket";
    let file = "empty-file.txt";
    let test_content = b"";
    
    let result = put_object_to_disk(location, bucket, file, test_content).await;
    assert!(result.is_ok());
    
    // Verify empty file was created
    let s = format!("{}/{}/{}", location, bucket, file);
    let path = std::path::Path::new(&s);
    assert!(path.exists());
    
    let contents = fs::read(&path).await.unwrap();
    assert_eq!(contents, test_content);
}

#[tokio::test]
async fn test_put_object_binary_content() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    let bucket = "test-bucket";
    let file = "binary-file.bin";
    let test_content = vec![0u8, 1, 2, 3, 255, 254, 253];
    
    let result = put_object_to_disk(location, bucket, file, &test_content).await;
    assert!(result.is_ok());
    
    // Verify binary content was written correctly
    let contents = get_object(location, bucket, file).await.unwrap();
    assert_eq!(contents, test_content);
}

#[tokio::test]
async fn test_path_construction() {
    let location = "/data";
    let bucket = "my-bucket";
    let file = "my-file.txt";
    
    let expected_path = "/data/my-bucket/my-file.txt";
    let constructed_path = format!("{}/{}/{}", location, bucket, file);
    
    assert_eq!(constructed_path, expected_path);
}

#[tokio::test]
async fn test_path_construction_with_slashes() {
    let location = "/data/";
    let bucket = "my-bucket/";
    let file = "/my-file.txt";
    
    let constructed_path = format!("{}/{}/{}", location, bucket, file);
    
    // Note: This test shows potential path traversal issues that should be addressed
    assert_eq!(constructed_path, "/data//my-bucket///my-file.txt");
}

#[tokio::test]
async fn test_get_object_large_file() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    let bucket = "test-bucket";
    let file = "large-file.txt";
    let test_content = vec![b'A'; 1024 * 1024]; // 1MB of 'A's
    
    put_object_to_disk(location, bucket, file, &test_content).await.unwrap();
    
    let result = get_object(location, bucket, file).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1024 * 1024);
}

#[tokio::test]
async fn test_bucket_and_file_names_with_special_characters() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    // Test with URL-safe special characters
    let bucket = "test-bucket-123";
    let file = "test_file-123.txt";
    let test_content = b"Special characters test";
    
    let result = put_object_to_disk(location, bucket, file, test_content).await;
    assert!(result.is_ok());
    
    let retrieved = get_object(location, bucket, file).await.unwrap();
    assert_eq!(retrieved, test_content);
}

// Test bucket creation functionality
async fn create_bucket_directory(location: &str, bucket: &str) -> anyhow::Result<()> {
    let bucket_path = format!("{}/{}", location, bucket);
    tokio::fs::create_dir_all(&bucket_path).await?;
    Ok(())
}

#[tokio::test]
async fn test_create_bucket() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    let bucket = "new-test-bucket";
    
    let result = create_bucket_directory(location, bucket).await;
    assert!(result.is_ok());
    
    let bucket_path = format!("{}/{}", location, bucket);
    let path = std::path::Path::new(&bucket_path);
    assert!(path.exists());
    assert!(path.is_dir());
}

#[tokio::test]
async fn test_create_bucket_already_exists() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    let bucket = "existing-bucket";
    
    // Create bucket first time
    let result1 = create_bucket_directory(location, bucket).await;
    assert!(result1.is_ok());
    
    // Create bucket second time (should succeed)
    let result2 = create_bucket_directory(location, bucket).await;
    assert!(result2.is_ok());
}

// Test file deletion functionality
async fn delete_object_from_disk(location: &str, bucket: &str, file: &str) -> anyhow::Result<()> {
    let s = format!("{}/{}/{}", location, bucket, file);
    let path = std::path::Path::new(&s);
    tokio::fs::remove_file(path).await?;
    Ok(())
}

#[tokio::test]
async fn test_delete_object_success() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    let bucket = "test-bucket";
    let file = "test-file.txt";
    let test_content = b"Hello, World!";
    
    // Create and then delete the file
    put_object_to_disk(location, bucket, file, test_content).await.unwrap();
    
    let delete_result = delete_object_from_disk(location, bucket, file).await;
    assert!(delete_result.is_ok());
    
    // Verify file was deleted
    let s = format!("{}/{}/{}", location, bucket, file);
    let path = std::path::Path::new(&s);
    assert!(!path.exists());
}

#[tokio::test]
async fn test_delete_object_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().to_str().unwrap();
    
    let result = delete_object_from_disk(location, "nonexistent-bucket", "nonexistent-file.txt").await;
    assert!(result.is_err());
}