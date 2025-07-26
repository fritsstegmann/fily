use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum PathSecurityError {
    #[error("Invalid bucket name: {0}")]
    InvalidBucketName(String),
    #[error("Invalid object name: {0}")]
    InvalidObjectName(String),
    #[error("Path traversal attempt detected: {0}")]
    PathTraversalAttempt(String),
    #[error("Invalid character in path: {0}")]
    InvalidCharacter(String),
}

/// Sanitizes and validates bucket names according to S3 naming rules and security requirements
pub fn sanitize_bucket_name(bucket: &str) -> Result<String, PathSecurityError> {
    // Check for empty bucket name
    if bucket.is_empty() {
        return Err(PathSecurityError::InvalidBucketName(
            "Bucket name cannot be empty".to_string(),
        ));
    }

    // Check length constraints (S3 requirement: 3-63 characters)
    if bucket.len() < 3 || bucket.len() > 63 {
        return Err(PathSecurityError::InvalidBucketName(format!(
            "Bucket name must be between 3 and 63 characters, got {}",
            bucket.len()
        )));
    }

    // Check for path traversal attempts
    if bucket.contains("..") || bucket.contains("/") || bucket.contains("\\") {
        return Err(PathSecurityError::PathTraversalAttempt(bucket.to_string()));
    }

    // Check for null bytes or other dangerous characters
    if bucket.contains('\0') || bucket.contains('\n') || bucket.contains('\r') {
        return Err(PathSecurityError::InvalidCharacter(
            "Bucket name contains invalid control characters".to_string(),
        ));
    }

    // S3 bucket naming rules: lowercase letters, numbers, dots, and hyphens only
    // Must start and end with lowercase letter or number
    let first_char = bucket.chars().next().unwrap();
    let last_char = bucket.chars().last().unwrap();

    if !first_char.is_ascii_lowercase() && !first_char.is_ascii_digit() {
        return Err(PathSecurityError::InvalidBucketName(
            "Bucket name must start with lowercase letter or number".to_string(),
        ));
    }

    if !last_char.is_ascii_lowercase() && !last_char.is_ascii_digit() {
        return Err(PathSecurityError::InvalidBucketName(
            "Bucket name must end with lowercase letter or number".to_string(),
        ));
    }

    // Check all characters are valid
    for ch in bucket.chars() {
        if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '.' && ch != '-' {
            return Err(PathSecurityError::InvalidBucketName(format!(
                "Bucket name contains invalid character: '{}'",
                ch
            )));
        }
    }

    // Additional S3 rules: no consecutive periods, no period-dash combinations
    if bucket.contains("..") || bucket.contains(".-") || bucket.contains("-.") {
        return Err(PathSecurityError::InvalidBucketName(
            "Bucket name cannot contain consecutive periods or period-dash combinations"
                .to_string(),
        ));
    }

    // IP address pattern check (not allowed in S3)
    if is_ip_address_pattern(bucket) {
        return Err(PathSecurityError::InvalidBucketName(
            "Bucket name cannot be formatted as an IP address".to_string(),
        ));
    }

    Ok(bucket.to_string())
}

/// Sanitizes and validates object names for security
pub fn sanitize_object_name(object: &str) -> Result<String, PathSecurityError> {
    // Check for empty object name
    if object.is_empty() {
        return Err(PathSecurityError::InvalidObjectName(
            "Object name cannot be empty".to_string(),
        ));
    }

    // Check length constraints (S3 allows up to 1024 characters)
    if object.len() > 1024 {
        return Err(PathSecurityError::InvalidObjectName(format!(
            "Object name too long: {} characters (max 1024)",
            object.len()
        )));
    }

    // Check for path traversal attempts
    if object.contains("../")
        || object.contains("..\\")
        || object.starts_with("../")
        || object.starts_with("..\\")
    {
        return Err(PathSecurityError::PathTraversalAttempt(object.to_string()));
    }

    // Check for null bytes or other dangerous characters
    if object.contains('\0') || object.contains('\n') || object.contains('\r') {
        return Err(PathSecurityError::InvalidCharacter(
            "Object name contains invalid control characters".to_string(),
        ));
    }

    // Check for leading slash (should not start with /)
    if object.starts_with('/') {
        return Err(PathSecurityError::InvalidObjectName(
            "Object name cannot start with '/'".to_string(),
        ));
    }

    // Check for Windows absolute paths (C:\, D:\, etc.)
    if object.len() >= 3 && object.chars().nth(1) == Some(':') && object.chars().nth(2) == Some('\\') {
        let first_char = object.chars().next().unwrap();
        if first_char.is_ascii_alphabetic() {
            return Err(PathSecurityError::InvalidObjectName(
                "Object name cannot be a Windows absolute path".to_string(),
            ));
        }
    }

    // Normalize path separators to forward slashes only
    let normalized = object.replace('\\', "/");

    // Remove any empty path components (e.g., "path//to/object" -> "path/to/object")
    let components: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

    // Check each component for validity
    for component in &components {
        if component == &"." || component == &".." {
            return Err(PathSecurityError::PathTraversalAttempt(object.to_string()));
        }

        // Check for dangerous characters in components
        for ch in component.chars() {
            if ch.is_control() && ch != '\t' {
                return Err(PathSecurityError::InvalidCharacter(format!(
                    "Object name component contains invalid control character: '{}'",
                    ch.escape_debug()
                )));
            }
        }
    }

    Ok(components.join("/"))
}

