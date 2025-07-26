# Multi-stage Dockerfile for Fily S3-compatible file server
# Stage 1: Build stage
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy Cargo files first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release && rm -rf src

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Stage 2: Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN groupadd -r fily && useradd -r -g fily -d /app -s /bin/bash fily

# Create app directory and data directory
WORKDIR /app
RUN mkdir -p /app/data && chown -R fily:fily /app

# Copy the built binary from builder stage
COPY --from=builder /app/target/release/fily /app/fily
COPY --chown=fily:fily config-example.toml /app/config.toml

# Switch to non-root user
USER fily

# Expose the default port
EXPOSE 8333

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8333/ || exit 1

# Set environment variables
ENV RUST_LOG=info
ENV FILY_CONFIG_PATH=/app/config.toml

# Run the application
CMD ["./fily"]