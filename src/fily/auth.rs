use std::collections::HashMap;
use std::str::FromStr;

use axum::http::{HeaderMap, Method, Uri};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use percent_encoding::{percent_encode, AsciiSet, CONTROLS};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use tracing::{debug, error, info, instrument, warn};

type HmacSha256 = Hmac<Sha256>;

// AWS SigV4 constants
const AWS_ALGORITHM: &str = "AWS4-HMAC-SHA256";
const AWS_REQUEST: &str = "aws4_request";
const AWS_SERVICE: &str = "s3";
const SIGNED_HEADERS_SEPARATOR: &str = ";";
const AUTHORIZATION_HEADER: &str = "authorization";
const X_AMZ_DATE_HEADER: &str = "x-amz-date";
const X_AMZ_CONTENT_SHA256_HEADER: &str = "x-amz-content-sha256";

// URL encoding set for AWS SigV4 canonical requests
const ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'/')
    .add(b'`')
    .add(b'?')
    .add(b'{')
    .add(b'}');

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Missing authorization header")]
    MissingAuthorizationHeader,
    #[error("Invalid authorization header format")]
    InvalidAuthorizationHeader,
    #[error("Missing required header: {0}")]
    MissingRequiredHeader(String),
    #[error("Invalid date format")]
    InvalidDateFormat,
    #[error("Signature verification failed")]
    SignatureVerificationFailed,
    #[error("Invalid access key")]
    InvalidAccessKey,
    #[error("Request timestamp too old")]
    RequestTooOld,
    #[error("Malformed request")]
    MalformedRequest,
    #[error("Missing pre-signed URL parameter: {0}")]
    MissingPresignedParameter(String),
    #[error("Invalid pre-signed URL expiration")]
    InvalidExpiration,
    #[error("Pre-signed URL has expired")]
    PresignedUrlExpired,
    #[error("Invalid access key ID format: {0}")]
    InvalidAccessKeyIdFormat(String),
    #[error("Invalid secret access key format: {0}")]
    InvalidSecretAccessKeyFormat(String),
}

#[derive(Debug, Clone)]
pub struct AwsCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
}

impl AwsCredentials {
    pub fn new(access_key_id: String, secret_access_key: String, region: String) -> Result<Self, AuthError> {
        validate_access_key_id(&access_key_id)?;
        validate_secret_access_key(&secret_access_key)?;
        
        Ok(Self {
            access_key_id,
            secret_access_key,
            region,
        })
    }
}

#[derive(Debug)]
pub struct SignatureComponents {
    pub credential: String,
    pub signed_headers: String,
    pub signature: String,
}

impl FromStr for SignatureComponents {
    type Err = AuthError;

    fn from_str(auth_header: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = auth_header.split_whitespace().collect();

        debug!("signature parts, {:?}", parts);

        if parts.len() != 4 || parts[0] != AWS_ALGORITHM {
            debug!("Incorrect header {:?}", parts.len());
            return Err(AuthError::InvalidAuthorizationHeader);
        }

        let mut credential = None;
        let mut signed_headers = None;
        let mut signature = None;

        for part in parts {
            let part = part.trim().split(",").next().unwrap();

            debug!("part is {:?}", part);
            if let Some(value) = part.strip_prefix("Credential=") {
                credential = Some(value.to_string());
            } else if let Some(value) = part.strip_prefix("SignedHeaders=") {
                signed_headers = Some(value.to_string());
            } else if let Some(value) = part.strip_prefix("Signature=") {
                signature = Some(value.to_string());
            }
        }

        debug!(
            "debugging signature components {:?} {:?} {:?}",
            credential, signed_headers, signature
        );

        Ok(SignatureComponents {
            credential: credential.ok_or(AuthError::InvalidAuthorizationHeader)?,
            signed_headers: signed_headers.ok_or(AuthError::InvalidAuthorizationHeader)?,
            signature: signature.ok_or(AuthError::InvalidAuthorizationHeader)?,
        })
    }
}

pub struct AwsSignatureV4Validator {
    credentials: HashMap<String, AwsCredentials>,
}

