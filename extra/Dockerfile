# Multi-stage build for smaller final image
FROM rust:1.77-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libudev-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy dependency files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libudev1 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false modbus-stream

# Create app directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/modbus-stream /usr/local/bin/

# Copy configuration template
COPY config.toml.example ./config.toml

# Change ownership
RUN chown -R modbus-stream:modbus-stream /app

# Switch to app user
USER modbus-stream

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:3000/health || exit 1

# Run the application
CMD ["modbus-stream", "--config", "/app/config.toml"]
