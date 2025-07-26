# Docker Deployment Guide

This guide covers Docker deployment options for Fily S3-compatible file storage server.

## Quick Start

The simplest way to run Fily with Docker:

```bash
docker run -d \
  --name fily-s3 \
  -p 8333:8333 \
  -v fily-data:/app/data \
  -e AWS_ACCESS_KEY_ID="your_access_key" \
  -e AWS_SECRET_ACCESS_KEY="your_secret_key" \
  -e AWS_REGION="us-east-1" \
  fily:latest
```

## Docker Compose Deployments

### Basic Development Setup

For local development and testing:

```bash
cp docker-compose.development.yml docker-compose.yml
cp .env.example .env
# Edit .env with your credentials
docker-compose up -d
```

### Production Setup

For production deployments:

```bash
cp docker-compose.production.yml docker-compose.yml
# Set environment variables (see Production Environment Variables below)
docker-compose up -d
```

## Environment Variables

### Core Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `FILY_LOCATION` | `/app/data` | Storage directory inside container |
| `FILY_PORT` | `8333` | Port to listen on |
| `FILY_ADDRESS` | `0.0.0.0` | Address to bind to |
| `FILY_LOG_LEVEL` | `info` | Log level (trace, debug, info, warn, error) |

### AWS Credentials

Choose one of the following methods:

#### Method 1: Standard AWS Variables
```bash
AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE
AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
AWS_REGION=us-east-1
```

#### Method 2: Multiple Credentials (Indexed)
```bash
FILY_AWS_ACCESS_KEY_ID_0=AKIAIOSFODNN7EXAMPLE
FILY_AWS_SECRET_ACCESS_KEY_0=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
FILY_AWS_REGION_0=us-east-1

FILY_AWS_ACCESS_KEY_ID_1=AKIAI44QH8DHBEXAMPLE
FILY_AWS_SECRET_ACCESS_KEY_1=je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY
FILY_AWS_REGION_1=eu-west-1
```

#### Method 3: JSON Format
```bash
FILY_AWS_CREDENTIALS='[{"access_key_id":"AKIAIOSFODNN7EXAMPLE","secret_access_key":"wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY","region":"us-east-1"}]'
```

### Encryption (Optional)

```bash
FILY_ENCRYPTION_ENABLED=true
FILY_ENCRYPTION_MASTER_KEY=base64_encoded_32_byte_key
```

Generate a master key: `openssl rand -base64 32`

## Production Deployment

### Environment Variables for Production

Create a `.env` file for production:

```bash
# Core Configuration
FILY_LOG_LEVEL=warn

# Multi-tenant AWS Credentials
TENANT_1_ACCESS_KEY=AKIAIOSFODNN7EXAMPLE
TENANT_1_SECRET_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
TENANT_1_REGION=us-east-1

TENANT_2_ACCESS_KEY=AKIAI44QH8DHBEXAMPLE
TENANT_2_SECRET_KEY=je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY
TENANT_2_REGION=eu-west-1

# Encryption
ENCRYPTION_ENABLED=true
ENCRYPTION_MASTER_KEY=your_base64_encoded_32_byte_key
```

### Security Considerations

1. **Use secrets management** for production credentials:
   ```bash
   # Using Docker secrets
   echo "your_access_key" | docker secret create aws_access_key -
   echo "your_secret_key" | docker secret create aws_secret_key -
   ```

2. **Limit container resources**:
   ```yaml
   deploy:
     resources:
       limits:
         memory: 512M
         cpus: "0.5"
   ```

3. **Run as non-root user** (already configured in Dockerfile)

4. **Use read-only filesystem** for enhanced security

## Volume Management

### Development
```bash
# Named volume (managed by Docker)
docker volume create fily-data
```

### Production
```bash
# Bind mount to specific directory
mkdir -p /opt/fily/data
chown 1000:1000 /opt/fily/data  # fily user in container
```

Update docker-compose.yml:
```yaml
volumes:
  - /opt/fily/data:/app/data
```

## Health Checks

Fily includes built-in health checks:

