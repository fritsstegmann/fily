# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Fily is an S3-compatible file storage server built in Rust using Axum. It implements AWS S3 API endpoints with AWS SigV4 authentication, storing files locally on disk while providing an S3-compatible interface.

## Common Development Commands

### Building and Running
- `cargo build` - Build the project
- `cargo run` - Run the server (reads config from ./config.toml)
- `cargo test` - Run all tests
- `cargo clippy` - Run linting checks
- `cargo fmt` - Format code

### Configuration
The server reads configuration from `config.toml` in the project root:
- `fily.location` - Local storage directory (default: "./data")
- `fily.port` - Server port (default: "8333") 
- `fily.address` - Bind address (default: "0.0.0.0")
- `fily.aws_access_key_id` - AWS access key for authentication
- `fily.aws_secret_access_key` - AWS secret key for authentication
- `fily.aws_region` - AWS region for signature validation
- `encryption.enabled` - Enable XChaCha20-Poly1305 encryption (optional)
- `encryption.master_key` - Base64-encoded 32-byte encryption key (optional)

## Architecture

### Core Components
- `src/main.rs` - Entry point, config loading, and tracing setup
- `src/fily.rs` - Main server setup, routing, and graceful shutdown
- `src/fily/auth.rs` - AWS SigV4 authentication implementation
- `src/fily/auth_middleware.rs` - Authentication middleware layer
- `src/fily/s3_app_error.rs` - S3-compatible error responses with proper HTTP status codes
- `src/fily/etag.rs` - MD5-based ETag generation for object integrity
- `src/fily/metadata.rs` - Object metadata storage, MIME type detection, and user metadata
- `src/fily/encryption/` - XChaCha20-Poly1305 encryption modules for server-side encryption

### S3 API Handlers
Each S3 operation has its own handler module:
- `list_buckets.rs` - GET / (list all buckets)
- `create_bucket.rs` - PUT /{bucket} (create bucket with name validation)
- `create_general_bucket.rs` - PUT / (create bucket via body)
- `delete_bucket.rs` - DELETE /{bucket} (with emptiness validation)
- `search_bucket.rs` - GET /{bucket} (list objects in bucket - currently stub)
- `get_object.rs` - GET /{bucket}/{file} (with content-type, ETag, metadata)
- `put_object.rs` - PUT /{bucket}/{file} (with content-type detection, metadata, encryption)
- `delete_object.rs` - DELETE /{bucket}/{file} (with metadata cleanup)

### Authentication System
- Implements full AWS SigV4 signature validation
- Supports multiple AWS credentials via `AwsSignatureV4Validator`
- Validates request timestamps with 15-minute clock skew tolerance
- Canonical request creation follows AWS specification exactly

### File Storage
- Files stored in local directory specified by config
- Directory structure mirrors S3 bucket/object hierarchy
- Object metadata stored in `.fily-metadata/` directories as JSON files
- Optional XChaCha20-Poly1305 encryption for stored objects
- Concurrent access handled by tokio async runtime

## Development Notes

### Testing S3 Compatibility
Use the included `s3_upload.sh` script to test uploads:
```bash
./s3_upload.sh /path/to/file /bucket/path/
```

### AWS SigV4 Implementation
The authentication implementation is comprehensive and follows AWS specifications:
- Canonical request generation with proper URI encoding
- String-to-sign creation with credential scope
- HMAC-SHA256 signature calculation with derived keys
- Header normalization and sorting

### Error Handling
- Comprehensive S3-compatible error codes (25+ standard error types)
- Proper HTTP status code mapping (404, 409, 403, 400, 500)
- S3-compatible XML error responses with unique request IDs
- Smart error conversion from IO errors to appropriate S3 errors
- Bucket name validation following S3 naming rules

### Content-Type and Metadata Handling
- Automatic MIME type detection for 100+ file extensions via `mime_guess`
- Support for custom user metadata via `x-amz-meta-*` headers
- Content-Type, Content-Length, and ETag headers in responses
- Persistent metadata storage alongside objects

### ETag Implementation
- MD5-based ETag generation matching S3 behavior
- ETags returned in both GET and PUT responses
- Consistent handling for encrypted and unencrypted objects

### Encryption System
- XChaCha20-Poly1305 authenticated encryption
- Configurable per-server via config.toml
- Associated data includes bucket/object path for uniqueness
- Transparent encryption/decryption in handlers

