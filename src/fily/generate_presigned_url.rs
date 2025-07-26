use std::collections::HashMap;

use axum::extract::{Path, Query};
use axum::http::{HeaderMap, Method};
use axum::response::Json;
use chrono::Utc;
use percent_encoding::{percent_encode, AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument, warn};

use super::auth::AwsCredentials;
use super::s3_app_error::S3AppError;

// URL encoding set for AWS SigV4 canonical requests
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

// More restrictive encoding for query parameters in final URLs
const QUERY_ENCODE_SET: &AsciiSet = &ENCODE_SET
    .add(b'/');

// AWS SigV4 constants
const AWS_ALGORITHM: &str = "AWS4-HMAC-SHA256";
const AWS_REQUEST: &str = "aws4_request";
const AWS_SERVICE: &str = "s3";

#[derive(Debug, Deserialize)]
pub struct GeneratePresignedUrlQuery {
    #[serde(rename = "X-Amz-Expires")]
    pub expires: Option<u64>,
    #[serde(rename = "X-Amz-Method")]
    pub method: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PresignedUrlResponse {
    pub url: String,
    pub expires_in: u64,
    pub method: String,
}

#[instrument(
    name = "generate_presigned_url",
    skip(headers),
    fields(
        bucket = %bucket,
        object = %object,
        expires = ?params.expires,
        method = ?params.method
    )
)]
pub async fn handle(
    Path((bucket, object)): Path<(String, String)>,
    Query(params): Query<GeneratePresignedUrlQuery>,
    headers: HeaderMap,
) -> Result<Json<PresignedUrlResponse>, S3AppError> {
    info!(
        "Generating pre-signed URL for bucket: {}, object: {}",
        bucket, object
    );
    debug!(
        "Request parameters: expires={:?}, method={:?}",
        params.expires, params.method
    );

    // Get credentials from request (this would normally be extracted from auth)
    // For now, we'll use a placeholder - in a real implementation,
    // this would be extracted from the authenticated request
    debug!("Extracting credentials from request headers");
    let credentials = get_credentials_from_request(&headers)?;
    debug!(
        "Successfully extracted credentials for access key: {}",
        credentials.access_key_id
    );

    // Default expiration is 1 hour, max is 7 days
    let expires_seconds = params.expires.unwrap_or(3600).clamp(1, 604800);
    if params.expires.is_some() && (params.expires.unwrap() < 1 || params.expires.unwrap() > 604800)
    {
        warn!(
            "Expiration time {} was clamped to valid range (1-604800 seconds)",
            params.expires.unwrap()
        );
    }
    debug!("Using expiration time: {} seconds", expires_seconds);

    // Default method is GET
    let method_str = params.method.unwrap_or_else(|| "GET".to_string());
    debug!("Using HTTP method: {}", method_str);
    let method = method_str.parse::<Method>().map_err(|e| {
        error!("Failed to parse HTTP method '{}': {}", method_str, e);
        S3AppError::from(anyhow::anyhow!("Invalid HTTP method: {}", method_str))
    })?;

    // Generate current timestamp
    let now = Utc::now();
    let timestamp = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date = now.format("%Y%m%d").to_string();
    debug!("Generated timestamp: {}, date: {}", timestamp, date);

    // Create credential scope
    let credential_scope = format!(
        "{}/{}/{}/{}",
        date, credentials.region, AWS_SERVICE, AWS_REQUEST
    );
    let credential = format!("{}/{}", credentials.access_key_id, credential_scope);
    debug!("Created credential scope: {}", credential_scope);
    debug!("Full credential: {}", credential);

    // Build base URL
    let base_url = format!("/{}/{}", bucket, object);
    debug!("Base URL: {}", base_url);
    
    // Parse as URI to match validation logic
    let uri = base_url.parse::<axum::http::Uri>().map_err(|e| {
        error!("Failed to parse URI '{}': {}", base_url, e);
        S3AppError::from(anyhow::anyhow!("Invalid URI: {}", e))
    })?;

    // Create query parameters for pre-signed URL
    let mut query_params = HashMap::new();
    query_params.insert("X-Amz-Algorithm".to_string(), AWS_ALGORITHM.to_string());
    query_params.insert("X-Amz-Credential".to_string(), credential);
    query_params.insert("X-Amz-Date".to_string(), timestamp.clone());
    query_params.insert("X-Amz-Expires".to_string(), expires_seconds.to_string());
    query_params.insert("X-Amz-SignedHeaders".to_string(), "host".to_string());

    // Create canonical request (before adding signature)
    debug!("Creating canonical request");
    let canonical_request = create_canonical_request(&method, &uri, &query_params, &headers)?;
    debug!("Canonical request created successfully");
    debug!("Canonical request: {}", canonical_request);

    // Create string to sign
    debug!("Creating string to sign");
    let string_to_sign =
        create_string_to_sign(&canonical_request, &timestamp, &credentials.region)?;
    debug!("String to sign created successfully");
    debug!("String to sign: {}", string_to_sign);

    // Calculate signature
    debug!("Calculating signature");
    let signature = calculate_signature(&string_to_sign, &date, &credentials)?;
    debug!("Signature calculated successfully");
    debug!("Signature: {}", signature);

    // Add signature to query parameters for final URL
    query_params.insert("X-Amz-Signature".to_string(), signature);

    // Build final URL
    debug!("Building final presigned URL");
    let query_string = build_query_string(&query_params);
    let presigned_url = format!("{}?{}", uri.path(), query_string);

    info!(
        "Successfully generated pre-signed URL for {}/{}, expires in {} seconds",
        bucket, object, expires_seconds
    );
    debug!("Generated presigned URL: {}", presigned_url);

    let response = PresignedUrlResponse {
        url: presigned_url,
        expires_in: expires_seconds,
        method: method_str,
    };

    Ok(Json(response))
}