/// Constructs a safe file path within the storage directory
pub fn construct_safe_path(
    storage_root: &Path,
    bucket: &str,
    object: &str,
) -> Result<PathBuf, PathSecurityError> {
    // Sanitize inputs
    let safe_bucket = sanitize_bucket_name(bucket)?;
    let safe_object = sanitize_object_name(object)?;

    // Construct the path
    let mut path = storage_root.to_path_buf();
    path.push(&safe_bucket);
    path.push(&safe_object);

    // Final security check: ensure the constructed path is within storage_root
    let canonical_storage = storage_root.canonicalize().map_err(|_| {
        PathSecurityError::InvalidCharacter("Cannot canonicalize storage root".to_string())
    })?;

    // Create parent directories for the security check
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| {
            PathSecurityError::InvalidCharacter("Cannot create parent directories".to_string())
        })?;
        
        // Canonicalize only the parent directory (which we just created)
        let canonical_parent = parent.canonicalize().map_err(|e| {
            debug!("Failed to canonicalize parent directory: {:?}", e);
            PathSecurityError::PathTraversalAttempt(
                "Cannot canonicalize parent directory - possible traversal attempt".to_string(),
            )
        })?;

        // Check that the parent directory is within our storage root
        if !canonical_parent.starts_with(&canonical_storage) {
            return Err(PathSecurityError::PathTraversalAttempt(format!(
                "Constructed path escapes storage directory: {:?}",
                canonical_parent
            )));
        }
    }

    Ok(path)
}

/// Constructs a safe metadata path
pub fn construct_safe_metadata_path(
    storage_root: &Path,
    bucket: &str,
    object: &str,
) -> Result<PathBuf, PathSecurityError> {
    let safe_bucket = sanitize_bucket_name(bucket)?;
    let safe_object = sanitize_object_name(object)?;

    let mut path = storage_root.to_path_buf();
    path.push(&safe_bucket);
    path.push(".fily-metadata");

    // Create safe filename for metadata (replace path separators with underscores)
    let metadata_filename = format!("{}.json", safe_object.replace('/', "_"));
    path.push(metadata_filename);

    // Security check similar to construct_safe_path
    let canonical_storage = storage_root.canonicalize().map_err(|_| {
        PathSecurityError::InvalidCharacter("Cannot canonicalize storage root".to_string())
    })?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| {
            PathSecurityError::InvalidCharacter(
                "Cannot create metadata parent directories".to_string(),
            )
        })?;
    }

    // For metadata files, we don't need to check canonical path since the file might not exist yet
    // But we do validate that the parent directory is within our storage
    if let Some(parent) = path.parent() {
        if let Ok(canonical_parent) = parent.canonicalize() {
            if !canonical_parent.starts_with(&canonical_storage) {
                return Err(PathSecurityError::PathTraversalAttempt(format!(
                    "Metadata path escapes storage directory: {:?}",
                    canonical_parent
                )));
            }
        }
    }

    Ok(path)
}

/// Checks if a string matches an IP address pattern
fn is_ip_address_pattern(s: &str) -> bool {
    // Simple check for IPv4 pattern (x.x.x.x where x is 1-3 digits)
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }

    for part in parts {
        if part.is_empty() || part.len() > 3 {
            return false;
        }
        if !part.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        if let Ok(num) = part.parse::<u32>() {
            if num > 255 {
                return false;
            }
        } else {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sanitize_bucket_name_valid() {
        assert!(sanitize_bucket_name("my-bucket").is_ok());
        assert!(sanitize_bucket_name("bucket123").is_ok());
        assert!(sanitize_bucket_name("my.bucket.test").is_ok());
    }

    #[test]
    fn test_sanitize_bucket_name_invalid() {
        assert!(sanitize_bucket_name("").is_err());
        assert!(sanitize_bucket_name("ab").is_err()); // too short
        assert!(sanitize_bucket_name("My-Bucket").is_err()); // uppercase
        assert!(sanitize_bucket_name("bucket/path").is_err()); // path separator
        assert!(sanitize_bucket_name("../bucket").is_err()); // path traversal
        assert!(sanitize_bucket_name("192.168.1.1").is_err()); // IP address
    }

    #[test]
    fn test_sanitize_object_name_valid() {
        assert!(sanitize_object_name("file.txt").is_ok());
        assert!(sanitize_object_name("path/to/file.txt").is_ok());
        assert!(sanitize_object_name("folder/subfolder/file").is_ok());
    }

    #[test]
    fn test_sanitize_object_name_invalid() {
        assert!(sanitize_object_name("").is_err());
        assert!(sanitize_object_name("/file.txt").is_err()); // leading slash
        assert!(sanitize_object_name("../file.txt").is_err()); // path traversal
        assert!(sanitize_object_name("path/../file.txt").is_err()); // path traversal
        assert!(sanitize_object_name("file\0.txt").is_err()); // null byte
    }

    #[test]
    fn test_construct_safe_path() {
        let temp_dir = TempDir::new().unwrap();
        let storage_root = temp_dir.path();

        let result = construct_safe_path(storage_root, "test-bucket", "file.txt");
        if let Err(e) = &result {
            println!("Error: {:?}", e);
        }
        assert!(result.is_ok(), "construct_safe_path failed: {:?}", result.err());

        let path = result.unwrap();
        assert!(path.starts_with(storage_root));
        assert!(path.to_string_lossy().contains("test-bucket"));
        assert!(path.to_string_lossy().contains("file.txt"));
    }

    #[test]
    fn test_construct_safe_path_traversal_attempt() {
        let temp_dir = TempDir::new().unwrap();
        let storage_root = temp_dir.path();

        // These should fail due to path traversal attempts
        assert!(construct_safe_path(storage_root, "../etc", "passwd").is_err());
        assert!(construct_safe_path(storage_root, "bucket", "../../../etc/passwd").is_err());
    }
}
