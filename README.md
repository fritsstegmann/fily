# Fily

An S3-compatible file storage server built in Rust using Axum. Fily implements AWS S3 API endpoints with AWS SigV4 authentication, storing files locally on disk while providing an S3-compatible interface.

## Features

- **S3-Compatible API**: Implements core AWS S3 operations including bucket and object management
- **Secure AWS SigV4 Authentication**: Constant-time signature validation preventing timing attacks
- **S3-Compatible Error Codes**: Proper HTTP status codes and XML error responses matching AWS S3
- **Content-Type Handling**: Automatic MIME type detection and user metadata support
- **ETag Generation**: MD5-based ETags for object integrity verification
- **XChaCha20-Poly1305 Encryption**: Optional server-side encryption for stored objects
- **Path Traversal Protection**: Comprehensive input validation and path sanitization
- **Local Storage Backend**: Files are stored securely on local disk with validated paths
- **Multiple AWS Credentials**: Support for multiple AWS access key/secret key pairs
- **Security-Focused Logging**: Detailed logging without sensitive data exposure

## Supported S3 Operations

### Bucket Operations

- `GET /` - List all buckets
- `PUT /{bucket}` - Create bucket
- `DELETE /{bucket}` - Delete bucket
- `GET /{bucket}` - List objects in bucket

### Object Operations

- `GET /{bucket}/{file}` - Get object with content-type, ETag, and content-length headers
- `PUT /{bucket}/{file}` - Put object with content-type detection and user metadata support
- `DELETE /{bucket}/{file}` - Delete object and associated metadata

### Authentication

- AWS SigV4 signature validation for all requests
- Pre-signed URL support with expiration validation
- Multiple credential support

## Installation

### Prerequisites

- Rust 1.70 or later
- Cargo package manager

### Building from Source

```bash
git clone <repository-url>
cd fily
cargo build --release
```

## Configuration

Fily uses environment variables for configuration, providing flexibility for different deployment scenarios. Configuration is loaded automatically from environment variables with sensible defaults.

### Quick Start

Copy the example environment file and customize it:

```bash
cp .env.example .env
# Edit .env with your settings
```

### Environment Variables

#### Core Configuration
- **FILY_LOCATION**: Storage directory (default: `./data`)
- **FILY_PORT**: Server port (default: `8333`)
- **FILY_ADDRESS**: Bind address (default: `0.0.0.0`)
- **FILY_LOG_LEVEL**: Log level (default: `info`)

#### AWS Credentials (Multiple Methods Supported)

**Method 1 - Standard AWS Variables:**
```bash
export AWS_ACCESS_KEY_ID="AKIAIOSFODNN7EXAMPLE"
export AWS_SECRET_ACCESS_KEY="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
export AWS_REGION="us-east-1"
```

**Method 2 - Multiple Credentials (Indexed):**
```bash
export FILY_AWS_ACCESS_KEY_ID_0="AKIAIOSFODNN7EXAMPLE"
export FILY_AWS_SECRET_ACCESS_KEY_0="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
export FILY_AWS_REGION_0="us-east-1"

export FILY_AWS_ACCESS_KEY_ID_1="AKIAI44QH8DHBEXAMPLE"
export FILY_AWS_SECRET_ACCESS_KEY_1="je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY"
export FILY_AWS_REGION_1="eu-west-1"
```

**Method 3 - JSON Format (Advanced):**
```bash
export FILY_AWS_CREDENTIALS='[{"access_key_id":"AKIAIOSFODNN7EXAMPLE","secret_access_key":"wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY","region":"us-east-1"}]'
```

#### Encryption Configuration (Optional)
```bash
export FILY_ENCRYPTION_ENABLED=true
export FILY_ENCRYPTION_MASTER_KEY="base64_encoded_32_byte_key"
```

Generate a master key with: `openssl rand -base64 32`

### Configuration Help

Run `fily --help` to see all available configuration options and examples.

### Multiple AWS Credentials

