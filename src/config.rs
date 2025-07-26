use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use serde::Deserialize;
use std::env;

use crate::fily::{AwsCredentialConfig, Config, EncryptionConfig};

/// Environment variable configuration loader
/// Supports multiple AWS credentials via indexed environment variables
/// and JSON-based credential configuration
pub struct ConfigLoader;

#[derive(Deserialize)]
struct AwsCredentialsJson {
    credentials: Vec<AwsCredentialConfig>,
}

impl ConfigLoader {
    /// Load configuration from environment variables with fallback defaults
    pub fn load() -> Result<Config> {
        let location = env::var("FILY_LOCATION").unwrap_or_else(|_| "./data".to_string());
        let port = env::var("FILY_PORT").unwrap_or_else(|_| "8333".to_string());
        let address = env::var("FILY_ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_string());
        let log_level = env::var("FILY_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

        // Load AWS credentials (multiple methods supported)
        let aws_credentials = Self::load_aws_credentials()?;

        // Load encryption configuration
        let encryption = Self::load_encryption_config()?;

        Ok(Config {
            location,
            port,
            address,
            log_level,
            aws_credentials,
            encryption,
        })
    }

    /// Load AWS credentials from environment variables
    /// Supports multiple methods:
    /// 1. JSON format via FILY_AWS_CREDENTIALS
    /// 2. Indexed environment variables (FILY_AWS_ACCESS_KEY_ID_0, etc.)
    /// 3. Single credential via standard AWS env vars (AWS_ACCESS_KEY_ID, etc.)
    fn load_aws_credentials() -> Result<Vec<AwsCredentialConfig>> {
        let mut credentials = Vec::new();

        // Method 1: JSON format
        if let Ok(json_creds) = env::var("FILY_AWS_CREDENTIALS") {
            let parsed: AwsCredentialsJson = serde_json::from_str(&json_creds)
                .map_err(|e| anyhow!("Invalid FILY_AWS_CREDENTIALS JSON format: {}", e))?;
            credentials.extend(parsed.credentials);
        }

        // Method 2: Indexed environment variables
        let mut index = 0;
        loop {
            let access_key_var = format!("FILY_AWS_ACCESS_KEY_ID_{}", index);
            let secret_key_var = format!("FILY_AWS_SECRET_ACCESS_KEY_{}", index);
            let region_var = format!("FILY_AWS_REGION_{}", index);

            if let (Ok(access_key), Ok(secret_key), Ok(region)) = (
                env::var(&access_key_var),
                env::var(&secret_key_var),
                env::var(&region_var),
            ) {
                credentials.push(AwsCredentialConfig {
                    access_key_id: access_key,
                    secret_access_key: secret_key,
                    region,
                });
                index += 1;
            } else {
                break;
            }
        }

        // Method 3: Standard AWS environment variables (fallback)
        if credentials.is_empty() {
            if let (Ok(access_key), Ok(secret_key)) = (
                env::var("AWS_ACCESS_KEY_ID"),
                env::var("AWS_SECRET_ACCESS_KEY"),
            ) {
                let region = env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
                credentials.push(AwsCredentialConfig {
                    access_key_id: access_key,
                    secret_access_key: secret_key,
                    region,
                });
            }
        }

        // Method 4: Fily-specific environment variables (fallback)
        if credentials.is_empty() {
            if let (Ok(access_key), Ok(secret_key)) = (
                env::var("FILY_AWS_ACCESS_KEY_ID"),
                env::var("FILY_AWS_SECRET_ACCESS_KEY"),
            ) {
                let region =
                    env::var("FILY_AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
                credentials.push(AwsCredentialConfig {
                    access_key_id: access_key,
                    secret_access_key: secret_key,
                    region,
                });
            }
        }

        Ok(credentials)
    }

    /// Load encryption configuration from environment variables
    fn load_encryption_config() -> Result<Option<EncryptionConfig>> {
        let enabled = env::var("FILY_ENCRYPTION_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        if !enabled {
            return Ok(None);
        }

        let master_key = env::var("FILY_ENCRYPTION_MASTER_KEY").ok();

        Ok(Some(EncryptionConfig {
            enabled,
            master_key,
        }))
    }

    /// Print configuration help
    pub fn print_help() {
        println!("Fily Configuration - Environment Variables");
        println!("=========================================");
        println!();
        println!("Core Configuration:");
        println!("  FILY_LOCATION              Storage directory (default: ./data)");
        println!("  FILY_PORT                  Server port (default: 8333)");
        println!("  FILY_ADDRESS               Bind address (default: 0.0.0.0)");
        println!("  FILY_LOG_LEVEL             Log level (default: info)");
        println!();
        println!("AWS Credentials (Multiple Methods Supported):");
        println!();
        println!("Method 1 - JSON Format:");
        println!("  FILY_AWS_CREDENTIALS       JSON array of credentials");
        println!("  Example: '[{{\"access_key_id\":\"key1\",\"secret_access_key\":\"secret1\",\"region\":\"us-east-1\"}}]'");
        println!();
        println!("Method 2 - Indexed Variables:");
        println!("  FILY_AWS_ACCESS_KEY_ID_0    First access key");
        println!("  FILY_AWS_SECRET_ACCESS_KEY_0 First secret key");
        println!("  FILY_AWS_REGION_0          First region");
        println!("  FILY_AWS_ACCESS_KEY_ID_1    Second access key");
        println!("  FILY_AWS_SECRET_ACCESS_KEY_1 Second secret key");
        println!("  FILY_AWS_REGION_1          Second region");
        println!("  ... (continue with _2, _3, etc.)");
        println!();
        println!("Method 3 - Standard AWS Variables:");
        println!("  AWS_ACCESS_KEY_ID          Access key");
        println!("  AWS_SECRET_ACCESS_KEY      Secret key");
        println!("  AWS_REGION                 Region (default: us-east-1)");
        println!();
        println!("Method 4 - Fily-specific Variables:");
        println!("  FILY_AWS_ACCESS_KEY_ID     Access key");
        println!("  FILY_AWS_SECRET_ACCESS_KEY Secret key");
        println!("  FILY_AWS_REGION            Region (default: us-east-1)");
        println!();
        println!("Encryption Configuration:");
        println!("  FILY_ENCRYPTION_ENABLED    Enable encryption (true/false, default: false)");
        println!("  FILY_ENCRYPTION_MASTER_KEY Base64-encoded 32-byte master key");
        println!();
        println!("Example - Multiple Credentials:");
        println!("  export FILY_AWS_ACCESS_KEY_ID_0=\"AKIAIOSFODNN7EXAMPLE\"");
        println!(
            "  export FILY_AWS_SECRET_ACCESS_KEY_0=\"wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY\""
        );
        println!("  export FILY_AWS_REGION_0=\"us-east-1\"");
        println!("  export FILY_AWS_ACCESS_KEY_ID_1=\"AKIAI44QH8DHBEXAMPLE\"");
        println!(
            "  export FILY_AWS_SECRET_ACCESS_KEY_1=\"je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY\""
        );
        println!("  export FILY_AWS_REGION_1=\"eu-west-1\"");
    }

    /// Validate loaded configuration
    pub fn validate(config: &Config) -> Result<()> {
        // Validate port is a valid number
        config
            .port
            .parse::<u16>()
            .map_err(|_| anyhow!("Invalid port number: {}", config.port))?;

        // Validate log level
        match config.log_level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => {
                return Err(anyhow!(
                    "Invalid log level: {}. Must be one of: trace, debug, info, warn, error",
                    config.log_level
                ))
            }
        }

        // Validate AWS credentials
        for (i, cred) in config.aws_credentials.iter().enumerate() {
            if cred.access_key_id.is_empty() {
                return Err(anyhow!("AWS credential {} has empty access key ID", i));
            }
            if cred.secret_access_key.is_empty() {
                return Err(anyhow!("AWS credential {} has empty secret access key", i));
            }
            if cred.region.is_empty() {
                return Err(anyhow!("AWS credential {} has empty region", i));
            }
        }

        // Validate encryption configuration
        if let Some(encryption) = &config.encryption {
            if encryption.enabled && encryption.master_key.is_none() {
                return Err(anyhow!("Encryption is enabled but no master key provided"));
            }
            if let Some(key) = &encryption.master_key {
                // Validate base64 format and length
                let decoded = general_purpose::STANDARD
                    .decode(key)
                    .map_err(|_| anyhow!("Encryption master key must be valid base64"))?;
                if decoded.len() != 32 {
                    return Err(anyhow!(
                        "Encryption master key must be exactly 32 bytes (256 bits) when decoded"
                    ));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_load_basic_config() {
        // Clear environment
        let vars_to_clear = [
            "FILY_LOCATION",
            "FILY_PORT",
            "FILY_ADDRESS",
            "FILY_LOG_LEVEL",
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_REGION",
            "FILY_AWS_ACCESS_KEY_ID",
            "FILY_AWS_SECRET_ACCESS_KEY",
            "FILY_AWS_REGION",
            "FILY_AWS_CREDENTIALS",
            "FILY_AWS_ACCESS_KEY_ID_0",
            "FILY_AWS_SECRET_ACCESS_KEY_0",
            "FILY_AWS_REGION_0",
            "FILY_AWS_ACCESS_KEY_ID_1",
            "FILY_AWS_SECRET_ACCESS_KEY_1",
            "FILY_AWS_REGION_1",
        ];
        for var in &vars_to_clear {
            env::remove_var(var);
        }

        let config = ConfigLoader::load().unwrap();
        assert_eq!(config.location, "./data");
        assert_eq!(config.port, "8333");
        assert_eq!(config.address, "0.0.0.0");
        assert_eq!(config.log_level, "info");
        assert!(config.aws_credentials.is_empty());
    }

    #[test]
    fn test_load_aws_credentials_standard() {
        // Clear all credential environments first
        let vars_to_clear = [
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_REGION",
            "FILY_AWS_ACCESS_KEY_ID",
            "FILY_AWS_SECRET_ACCESS_KEY",
            "FILY_AWS_REGION",
            "FILY_AWS_CREDENTIALS",
            "FILY_AWS_ACCESS_KEY_ID_0",
            "FILY_AWS_SECRET_ACCESS_KEY_0",
            "FILY_AWS_REGION_0",
            "FILY_AWS_ACCESS_KEY_ID_1",
            "FILY_AWS_SECRET_ACCESS_KEY_1",
            "FILY_AWS_REGION_1",
        ];
        for var in &vars_to_clear {
            env::remove_var(var);
        }

        env::set_var("AWS_ACCESS_KEY_ID", "test_key");
        env::set_var("AWS_SECRET_ACCESS_KEY", "test_secret");
        env::set_var("AWS_REGION", "us-west-2");

        let credentials = ConfigLoader::load_aws_credentials().unwrap();
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].access_key_id, "test_key");
        assert_eq!(credentials[0].secret_access_key, "test_secret");
        assert_eq!(credentials[0].region, "us-west-2");

        // Cleanup
        env::remove_var("AWS_ACCESS_KEY_ID");
        env::remove_var("AWS_SECRET_ACCESS_KEY");
        env::remove_var("AWS_REGION");
    }

    #[test]
    fn test_load_aws_credentials_indexed() {
        // Clear all credential environments first
        let vars_to_clear = [
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_REGION",
            "FILY_AWS_ACCESS_KEY_ID",
            "FILY_AWS_SECRET_ACCESS_KEY",
            "FILY_AWS_REGION",
            "FILY_AWS_CREDENTIALS",
            "FILY_AWS_ACCESS_KEY_ID_0",
            "FILY_AWS_SECRET_ACCESS_KEY_0",
            "FILY_AWS_REGION_0",
            "FILY_AWS_ACCESS_KEY_ID_1",
            "FILY_AWS_SECRET_ACCESS_KEY_1",
            "FILY_AWS_REGION_1",
        ];
        for var in &vars_to_clear {
            env::remove_var(var);
        }

        env::set_var("FILY_AWS_ACCESS_KEY_ID_0", "key1");
        env::set_var("FILY_AWS_SECRET_ACCESS_KEY_0", "secret1");
        env::set_var("FILY_AWS_REGION_0", "us-east-1");
        env::set_var("FILY_AWS_ACCESS_KEY_ID_1", "key2");
        env::set_var("FILY_AWS_SECRET_ACCESS_KEY_1", "secret2");
        env::set_var("FILY_AWS_REGION_1", "eu-west-1");

        let credentials = ConfigLoader::load_aws_credentials().unwrap();
        assert_eq!(credentials.len(), 2);
        assert_eq!(credentials[0].access_key_id, "key1");
        assert_eq!(credentials[1].access_key_id, "key2");

        // Cleanup
        env::remove_var("FILY_AWS_ACCESS_KEY_ID_0");
        env::remove_var("FILY_AWS_SECRET_ACCESS_KEY_0");
        env::remove_var("FILY_AWS_REGION_0");
        env::remove_var("FILY_AWS_ACCESS_KEY_ID_1");
        env::remove_var("FILY_AWS_SECRET_ACCESS_KEY_1");
        env::remove_var("FILY_AWS_REGION_1");
    }

    #[test]
    fn test_validate_config() {
        let config = Config {
            location: "./data".to_string(),
            port: "8333".to_string(),
            address: "0.0.0.0".to_string(),
            log_level: "info".to_string(),
            aws_credentials: vec![],
            encryption: None,
        };

        assert!(ConfigLoader::validate(&config).is_ok());
    }

    #[test]
    fn test_validate_invalid_port() {
        let config = Config {
            location: "./data".to_string(),
            port: "not_a_number".to_string(),
            address: "0.0.0.0".to_string(),
            log_level: "info".to_string(),
            aws_credentials: vec![],
            encryption: None,
        };

        assert!(ConfigLoader::validate(&config).is_err());
    }
}