```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:8333/"]
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 40s
```

Check health status:
```bash
docker-compose ps
docker inspect fily-s3 | grep -A 5 Health
```

## Monitoring and Logs

### View logs
```bash
docker-compose logs -f fily
docker logs fily-s3
```

### Log levels
- `trace`: Very detailed debugging
- `debug`: Development debugging
- `info`: General information (default)
- `warn`: Warning messages (recommended for production)
- `error`: Error messages only

### Structured logging
Fily outputs structured logs suitable for log aggregation:
```json
{"timestamp":"2024-01-01T12:00:00Z","level":"INFO","target":"fily","message":"Server started"}
```

## Networking

### Reverse Proxy Setup

Example Nginx configuration:

```nginx
upstream fily {
    server fily:8333;
}

server {
    listen 443 ssl;
    server_name s3.yourdomain.com;
    
    ssl_certificate /etc/ssl/certs/yourdomain.crt;
    ssl_certificate_key /etc/ssl/private/yourdomain.key;
    
    location / {
        proxy_pass http://fily;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Large file upload support
        client_max_body_size 1G;
        proxy_read_timeout 300s;
        proxy_connect_timeout 75s;
    }
}
```

### Load Balancing

For high availability, run multiple Fily instances:

```yaml
version: '3.8'
services:
  fily1:
    build: .
    environment:
      - FILY_PORT=8333
  fily2:
    build: .
    environment:
      - FILY_PORT=8334
  
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
    depends_on:
      - fily1
      - fily2
```

## Backup and Recovery

### Data Backup
```bash
# Backup data volume
docker run --rm -v fily-data:/data -v $(pwd):/backup alpine tar czf /backup/fily-backup.tar.gz -C /data .

# Restore data volume
docker run --rm -v fily-data:/data -v $(pwd):/backup alpine tar xzf /backup/fily-backup.tar.gz -C /data
```

### Configuration Backup
```bash
# Backup environment variables
docker exec fily-s3 env | grep FILY > fily-config-backup.env
```

## Troubleshooting

### Common Issues

1. **Permission denied errors**:
   ```bash
   # Check volume permissions
   docker exec fily-s3 ls -la /app/data
   
   # Fix permissions
   docker exec -u root fily-s3 chown -R fily:fily /app/data
   ```

2. **Configuration validation errors**:
   ```bash
   # Check configuration
   docker exec fily-s3 ./fily --help
   
   # View startup logs
   docker logs fily-s3
   ```

3. **Network connectivity issues**:
   ```bash
   # Test internal connectivity
   docker exec fily-s3 curl -f http://localhost:8333/
   
   # Test external connectivity
   curl -f http://localhost:8333/
   ```

### Debug Mode

Enable debug logging:
```bash
docker-compose exec fily env FILY_LOG_LEVEL=debug ./fily
```

### Container Shell Access

Access the container for debugging:
```bash
docker exec -it fily-s3 /bin/bash
```

## Building Custom Images

### Multi-stage Build
The Dockerfile uses a multi-stage build for optimal image size:

```bash
# Build development image (includes build tools)
docker build --target builder -t fily:dev .

# Build production image (minimal runtime)
docker build -t fily:latest .
```

### Custom Builds
```bash
# Build with specific Rust version
docker build --build-arg RUST_VERSION=1.75 -t fily:custom .

# Build with different base image
docker build --build-arg BASE_IMAGE=alpine:latest -t fily:alpine .
```

## Performance Tuning

### Container Resources
```yaml
deploy:
  resources:
    limits:
      memory: 1G
      cpus: "1.0"
    reservations:
      memory: 512M
      cpus: "0.5"
```

### Volume Performance
- Use `tmpfs` for temporary files
- Use SSD storage for data volumes
- Consider using volume drivers optimized for your storage backend

### Network Performance
- Use host networking for maximum throughput: `network_mode: host`
- Tune kernel parameters for high-concurrency workloads

## Kubernetes Deployment

See `docs/KUBERNETES.md` for Kubernetes deployment examples and best practices.