impl AwsSignatureV4Validator {
    pub fn new() -> Self {
        Self {
            credentials: HashMap::new(),
        }
    }

    pub fn add_credentials(&mut self, access_key_id: String, credentials: AwsCredentials) -> Result<(), AuthError> {
        // Validate that the access key ID matches the one in credentials
        if access_key_id != credentials.access_key_id {
            return Err(AuthError::InvalidAccessKeyIdFormat(
                "Access key ID parameter does not match credentials access key ID".to_string()
            ));
        }
        
        // Validation already happened in AwsCredentials::new(), so we can safely insert
        self.credentials.insert(access_key_id, credentials);
        Ok(())
    }

    pub async fn validate_request(
        &self,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        body: &[u8],
    ) -> Result<String, AuthError> {
        // Extract authorization header
        let auth_header = headers
            .get(AUTHORIZATION_HEADER)
            .ok_or(AuthError::MissingAuthorizationHeader)?
            .to_str()
            .map_err(|_| AuthError::InvalidAuthorizationHeader)?;

        // Parse signature components
        let signature_components = SignatureComponents::from_str(auth_header)?;

        // Extract access key from credential
        let access_key_id = self.extract_access_key_id(&signature_components.credential)?;

        // Get credentials for this access key
        let credentials = self
            .credentials
            .get(&access_key_id)
            .ok_or(AuthError::InvalidAccessKey)?;

        // Validate timestamp
        self.validate_timestamp(headers)?;

        // Calculate expected signature
        let expected_signature = self
            .calculate_signature(
                method,
                uri,
                headers,
                body,
                credentials,
                &signature_components,
            )
            .await?;

        // Compare signatures using constant-time comparison to prevent timing attacks
        let signatures_match: bool = expected_signature
            .as_bytes()
            .ct_eq(signature_components.signature.as_bytes())
            .into();
        
        if !signatures_match {
            error!("Signature verification failed - authentication denied");
            // Do not log signatures to prevent cryptographic material exposure
            return Err(AuthError::SignatureVerificationFailed);
        }

        Ok(access_key_id)
    }

