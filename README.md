# Fily

An S3-compatible file storage server built in Rust using Axum. Fily implements AWS S3 API endpoints with AWS SigV4 authentication, storing files locally on disk while providing an S3-compatible interface.

## Features

- **S3-Compatible API**: Implements core AWS S3 operations including bucket and object management
- **AWS SigV4 Authentication**: Full support for AWS Signature Version 4 authentication
- **S3-Compatible Error Codes**: Proper HTTP status codes and XML error responses matching AWS S3
- **Content-Type Handling**: Automatic MIME type detection and user metadata support
- **ETag Generation**: MD5-based ETags for object integrity verification
- **XChaCha20-Poly1305 Encryption**: Optional server-side encryption for stored objects
- **Pre-signed URL Support**: Generate and validate pre-signed URLs for temporary access
- **Local Storage Backend**: Files are stored on local disk with configurable location
- **Multiple AWS Credentials**: Support for multiple AWS access key/secret key pairs
- **Comprehensive Logging**: Detailed request/response logging with configurable levels

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

Fily uses a `config.toml` file in the project root for configuration. Copy the example configuration and customize it:

```bash
cp config-example.toml config.toml
# Edit config.toml with your settings
```

```toml
[fily]
location = "./data"           # Local storage directory
port = "8333"                # Server port
address = "0.0.0.0"          # Bind address

# AWS credentials for authentication
aws_access_key_id = "your_access_key"
aws_secret_access_key = "your_secret_key"
aws_region = "us-east-1"

# Optional encryption configuration
[encryption]
enabled = false
master_key = "base64_encoded_32_byte_key"
```

### Configuration Options

#### [fily] Section
- **location**: Directory where files will be stored locally (default: "./data")
- **port**: Port number for the server to listen on (default: "8333")
- **address**: IP address to bind to (default: "0.0.0.0")
- **aws_access_key_id**: AWS access key ID for authentication
- **aws_secret_access_key**: AWS secret access key for authentication
- **aws_region**: AWS region for signature validation (default: "us-east-1")

#### [encryption] Section (Optional)
- **enabled**: Enable/disable XChaCha20-Poly1305 encryption for stored objects
- **master_key**: Base64-encoded 32-byte master key for encryption

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
├── main.rs                    # Entry point and configuration
├── fily.rs                   # Main server setup and routing
└── fily/
    ├── auth.rs               # AWS SigV4 authentication
    ├── auth_middleware.rs    # Authentication middleware
    ├── s3_app_error.rs       # S3-compatible error responses
    ├── etag.rs               # ETag generation for object integrity
    ├── metadata.rs           # Object metadata storage and MIME detection
    ├── encryption/           # XChaCha20-Poly1305 encryption modules
    ├── list_buckets.rs       # List buckets handler
    ├── create_bucket.rs      # Create bucket handler
    ├── delete_bucket.rs      # Delete bucket handler
    ├── search_bucket.rs      # List objects handler
    ├── get_object.rs         # Get object handler
    ├── put_object.rs         # Put object handler
    ├── delete_object.rs      # Delete object handler
    └── generate_presigned_url.rs # Pre-signed URL generation

tests/
├── auth_tests.rs             # Authentication tests
├── s3_handlers_tests.rs      # S3 handler tests
├── s3_error_codes_tests.rs   # S3 error code tests
├── content_type_tests.rs     # Content-type and metadata tests
├── etag_tests.rs             # ETag generation tests
├── middleware_tests.rs       # Middleware tests
├── error_handling_tests.rs   # Error handling tests
└── presigned_url_tests.rs    # Pre-signed URL tests
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

## Security Considerations

- All requests require proper AWS SigV4 authentication
- Pre-signed URLs have configurable expiration times (max 7 days)
- Request timestamps are validated with 15-minute clock skew tolerance
- File paths are validated to prevent directory traversal attacks
- No sensitive information is logged at default log levels

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

