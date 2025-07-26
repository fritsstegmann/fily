use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use mime_guess::MimeGuess;

use super::path_security::construct_safe_metadata_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMetadata {
    pub content_type: String,
    pub content_length: u64, 
    pub etag: String,
    pub last_modified: String,
    pub user_metadata: HashMap<String, String>,
}

impl ObjectMetadata {
    pub fn new(
        content_type: Option<String>,
        content_length: u64,
        etag: String,
        file_path: &str,
    ) -> Self {
        let content_type = content_type.unwrap_or_else(|| {
            detect_content_type(file_path)
        });

        let last_modified = chrono::Utc::now()
            .format("%a, %d %b %Y %H:%M:%S GMT")
            .to_string();

        Self {
            content_type,
            content_length,
            etag,
            last_modified,
            user_metadata: HashMap::new(),
        }
    }

    pub fn add_user_metadata(&mut self, key: String, value: String) {
        self.user_metadata.insert(key, value);
    }
}

pub fn detect_content_type(file_path: &str) -> String {
    let guess = MimeGuess::from_path(file_path);
    guess
        .first()
        .map(|mime| mime.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

pub fn extract_user_metadata(headers: &hyper::HeaderMap) -> HashMap<String, String> {
    let mut user_metadata = HashMap::new();
    
    for (name, value) in headers.iter() {
        let name_str = name.as_str();
        if name_str.starts_with("x-amz-meta-") {
            if let Ok(value_str) = value.to_str() {
                let key = name_str.strip_prefix("x-amz-meta-").unwrap().to_string();
                user_metadata.insert(key, value_str.to_string());
            }
        }
    }
    
    user_metadata
}

pub async fn save_metadata(
    storage_path: &Path,
    bucket: &str,
    object: &str,
    metadata: &ObjectMetadata,
) -> anyhow::Result<()> {
    // Use secure metadata path construction to prevent path injection attacks
    let metadata_file = construct_safe_metadata_path(storage_path, bucket, object)
        .map_err(|e| anyhow::anyhow!("Metadata path security violation: {}", e))?;
    
    let metadata_json = serde_json::to_string_pretty(metadata)?;
    tokio::fs::write(metadata_file, metadata_json).await?;
    Ok(())
}

pub async fn load_metadata(
    storage_path: &Path,
    bucket: &str,
    object: &str,
) -> anyhow::Result<Option<ObjectMetadata>> {
    // Use secure metadata path construction to prevent path injection attacks
    let metadata_file = construct_safe_metadata_path(storage_path, bucket, object)
        .map_err(|e| anyhow::anyhow!("Metadata path security violation: {}", e))?;
    
    if !metadata_file.exists() {
        return Ok(None);
    }
    
    let metadata_json = tokio::fs::read_to_string(metadata_file).await?;
    let metadata: ObjectMetadata = serde_json::from_str(&metadata_json)?;
    Ok(Some(metadata))
}

pub async fn delete_metadata(
    storage_path: &Path,
    bucket: &str,
    object: &str,
) -> anyhow::Result<()> {
    // Use secure metadata path construction to prevent path injection attacks
    let metadata_file = construct_safe_metadata_path(storage_path, bucket, object)
        .map_err(|e| anyhow::anyhow!("Metadata path security violation: {}", e))?;
    
    if metadata_file.exists() {
        tokio::fs::remove_file(metadata_file).await?;
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_content_type() {
        assert_eq!(detect_content_type("test.txt"), "text/plain");
        assert_eq!(detect_content_type("test.json"), "application/json");
        assert_eq!(detect_content_type("test.jpg"), "image/jpeg");
        assert_eq!(detect_content_type("test.unknown"), "application/octet-stream");
    }

    #[test]
    fn test_extract_user_metadata() {
        let mut headers = hyper::HeaderMap::new();
        headers.insert("x-amz-meta-author", "test-user".parse().unwrap());
        headers.insert("x-amz-meta-version", "1.0".parse().unwrap());
        headers.insert("content-type", "text/plain".parse().unwrap());
        
        let user_metadata = extract_user_metadata(&headers);
        
        assert_eq!(user_metadata.get("author"), Some(&"test-user".to_string()));
        assert_eq!(user_metadata.get("version"), Some(&"1.0".to_string()));
        assert_eq!(user_metadata.len(), 2);
    }

    #[tokio::test]
    async fn test_save_and_load_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path();
        
        let mut metadata = ObjectMetadata::new(
            Some("text/plain".to_string()),
            1024,
            "\"abc123\"".to_string(),
            "test.txt",
        );
        metadata.add_user_metadata("author".to_string(), "test-user".to_string());
        
        save_metadata(storage_path, "test-bucket", "test-object", &metadata)
            .await
            .unwrap();
        
        let loaded = load_metadata(storage_path, "test-bucket", "test-object")
            .await
            .unwrap()
            .unwrap();
        
        assert_eq!(loaded.content_type, "text/plain");
        assert_eq!(loaded.content_length, 1024);
        assert_eq!(loaded.etag, "\"abc123\"");
        assert_eq!(loaded.user_metadata.get("author"), Some(&"test-user".to_string()));
    }
}