Fily supports multiple AWS credentials, allowing you to authenticate different clients with different access keys. This is useful for:

- **Multi-tenant scenarios**: Different access keys for different customers
- **Role separation**: Different keys for read-only vs read-write access  
- **Cross-region access**: Different keys for different AWS regions
- **Development/testing**: Separate keys for different environments

Priority order for credential loading:
1. JSON format (`FILY_AWS_CREDENTIALS`)
2. Indexed variables (`FILY_AWS_ACCESS_KEY_ID_0`, etc.)
3. Standard AWS variables (`AWS_ACCESS_KEY_ID`, etc.)
4. Fily-specific variables (`FILY_AWS_ACCESS_KEY_ID`, etc.)

### Configuration Validation

Fily validates all configuration on startup:
- Port numbers must be valid (1-65535)
- Log levels must be valid (trace, debug, info, warn, error)
- AWS credentials must be properly formatted (20-char access keys, 40-char secrets)
- Encryption keys must be valid base64 and exactly 32 bytes when decoded

## Running

### Development

```bash
cargo run
```

### Production

```bash
cargo build --release
./target/release/fily
```

### Docker

**Quick Start:**
```bash
docker run -d \
  --name fily-s3 \
  -p 8333:8333 \
  -v fily-data:/app/data \
  -e AWS_ACCESS_KEY_ID="your_access_key" \
  -e AWS_SECRET_ACCESS_KEY="your_secret_key" \
  fily:latest
```

**Docker Compose:**
```bash
# Copy and customize environment
cp .env.example .env
# Edit .env with your credentials

# Development
docker-compose -f docker-compose.development.yml up -d

# Production  
docker-compose -f docker-compose.production.yml up -d
```

See [docs/DOCKER.md](docs/DOCKER.md) for comprehensive Docker deployment guide.

The server will start and listen on the configured address and port (default: `0.0.0.0:8333`).

## Usage Examples

### Using AWS CLI

Configure AWS CLI with your credentials:

```bash
aws configure set aws_access_key_id your_access_key
aws configure set aws_secret_access_key your_secret_key
aws configure set default.region us-east-1
```

Create a bucket:

```bash
aws --endpoint-url=http://localhost:8333 s3 mb s3://my-bucket
```

Upload a file:

```bash
aws --endpoint-url=http://localhost:8333 s3 cp file.txt s3://my-bucket/
```

Upload a file with metadata:

```bash
aws --endpoint-url=http://localhost:8333 s3 cp file.txt s3://my-bucket/ \
  --metadata "author=john,version=1.0"
```

List objects:

```bash
aws --endpoint-url=http://localhost:8333 s3 ls s3://my-bucket/
```

Download a file:

```bash
aws --endpoint-url=http://localhost:8333 s3 cp s3://my-bucket/file.txt ./downloaded-file.txt
```

## Authentication

Fily implements complete AWS SigV4 authentication including:

- **Canonical Request Generation**: Proper URI encoding and header normalization
- **String-to-Sign Creation**: With credential scope and timestamp validation
- **HMAC-SHA256 Signature**: Using derived keys following AWS specification
- **Clock Skew Tolerance**: 15-minute tolerance for request timestamps
- **Pre-signed URL Validation**: With expiration time checks (max 7 days)

### Multiple Credentials

You can add multiple AWS credential sets programmatically:

```rust
let mut validator = AwsSignatureV4Validator::new();
validator.add_aws_credentials("key1", "secret1", "us-east-1");
validator.add_aws_credentials("key2", "secret2", "eu-west-1");
```

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test auth_tests
cargo test s3_handlers_tests
cargo test middleware_tests
```

### Code Quality

```bash
# Format code
cargo fmt

# Run lints
cargo clippy