    #[instrument(
        name = "validate_presigned_request",
        skip(self, headers, _body),
        fields(
            method = %method,
            uri_path = %uri.path(),
            has_query = uri.query().is_some()
        )
    )]
    pub async fn validate_presigned_request(
        &self,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        _body: &[u8],
    ) -> Result<String, AuthError> {
        let query_params = self.parse_query_parameters(uri)?;

        let algorithm = query_params.get("X-Amz-Algorithm").ok_or_else(|| {
            error!("Missing X-Amz-Algorithm parameter in pre-signed URL");
            AuthError::MissingPresignedParameter("X-Amz-Algorithm".to_string())
        })?;

        if algorithm != AWS_ALGORITHM {
            error!(
                "Invalid algorithm '{}', expected '{}'",
                algorithm, AWS_ALGORITHM
            );
            return Err(AuthError::InvalidAuthorizationHeader);
        }

        let credential = query_params.get("X-Amz-Credential").ok_or_else(|| {
            error!("Missing X-Amz-Credential parameter in pre-signed URL");
            AuthError::MissingPresignedParameter("X-Amz-Credential".to_string())
        })?;

        let date = query_params.get("X-Amz-Date").ok_or_else(|| {
            error!("Missing X-Amz-Date parameter in pre-signed URL");
            AuthError::MissingPresignedParameter("X-Amz-Date".to_string())
        })?;

        let expires = query_params.get("X-Amz-Expires").ok_or_else(|| {
            error!("Missing X-Amz-Expires parameter in pre-signed URL");
            AuthError::MissingPresignedParameter("X-Amz-Expires".to_string())
        })?;

        let signed_headers = query_params.get("X-Amz-SignedHeaders").ok_or_else(|| {
            error!("Missing X-Amz-SignedHeaders parameter in pre-signed URL");
            AuthError::MissingPresignedParameter("X-Amz-SignedHeaders".to_string())
        })?;

        let signature = query_params.get("X-Amz-Signature").ok_or_else(|| {
            error!("Missing X-Amz-Signature parameter in pre-signed URL");
            AuthError::MissingPresignedParameter("X-Amz-Signature".to_string())
        })?;

        let access_key_id = self.extract_access_key_id(credential)?;

        let credentials = self.credentials.get(&access_key_id).ok_or_else(|| {
            error!("Authentication failed - invalid credentials");
            // Do not log access key ID to prevent enumeration attacks
            AuthError::InvalidAccessKey
        })?;

        self.validate_presigned_expiration(date, expires)?;

        let signature_components = SignatureComponents {
            credential: credential.clone(),
            signed_headers: signed_headers.clone(),
            signature: signature.clone(),
        };

        let expected_signature = self
            .calculate_presigned_signature(
                method,
                uri,
                headers,
                credentials,
                &signature_components,
                &query_params,
            )
            .await?;

        // Compare signatures using constant-time comparison to prevent timing attacks
        let signatures_match: bool = expected_signature.as_bytes().ct_eq(signature.as_bytes()).into();
        
        if !signatures_match {
            error!("Pre-signed URL signature verification failed - authentication denied");
            // Do not log signatures to prevent cryptographic material exposure
            return Err(AuthError::SignatureVerificationFailed);
        }

        info!(
            "Pre-signed URL signature validation successful for access key: {}",
            access_key_id
        );
        debug!("Signature verification completed successfully");

        Ok(access_key_id)
    }

    fn extract_access_key_id(&self, credential: &str) -> Result<String, AuthError> {
        let parts: Vec<&str> = credential.split('/').collect();
        if parts.len() < 2 {
            return Err(AuthError::InvalidAuthorizationHeader);
        }
        Ok(parts[0].to_string())
    }

    fn validate_timestamp(&self, headers: &HeaderMap) -> Result<(), AuthError> {
        let x_amz_date = headers
            .get(X_AMZ_DATE_HEADER)
            .ok_or(AuthError::MissingRequiredHeader(
                X_AMZ_DATE_HEADER.to_string(),
            ))?
            .to_str()
            .map_err(|_| AuthError::InvalidDateFormat)?;

        let request_time = timestamp_parser(x_amz_date)?;

        let now = Utc::now();
        let request_time_utc = request_time.with_timezone(&Utc);

        // Allow 15 minutes of clock skew
        let max_age = chrono::Duration::minutes(15);
        if now.signed_duration_since(request_time_utc) > max_age {
            return Err(AuthError::RequestTooOld);
        }

        Ok(())
    }

    async fn calculate_signature(
        &self,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        body: &[u8],
        credentials: &AwsCredentials,
        components: &SignatureComponents,
    ) -> Result<String, AuthError> {
        // Step 1: Create canonical request
        let canonical_request =
            self.create_canonical_request(method, uri, headers, body, components)?;

        debug!("canonical_request:\n{}", canonical_request);

        // Step 2: Create string to sign
        let string_to_sign =
            self.create_string_to_sign(&canonical_request, headers, &credentials.region)?;

        // Step 3: Calculate signature
        let signature = self.calculate_signature_value(&string_to_sign, headers, credentials)?;

        Ok(signature)
    }

    fn create_canonical_request(
        &self,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        body: &[u8],
        components: &SignatureComponents,
    ) -> Result<String, AuthError> {
        // HTTP method
        let method_str = method.as_str();

        // Canonical URI
        let canonical_uri = self.canonical_uri(uri);

        // Canonical query string
        let canonical_query_string = self.canonical_query_string(uri);

        // Canonical headers
        let (canonical_headers, signed_headers) = self.canonical_headers(headers, components)?;

        // Payload hash
        let payload_hash = if let Some(content_sha256) = headers.get(X_AMZ_CONTENT_SHA256_HEADER) {
            content_sha256
                .to_str()
                .map_err(|_| AuthError::MalformedRequest)?
                .to_string()
        } else {
            hex::encode(Sha256::digest(body))
        };

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method_str,
            canonical_uri,
            canonical_query_string,
            canonical_headers,
            signed_headers,
            payload_hash
        );

        Ok(canonical_request)
    }

    fn canonical_uri(&self, uri: &Uri) -> String {
        let path = uri.path();
        if path.is_empty() {
            "/".to_string()
        } else {
            // URI encode each path segment
            path.split('/')
                .map(|segment| self.uri_encode(segment))
                .collect::<Vec<_>>()
                .join("/")
        }
    }

    fn uri_encode(&self, s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                _ => format!("%{:02X}", c as u8),
            })
            .collect()
    }

    fn canonical_query_string(&self, uri: &Uri) -> String {
        if let Some(query) = uri.query() {
            let mut params: Vec<(String, String)> = Vec::new();

            for pair in query.split('&') {
                if let Some((key, value)) = pair.split_once('=') {
                    params.push((key.to_string(), value.to_string()));
                } else {
                    params.push((pair.to_string(), String::new()));
                }
            }

            warn!("params {:?}", params);

            params.sort_by(|a, b| a.0.cmp(&b.0));

            params
                .iter()
                .map(|(k, v)| format!("{}={}", k, self.uri_encode(v)))
                .collect::<Vec<_>>()
                .join("&")
        } else {
            String::new()
        }
    }

    fn canonical_headers(
        &self,
        headers: &HeaderMap,
        components: &SignatureComponents,
    ) -> Result<(String, String), AuthError> {
        let mut canonical_headers = Vec::new();
        let mut header_names = Vec::new();

        let signed_headers: Vec<&str> = components.signed_headers.split(";").collect();

        for (name, value) in headers.iter() {
            let name_str = name.as_str().to_lowercase();
            let value_str = value.to_str().map_err(|_| AuthError::MalformedRequest)?;

            // Normalize whitespace in header values
            let normalized_value = value_str.split_whitespace().collect::<Vec<_>>().join(" ");

            canonical_headers.push((name_str.clone(), normalized_value.trim().to_owned()));

            if signed_headers.iter().any(|&x| x == name) {
                header_names.push(name_str);
            }
        }

        // Sort headers by name
        canonical_headers.sort_by(|a, b| a.0.cmp(&b.0));
        header_names.sort();

        let canonical_headers_str = canonical_headers
            .iter()
            .filter(|(name, _)| signed_headers.iter().any(|&x| x == name))
            .map(|(name, value)| format!("{}:{}", name, value.trim()))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        let signed_headers_str = header_names.join(SIGNED_HEADERS_SEPARATOR);

        Ok((canonical_headers_str, signed_headers_str))
    }

    fn create_string_to_sign(
        &self,
        canonical_request: &str,
        headers: &HeaderMap,
        region: &str,
    ) -> Result<String, AuthError> {
        let x_amz_date = headers
            .get(X_AMZ_DATE_HEADER)
            .ok_or(AuthError::MissingRequiredHeader(
                X_AMZ_DATE_HEADER.to_string(),
            ))?
            .to_str()
            .map_err(|_| AuthError::InvalidDateFormat)?;

        debug!("x_amz_date {:?}", x_amz_date);

        let date = &x_amz_date[..8]; // YYYYMMDD
        let credential_scope = format!("{}/{}/{}/{}", date, region, AWS_SERVICE, AWS_REQUEST);

        let hashed_canonical_request = hex::encode(Sha256::digest(canonical_request.as_bytes()));

        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            AWS_ALGORITHM, x_amz_date, credential_scope, hashed_canonical_request
        );

        Ok(string_to_sign)
    }

    fn calculate_signature_value(
        &self,
        string_to_sign: &str,
        headers: &HeaderMap,
        credentials: &AwsCredentials,
    ) -> Result<String, AuthError> {
        let x_amz_date = headers
            .get(X_AMZ_DATE_HEADER)
            .ok_or(AuthError::MissingRequiredHeader(
                X_AMZ_DATE_HEADER.to_string(),
            ))?
            .to_str()
            .map_err(|_| AuthError::InvalidDateFormat)?;

        let date = &x_amz_date[..8]; // YYYYMMDD

        // Derive signing key
        let k_date = self.hmac_sha256(
            format!("AWS4{}", credentials.secret_access_key).as_bytes(),
            date.as_bytes(),
        );

        debug!("credentials {:?}", credentials);

        let k_region = self.hmac_sha256(&k_date, credentials.region.as_bytes());
        let k_service = self.hmac_sha256(&k_region, AWS_SERVICE.as_bytes());
        let k_signing = self.hmac_sha256(&k_service, AWS_REQUEST.as_bytes());

        // Calculate signature
        let signature = self.hmac_sha256(&k_signing, string_to_sign.as_bytes());

        Ok(hex::encode(signature))
    }

    fn hmac_sha256(&self, key: &[u8], data: &[u8]) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    }

    fn parse_query_parameters(&self, uri: &Uri) -> Result<HashMap<String, String>, AuthError> {
        let mut params = HashMap::new();

        if let Some(query) = uri.query() {
            for param in query.split('&') {
                if let Some((key, value)) = param.split_once('=') {
                    // URL decode the key and value
                    let decoded_key = percent_encoding::percent_decode_str(key)
                        .decode_utf8()
                        .map_err(|_| AuthError::MalformedRequest)?
                        .to_string();
                    let decoded_value = percent_encoding::percent_decode_str(value)
                        .decode_utf8()
                        .map_err(|_| AuthError::MalformedRequest)?
                        .to_string();
                    params.insert(decoded_key, decoded_value);
                }
            }
        }

        Ok(params)
    }

    #[instrument(
        name = "validate_presigned_expiration",
        skip(self),
        fields(
            date = %date,
            expires = %expires
        )
    )]
    fn validate_presigned_expiration(&self, date: &str, expires: &str) -> Result<(), AuthError> {
        let expires_seconds: u64 = expires.parse().map_err(|e| {
            error!("Failed to parse expiration time '{}': {}", expires, e);
            AuthError::InvalidExpiration
        })?;

        if !(1..=604800).contains(&expires_seconds) {
            error!(
                "Expiration time {} is outside valid range (1-604800 seconds)",
                expires_seconds
            );
            return Err(AuthError::InvalidExpiration);
        }

        let request_time = timestamp_parser(date)?;
        let request_time_utc = request_time.with_timezone(&Utc);

        // Calculate expiration time
        let expiration_time = request_time_utc + chrono::Duration::seconds(expires_seconds as i64);
        let now = Utc::now();

        // Check if the URL has expired
        if now > expiration_time {
            error!(
                "Pre-signed URL has expired (current: {}, expiration: {})",
                now, expiration_time
            );
            return Err(AuthError::PresignedUrlExpired);
        }

        Ok(())
    }

    #[instrument(
        name = "calculate_presigned_signature", 
        skip(self, headers, credentials, components, query_params),
        fields(
            method = %method,
            uri_path = %uri.path(),
            credential = %components.credential,
            signed_headers = %components.signed_headers
        )
    )]
    async fn calculate_presigned_signature(
        &self,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        credentials: &AwsCredentials,
        components: &SignatureComponents,
        query_params: &HashMap<String, String>,
    ) -> Result<String, AuthError> {
        let canonical_request = self.create_presigned_canonical_request(
            method,
            uri,
            headers,
            components,
            query_params,
        )?;
        debug!("Pre-signed canonical request: {}", canonical_request);

        let date = query_params.get("X-Amz-Date").ok_or_else(|| {
            error!("Missing X-Amz-Date in query parameters for signature calculation");
            AuthError::MissingPresignedParameter("X-Amz-Date".to_string())
        })?;

        let string_to_sign =
            self.create_presigned_string_to_sign(&canonical_request, date, &credentials.region)?;
        debug!("Pre-signed string to sign: {}", string_to_sign);

        let signature =
            self.calculate_presigned_signature_value(&string_to_sign, date, credentials)?;

        Ok(signature)
    }

    fn create_presigned_canonical_request(
        &self,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        components: &SignatureComponents,
        query_params: &HashMap<String, String>,
    ) -> Result<String, AuthError> {
        // HTTP method
        let method_str = method.as_str();

        // Canonical URI
        let canonical_uri = self.canonical_uri(uri);

        // Canonical query string for pre-signed URL (exclude X-Amz-Signature)
        let canonical_query_string = self.create_presigned_canonical_query_string(query_params)?;

        // Canonical headers for pre-signed URL
        let (canonical_headers, _) = self.canonical_headers_presigned(headers, components)?;

        // For pre-signed URLs, the payload hash is always UNSIGNED-PAYLOAD
        let payload_hash = "UNSIGNED-PAYLOAD";

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method_str,
            canonical_uri,
            canonical_query_string,
            canonical_headers,
            components.signed_headers,
            payload_hash
        );

        Ok(canonical_request)
    }

    fn create_presigned_canonical_query_string(
        &self,
        query_params: &HashMap<String, String>,
    ) -> Result<String, AuthError> {
        let mut params: Vec<(String, String)> = query_params
            .iter()
            .filter(|(k, _)| k.as_str() != "X-Amz-Signature") // Exclude signature from canonical query string
            .map(|(k, v)| {
                // URL encode key and value according to AWS spec
                let encoded_key = percent_encode(k.as_bytes(), ENCODE_SET).to_string();
                let encoded_value = percent_encode(v.as_bytes(), ENCODE_SET).to_string();
                (encoded_key, encoded_value)
            })
            .collect();

        // Sort by key
        params.sort_by(|a, b| a.0.cmp(&b.0));

        let query_string = params
            .iter()
            .map(|(k, v)| {
                if v.is_empty() {
                    k.clone()
                } else {
                    format!("{}={}", k, v)
                }
            })
            .collect::<Vec<_>>()
            .join("&");

        Ok(query_string)
    }

    fn canonical_headers_presigned(
        &self,
        headers: &HeaderMap,
        components: &SignatureComponents,
    ) -> Result<(String, String), AuthError> {
        let mut canonical_headers = Vec::new();
        let mut header_names = Vec::new();

        let signed_headers: Vec<&str> = components.signed_headers.split(";").collect();

        for (name, value) in headers.iter() {
            let name_str = name.as_str().to_lowercase();
            let value_str = value.to_str().map_err(|_| AuthError::MalformedRequest)?;

            // Normalize whitespace in header values
            let normalized_value = value_str.split_whitespace().collect::<Vec<_>>().join(" ");

            canonical_headers.push((name_str.clone(), normalized_value.trim().to_owned()));

            if signed_headers.iter().any(|&x| x == name_str) {
                header_names.push(name_str);
            }
        }

        // Sort headers by name
        canonical_headers.sort_by(|a, b| a.0.cmp(&b.0));
        header_names.sort();

        let canonical_headers_str = canonical_headers
            .iter()
            .filter(|(name, _)| signed_headers.iter().any(|&x| x == name))
            .map(|(name, value)| format!("{}:{}", name, value))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        let signed_headers_str = header_names.join(SIGNED_HEADERS_SEPARATOR);

        Ok((canonical_headers_str, signed_headers_str))
    }

    fn create_presigned_string_to_sign(
        &self,
        canonical_request: &str,
        date: &str,
        region: &str,
    ) -> Result<String, AuthError> {
        let date_only = &date[..8]; // YYYYMMDD
        let credential_scope = format!("{}/{}/{}/{}", date_only, region, AWS_SERVICE, AWS_REQUEST);

        let hashed_canonical_request = hex::encode(Sha256::digest(canonical_request.as_bytes()));

        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            AWS_ALGORITHM, date, credential_scope, hashed_canonical_request
        );

        Ok(string_to_sign)
    }

    fn calculate_presigned_signature_value(
        &self,
        string_to_sign: &str,
        date: &str,
        credentials: &AwsCredentials,
    ) -> Result<String, AuthError> {
        let date_only = &date[..8]; // YYYYMMDD

        // Derive signing key
        let k_date = self.hmac_sha256(
            format!("AWS4{}", credentials.secret_access_key).as_bytes(),
            date_only.as_bytes(),
        );

        let k_region = self.hmac_sha256(&k_date, credentials.region.as_bytes());
        let k_service = self.hmac_sha256(&k_region, AWS_SERVICE.as_bytes());
        let k_signing = self.hmac_sha256(&k_service, AWS_REQUEST.as_bytes());

        // Calculate signature
        let signature = self.hmac_sha256(&k_signing, string_to_sign.as_bytes());

        Ok(hex::encode(signature))
    }
}