### Logging
Configured via `tracing` with level set in config.toml. All AWS auth operations include detailed debug logging.

## Security and Performance Audit Requirements

### Pre-Commit Audit Checklist

Before committing any changes to the repository, developers MUST perform both security and performance audits:

#### Security Audit Requirements

1. **Authentication Security Review:**
   - Verify no hardcoded credentials or secrets in code
   - Check for timing attack vulnerabilities in signature validation
   - Ensure no sensitive information is logged (signatures, keys, etc.)
   - Validate all user inputs are properly sanitized

2. **File Handling Security Review:**
   - Check for path traversal vulnerabilities in file operations
   - Verify bucket and object names are properly validated
   - Ensure metadata file paths are sanitized
   - Review file permission and access control implementations

3. **Input Validation Review:**
   - Verify all external inputs are validated and sanitized
   - Check for injection vulnerabilities (path injection, command injection)
   - Ensure proper error handling without information disclosure
   - Validate request size limits and resource consumption controls

4. **Encryption Security Review:**
   - Verify encryption implementation uses secure practices
   - Check for proper key management and storage
   - Ensure nonce/IV generation is cryptographically secure
   - Review for side-channel attack vulnerabilities

#### Performance Audit Requirements

1. **Memory Management Review:**
   - Check for excessive memory allocations in hot paths
   - Verify streaming operations for large requests/responses
   - Ensure proper buffer reuse and memory pool usage
   - Review for memory leaks and unbounded growth

2. **Concurrency and Scalability Review:**
   - Verify async/await patterns are properly implemented
   - Check for blocking operations in async contexts
   - Ensure proper backpressure and rate limiting
   - Review concurrency limits and resource management

3. **I/O Performance Review:**
   - Check for efficient file I/O patterns
   - Verify metadata operations are optimized
   - Ensure proper caching strategies are implemented
   - Review for unnecessary disk operations

4. **Request Processing Performance:**
   - Verify authentication overhead is minimized
   - Check for efficient request parsing and validation
   - Ensure response generation is optimized
   - Review for unnecessary string allocations and copies

### Audit Tools and Commands

#### Security Audit Commands:
```bash
# Run security-focused tests
cargo test --test security_tests

# Check for hardcoded secrets
git grep -i "password\|secret\|key" src/
git grep "AKIA\|wJalr" src/

# Static analysis for security issues
cargo clippy -- -W clippy::unwrap_used -W clippy::expect_used

# Dependency vulnerability scan
cargo audit
```

#### Performance Audit Commands:
```bash
# Run performance tests
cargo test --test performance_tests --release

# Memory usage analysis
cargo run --release -- --memory-profile

# CPU profiling
cargo flamegraph --bin fily

# Benchmark critical paths
cargo bench
```

### Mandatory Security Checks Before Commit

1. **No Hardcoded Secrets:** Verify no credentials, keys, or secrets in code
2. **Input Sanitization:** All user inputs are validated and sanitized
3. **Path Security:** File paths are constructed safely without traversal risks
4. **Authentication Security:** No timing attacks or credential exposure
5. **Error Handling:** No sensitive information disclosed in errors
6. **Logging Security:** No secrets or sensitive data in log statements

### Mandatory Performance Checks Before Commit

1. **Memory Efficiency:** No excessive allocations in request processing
2. **Async Correctness:** No blocking operations in async contexts
3. **Resource Limits:** Proper bounds on memory, connections, and resources
4. **Caching Strategy:** Appropriate caching for frequently accessed data
5. **I/O Efficiency:** Optimal file and network I/O patterns
6. **Scalability:** Code supports concurrent operations without bottlenecks

### Audit Documentation Requirements

For significant changes, include in commit message:
- Security impact assessment
- Performance impact analysis
- Resource usage implications
- Scalability considerations

### Emergency Commit Process

For critical security fixes, the audit process may be expedited but MUST include:
1. Immediate security review by another team member
2. Basic performance sanity check
3. Explicit documentation of security risk and mitigation
4. Follow-up comprehensive audit within 24 hours

### Continuous Monitoring

Implement ongoing monitoring for:
- Authentication failure patterns
- Resource usage anomalies
- Performance degradation
- Security event detection
- Error rate monitoring

Refer to `docs/SECURITY_AUDIT.md` and `docs/PERFORMANCE_REPORT.md` for detailed guidelines and findings.