# Check for security issues
cargo audit
```

### Project Structure

```
src/
├── main.rs                    # Entry point and environment-based configuration
├── config.rs                  # Environment variable configuration loader
├── fily.rs                   # Main server setup and routing with multi-credential support
└── fily/
    ├── auth.rs               # Secure AWS SigV4 authentication with timing attack protection
    ├── auth_middleware.rs    # Authentication middleware
    ├── s3_app_error.rs       # S3-compatible error responses
    ├── etag.rs               # ETag generation for object integrity
    ├── metadata.rs           # Object metadata storage and MIME detection
    ├── path_security.rs      # Path traversal protection and input validation
    ├── encryption/           # XChaCha20-Poly1305 encryption modules
    ├── list_buckets.rs       # List buckets handler
    ├── create_bucket.rs      # Create bucket handler
    ├── delete_bucket.rs      # Delete bucket handler
    ├── search_bucket.rs      # List objects handler
    ├── get_object.rs         # Secure get object handler
    ├── put_object.rs         # Secure put object handler
    └── delete_object.rs      # Secure delete object handler

tests/
├── auth_tests.rs             # Authentication tests
├── s3_handlers_tests.rs      # S3 handler tests
├── s3_error_codes_tests.rs   # S3 error code tests
├── content_type_tests.rs     # Content-type and metadata tests
├── etag_tests.rs             # ETag generation tests
├── middleware_tests.rs       # Middleware tests
├── error_handling_tests.rs   # Error handling tests
├── presigned_url_tests.rs    # Pre-signed URL tests
└── metadata_security_tests.rs # Metadata path injection security tests

docs/
├── SECURITY_AUDIT.md         # Security vulnerability assessment
├── DOCKER.md                 # Docker deployment guide
├── PERFORMANCE_REPORT.md     # Performance analysis and optimization
├── S3_COMPATIBILITY_GAP_ANALYSIS.md  # S3 API compatibility analysis
└── S3_VERSIONING_DESIGN.md   # S3 versioning implementation design

# Docker Configuration
├── Dockerfile                # Multi-stage Docker build
├── docker-compose.yml        # Basic Docker Compose setup
├── docker-compose.development.yml  # Development environment
├── docker-compose.production.yml   # Production environment
└── .env.example              # Environment variable examples
```

## Error Handling

Fily implements comprehensive S3-compatible error codes with proper HTTP status mappings:

### Supported Error Codes

- **NoSuchBucket** (404) - Bucket does not exist
- **NoSuchKey** (404) - Object does not exist  
- **BucketAlreadyExists** (409) - Bucket name already taken
- **BucketNotEmpty** (409) - Cannot delete non-empty bucket
- **InvalidBucketName** (400) - Invalid bucket name format
- **AccessDenied** (403) - Permission denied
- **InternalError** (500) - Server-side errors

All error responses follow S3 XML format with unique request IDs:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>NoSuchBucket</Code>
    <Message>The specified bucket does not exist.</Message>
    <Resource>/nonexistent-bucket</Resource>
    <RequestId>01234567-89ab-cdef-0123-456789abcdef</RequestId>
</Error>
```

## Logging

Fily uses structured logging with the `tracing` crate. Set the log level via environment variable:

```bash
RUST_LOG=debug ./fily
```

Log levels available: `trace`, `debug`, `info`, `warn`, `error`

## Security Features

- **Secure Authentication**: Constant-time AWS SigV4 signature validation prevents timing attacks
- **Path Traversal Protection**: Comprehensive input validation and path sanitization
- **Request Validation**: Timestamps validated with 15-minute clock skew tolerance
- **Secure Logging**: No sensitive information (signatures, keys) logged at any level
- **Input Sanitization**: S3-compliant bucket and object name validation
- **Directory Isolation**: Files are strictly contained within configured storage directory
- **Access Key Protection**: Generic error messages prevent access key enumeration

## Limitations

- Local storage only (no cloud storage backends)
- Basic S3 API subset (core operations only)
- No support for S3 advanced features (versioning, lifecycle policies, etc.)
- No built-in SSL/TLS (use reverse proxy for HTTPS)
- Single-node deployment only

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass: `cargo test`
6. Format code: `cargo fmt`
7. Run lints: `cargo clippy`
8. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

[Add support/contact information here]