fn timestamp_parser(date_str: &str) -> Result<DateTime<chrono::FixedOffset>, AuthError> {
    let request_time = DateTime::parse_from_str(&format!("{}+00:00", date_str), "%Y%m%dT%H%M%SZ%z")
        .map_err(|e| {
            error!("validate_timestamp: error {:?} {}", e, e);
            AuthError::InvalidDateFormat
        })?;
    Ok(request_time)
}

/// Validates AWS Access Key ID format according to AWS specifications
/// Format: 20 characters, starts with "AKIA", uppercase letters and digits only
fn validate_access_key_id(access_key_id: &str) -> Result<(), AuthError> {
    if access_key_id.len() != 20 {
        return Err(AuthError::InvalidAccessKeyIdFormat(
            format!("Access key ID must be exactly 20 characters long, got {}", access_key_id.len())
        ));
    }

    if !access_key_id.starts_with("AKIA") {
        return Err(AuthError::InvalidAccessKeyIdFormat(
            "Access key ID must start with 'AKIA'".to_string()
        ));
    }

    // Check that all characters are uppercase letters A-Z or digits 0-9
    if !access_key_id.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return Err(AuthError::InvalidAccessKeyIdFormat(
            "Access key ID must contain only uppercase letters A-Z and digits 0-9".to_string()
        ));
    }

    Ok(())
}