#[instrument(name = "get_credentials_from_request", skip(_headers))]
fn get_credentials_from_request(_headers: &HeaderMap) -> Result<AwsCredentials, S3AppError> {
    // This is a placeholder implementation
    // In a real system, you would extract credentials from the authenticated request
    // or from a credential store based on the authenticated user

    debug!("Using placeholder credentials for demo purposes");
    warn!("Using hardcoded demo credentials - replace with proper credential resolution in production");

    // For demo purposes, return default credentials
    // This should be replaced with proper credential resolution
    Ok(AwsCredentials {
        access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
        secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
        region: "us-east-1".to_string(),
    })
}

#[instrument(
    name = "create_canonical_request",
    skip(query_params, headers),
    fields(
        method = %method,
        uri = %uri,
        param_count = query_params.len()
    )
)]
fn create_canonical_request(
    method: &Method,
    uri: &axum::http::Uri,
    query_params: &HashMap<String, String>,
    headers: &HeaderMap,
) -> Result<String, S3AppError> {
    // HTTP method
    let method_str = method.as_str();
    debug!("HTTP method: {}", method_str);

    // Canonical URI (using same logic as auth.rs)
    let canonical_uri = canonical_uri_from_uri(uri);
    debug!("Canonical URI: {}", canonical_uri);

    // Canonical query string (excluding signature)
    debug!(
        "Creating canonical query string from {} parameters",
        query_params.len()
    );
    let canonical_query_string = create_canonical_query_string(query_params)?;
    debug!("Canonical query string: {}", canonical_query_string);

    // Canonical headers (minimal for pre-signed URL)
    // Use only host header for presigned URLs, like in validation
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:8333");
    let canonical_headers = format!("host:{}\n", host);
    let signed_headers = "host";
    debug!("Canonical headers: {}", canonical_headers.trim());
    debug!("Signed headers: {}", signed_headers);

    // Payload hash for pre-signed URL is always UNSIGNED-PAYLOAD
    let payload_hash = "UNSIGNED-PAYLOAD";
    debug!("Payload hash: {}", payload_hash);

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method_str,
        canonical_uri,
        canonical_query_string,
        canonical_headers,
        signed_headers,
        payload_hash
    );

    debug!("Canonical request components assembled successfully");
    Ok(canonical_request)
}

