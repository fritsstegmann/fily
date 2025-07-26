# Docker Deployment Guide for Fily

This guide explains how to build and run Fily S3-compatible file server using Docker.

## Quick Start

### Using Docker Compose (Recommended)

1. **Build and start the service:**
   ```bash
   docker-compose up -d
   ```

2. **View logs:**
   ```bash
   docker-compose logs -f fily
   ```

3. **Stop the service:**
   ```bash
   docker-compose down
   ```

### Using Docker directly

1. **Build the image:**
   ```bash
   docker build -t fily-s3 .
   ```

2. **Run the container:**
   ```bash
   docker run -d \
     --name fily-s3 \
     -p 8333:8333 \
     -v fily-data:/app/data \
     fily-s3
   ```

## Configuration

### Default Configuration
The container uses the `config-example.toml` file as the default configuration with:
- **Port:** 8333
- **Storage:** `/app/data` (mounted as volume)
- **Encryption:** Enabled with example key
- **AWS Credentials:** Example credentials (change for production!)

### Custom Configuration

1. **Create your own config file:**
   ```bash
   cp config-example.toml config.toml
   # Edit config.toml with your settings
   ```

2. **Update docker-compose.yml to mount your config:**
   ```yaml
   volumes:
     - ./config.toml:/app/config.toml:ro
     - fily-data:/app/data
   ```

3. **Generate a secure master key:**
   ```bash
   openssl rand -base64 32
   ```

## Security Considerations

### Production Deployment

1. **Change AWS credentials:**
   ```toml
   aws_access_key_id = "AKIA..." # Your actual access key
   aws_secret_access_key = "..." # Your actual secret key
   ```

2. **Generate new encryption key:**
   ```toml
   [fily.encryption]
   enabled = true
   master_key = "your-generated-key-here"
   ```

3. **Use environment variables for secrets:**
   ```bash
   docker run -d \
     --name fily-s3 \
     -p 8333:8333 \
     -e AWS_ACCESS_KEY_ID="your-key" \
     -e AWS_SECRET_ACCESS_KEY="your-secret" \
     -e ENCRYPTION_MASTER_KEY="your-master-key" \
     -v fily-data:/app/data \
     fily-s3
   ```

## Testing the Deployment

1. **Health check:**
   ```bash
   curl http://localhost:8333/
   ```

2. **Test with AWS CLI:**
   ```bash
   aws configure set aws_access_key_id AKIAIOSFODNN7EXAMPLE
   aws configure set aws_secret_access_key wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
   aws configure set default.region us-east-1
   
   # Create bucket
   aws --endpoint-url=http://localhost:8333 s3 mb s3://test-bucket
   
   # Upload file
   echo "Hello, Fily!" > test.txt
   aws --endpoint-url=http://localhost:8333 s3 cp test.txt s3://test-bucket/
   
   # Download file
   aws --endpoint-url=http://localhost:8333 s3 cp s3://test-bucket/test.txt downloaded.txt
   ```

## Container Details

### Image Size Optimization
- **Multi-stage build** reduces final image size
- **Debian slim** base image for minimal runtime
- **Only essential dependencies** included

### Security Features
- **Non-root user** (fily:fily)
- **Minimal attack surface** with slim base image
- **No build tools** in final image

### Monitoring
- **Health checks** configured for container orchestration
- **Structured logging** with configurable levels
- **Prometheus metrics** (if enabled in config)

## Troubleshooting

### Common Issues

1. **Permission denied on data directory:**
   ```bash
   docker run --user $(id -u):$(id -g) ...
   ```

2. **Configuration not loading:**
   ```bash
   docker logs fily-s3
   # Check for config parsing errors
   ```

3. **Port already in use:**
   ```bash
   docker run -p 8334:8333 ...  # Use different host port
   ```

### Debug Mode
```bash
docker run -e RUST_LOG=debug fily-s3
```

## Volumes and Persistence

- **`/app/data`**: S3 object storage (should be persistent)
- **`/app/config.toml`**: Configuration file (optional mount)

Make sure to use named volumes or host mounts for data persistence in production.