/// Validates AWS Secret Access Key format according to AWS specifications  
/// Format: 40 characters, Base64-like character set (A-Z, a-z, 0-9, +, /)
fn validate_secret_access_key(secret_access_key: &str) -> Result<(), AuthError> {
    if secret_access_key.len() != 40 {
        return Err(AuthError::InvalidSecretAccessKeyFormat(
            format!("Secret access key must be exactly 40 characters long, got {}", secret_access_key.len())
        ));
    }

    // Check that all characters are valid Base64 characters
    if !secret_access_key.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '+' || c == '/'
    }) {
        return Err(AuthError::InvalidSecretAccessKeyFormat(
            "Secret access key must contain only Base64 characters (A-Z, a-z, 0-9, +, /)".to_string()
        ));
    }

    Ok(())
}

impl Default for AwsSignatureV4Validator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Uri;

    #[test]
    fn test_timestamp_parser() {
        let converted_date = timestamp_parser("20250706T120828Z");
        converted_date.unwrap();
    }

    #[test]
    fn test_signature_components_parsing() {
        let auth_header = "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20250706/us-east-1/s3/aws4_request, SignedHeaders=host;x-amz-date, Signature=example";
        let components = SignatureComponents::from_str(auth_header).unwrap();

        assert_eq!(
            components.credential,
            "AKIAIOSFODNN7EXAMPLE/20250706/us-east-1/s3/aws4_request"
        );
        assert_eq!(components.signed_headers, "host;x-amz-date");
        assert_eq!(components.signature, "example");
    }

    #[test]
    fn test_canonical_uri_encoding() {
        let validator = AwsSignatureV4Validator::new();
        let uri: Uri = "/test_path/file.txt".parse().unwrap();
        let canonical = validator.canonical_uri(&uri);
        assert_eq!(canonical, "/test_path/file.txt");
    }

    #[test]
    fn test_canonical_query_string() {
        let validator = AwsSignatureV4Validator::new();
        let uri: Uri = "/test?b=value&a=another".parse().unwrap();
        let canonical = validator.canonical_query_string(&uri);
        assert_eq!(canonical, "a=another&b=value");
    }

    #[test]
    fn test_valid_access_key_id() {
        // Valid AWS access key ID format
        assert!(validate_access_key_id("AKIAIOSFODNN7EXAMPLE").is_ok());
        assert!(validate_access_key_id("AKIA1234567890ABCDEF").is_ok());
        assert!(validate_access_key_id("AKIAZ9Z9Z9Z9Z9Z9Z9Z9").is_ok());
    }

    #[test]
    fn test_invalid_access_key_id_length() {
        // Too short
        assert!(validate_access_key_id("AKIA123").is_err());
        // Too long
        assert!(validate_access_key_id("AKIAIOSFODNN7EXAMPLEEXTRA").is_err());
    }

    #[test]
    fn test_invalid_access_key_id_prefix() {
        // Wrong prefix
        assert!(validate_access_key_id("ABIAIOSFODNN7EXAMPLE").is_err());
        assert!(validate_access_key_id("XKIAIOSFODNN7EXAMPLE").is_err());
    }

    #[test]
    fn test_invalid_access_key_id_characters() {
        // Lowercase letters
        assert!(validate_access_key_id("AKIAiosfodnn7EXAMPLE").is_err());
        // Special characters
        assert!(validate_access_key_id("AKIA-OSFODNN7EXAMPLE").is_err());
        assert!(validate_access_key_id("AKIA+OSFODNN7EXAMPLE").is_err());
    }

    #[test]
    fn test_valid_secret_access_key() {
        // Valid AWS secret access key format (Base64 without padding)
        assert!(validate_secret_access_key("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY").is_ok());
        assert!(validate_secret_access_key("1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZabcd").is_ok());
        assert!(validate_secret_access_key("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij+/AB").is_ok());
    }

    #[test] 
    fn test_invalid_secret_access_key_length() {
        // Too short
        assert!(validate_secret_access_key("wJalrXUtnFEMI/K7MDENG").is_err());
        // Too long
        assert!(validate_secret_access_key("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEYEXTRA").is_err());
    }

    #[test]
    fn test_invalid_secret_access_key_characters() {
        // Invalid characters
        assert!(validate_secret_access_key("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLE@!#").is_err());
        assert!(validate_secret_access_key("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLE-_=").is_err());
    }

    #[test]
    fn test_aws_credentials_new_valid() {
        let credentials = AwsCredentials::new(
            "AKIAIOSFODNN7EXAMPLE".to_string(),
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            "us-east-1".to_string(),
        );
        assert!(credentials.is_ok());
    }

    #[test]
    fn test_aws_credentials_new_invalid_access_key() {
        let credentials = AwsCredentials::new(
            "INVALID_ACCESS_KEY".to_string(),
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            "us-east-1".to_string(),
        );
        assert!(credentials.is_err());
        assert!(matches!(credentials.unwrap_err(), AuthError::InvalidAccessKeyIdFormat(_)));
    }

    #[test]
    fn test_aws_credentials_new_invalid_secret_key() {
        let credentials = AwsCredentials::new(
            "AKIAIOSFODNN7EXAMPLE".to_string(),
            "INVALID_SECRET_KEY".to_string(),
            "us-east-1".to_string(),
        );
        assert!(credentials.is_err());
        assert!(matches!(credentials.unwrap_err(), AuthError::InvalidSecretAccessKeyFormat(_)));
    }

    #[test]
    fn test_validator_add_credentials_valid() {
        let mut validator = AwsSignatureV4Validator::new();
        let credentials = AwsCredentials::new(
            "AKIAIOSFODNN7EXAMPLE".to_string(),
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            "us-east-1".to_string(),
        ).unwrap();
        
        let result = validator.add_credentials("AKIAIOSFODNN7EXAMPLE".to_string(), credentials);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validator_add_credentials_mismatched_key() {
        let mut validator = AwsSignatureV4Validator::new();
        let credentials = AwsCredentials::new(
            "AKIAIOSFODNN7EXAMPLE".to_string(),
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            "us-east-1".to_string(),
        ).unwrap();
        
        let result = validator.add_credentials("AKIA1234567890ABCDEF".to_string(), credentials);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::InvalidAccessKeyIdFormat(_)));
    }
}