#[instrument(
    name = "create_canonical_query_string",
    skip(query_params),
    fields(param_count = query_params.len())
)]
fn create_canonical_query_string(
    query_params: &HashMap<String, String>,
) -> Result<String, S3AppError> {
    debug!(
        "Processing {} query parameters for canonical string",
        query_params.len()
    );

    let mut params: Vec<(String, String)> = query_params
        .iter()
        .filter(|(k, _)| {
            let include = k.as_str() != "X-Amz-Signature";
            if !include {
                debug!("Excluding X-Amz-Signature from canonical query string");
            }
            include
        })
        .map(|(k, v)| {
            // URL encode key and value according to AWS spec
            let encoded_key = percent_encode(k.as_bytes(), ENCODE_SET).to_string();
            let encoded_value = percent_encode(v.as_bytes(), ENCODE_SET).to_string();
            debug!("Encoded parameter: {}={}", encoded_key, encoded_value);
            (encoded_key, encoded_value)
        })
        .collect();

    // Sort by key
    debug!("Sorting {} parameters by key", params.len());
    params.sort_by(|a, b| a.0.cmp(&b.0));

    let query_string = params
        .iter()
        .map(|(k, v)| {
            let param = if v.is_empty() {
                k.clone()
            } else {
                format!("{}={}", k, v)
            };
            debug!("Query string parameter: {}", param);
            param
        })
        .collect::<Vec<_>>()
        .join("&");

    debug!(
        "Canonical query string created with {} parameters",
        params.len()
    );
    Ok(query_string)
}

#[instrument(
    name = "create_string_to_sign",
    skip(canonical_request),
    fields(
        timestamp = %timestamp,
        region = %region,
        canonical_request_len = canonical_request.len()
    )
)]
fn create_string_to_sign(
    canonical_request: &str,
    timestamp: &str,
    region: &str,
) -> Result<String, S3AppError> {
    let date = &timestamp[..8]; // YYYYMMDD
    debug!("Extracted date from timestamp: {}", date);

    let credential_scope = format!("{}/{}/{}/{}", date, region, AWS_SERVICE, AWS_REQUEST);
    debug!("Created credential scope: {}", credential_scope);

    use sha2::{Digest, Sha256};
    let hashed_canonical_request = hex::encode(Sha256::digest(canonical_request.as_bytes()));
    debug!(
        "Hashed canonical request (SHA256): {}",
        hashed_canonical_request
    );

    let string_to_sign = format!(
        "{}\n{}\n{}\n{}",
        AWS_ALGORITHM, timestamp, credential_scope, hashed_canonical_request
    );

    debug!("String to sign created with algorithm: {}", AWS_ALGORITHM);
    Ok(string_to_sign)
}

#[instrument(
    name = "calculate_signature",
    skip(string_to_sign, credentials),
    fields(
        date = %date,
        region = %credentials.region,
        string_to_sign_len = string_to_sign.len()
    )
)]
fn calculate_signature(
    string_to_sign: &str,
    date: &str,
    credentials: &AwsCredentials,
) -> Result<String, S3AppError> {
    debug!("Starting signature calculation process");

    // Derive signing key
    debug!("Deriving signing key components");
    let k_date = hmac_sha256(
        format!("AWS4{}", credentials.secret_access_key).as_bytes(),
        date.as_bytes(),
    );
    debug!("k_date calculated");

    let k_region = hmac_sha256(&k_date, credentials.region.as_bytes());
    debug!("k_region calculated");

    let k_service = hmac_sha256(&k_region, AWS_SERVICE.as_bytes());
    debug!("k_service calculated");

    let k_signing = hmac_sha256(&k_service, AWS_REQUEST.as_bytes());
    debug!("k_signing calculated - signing key derivation complete");

    // Calculate signature
    debug!("Calculating final signature");
    let signature = hmac_sha256(&k_signing, string_to_sign.as_bytes());
    let hex_signature = hex::encode(signature);

    debug!("Signature calculation completed successfully");
    Ok(hex_signature)
}

fn canonical_uri_from_uri(uri: &axum::http::Uri) -> String {
    let path = uri.path();
    if path.is_empty() {
        "/".to_string()
    } else {
        // URI encode each path segment
        path.split('/')
            .map(|segment| percent_encode(segment.as_bytes(), ENCODE_SET).to_string())
            .collect::<Vec<_>>()
            .join("/")
    }
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn build_query_string(params: &HashMap<String, String>) -> String {
    let mut sorted_params: Vec<(&String, &String)> = params.iter().collect();
    sorted_params.sort_by(|a, b| a.0.cmp(b.0));

    sorted_params
        .iter()
        .map(|(k, v)| {
            // URL encode for the final presigned URL using more restrictive encoding
            let encoded_key = percent_encode(k.as_bytes(), QUERY_ENCODE_SET).to_string();
            let encoded_value = percent_encode(v.as_bytes(), QUERY_ENCODE_SET).to_string();
            format!("{}={}", encoded_key, encoded_value)
        })
        .collect::<Vec<_>>()
        .join("&")
}
