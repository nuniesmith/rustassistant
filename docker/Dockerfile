# ============================================================================
# Multi-stage Dockerfile for Rustassistant
# ============================================================================

# ----------------------------------------------------------------------------
# Stage 1: Builder
# ----------------------------------------------------------------------------
FROM rust:1.75-slim as builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY static ./static

# Build release binary
RUN cargo build --release --bin devflow-server

# ----------------------------------------------------------------------------
# Stage 2: Runtime
# ----------------------------------------------------------------------------
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /build/target/release/devflow-server /usr/local/bin/

# Copy static assets and config
COPY static ./static
COPY config ./config

# Create data directory
RUN mkdir -p /app/data

# Set environment
ENV RUST_LOG=info,devflow=debug \
    HOST=0.0.0.0 \
    PORT=3000

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:3000/health || exit 1

# Run the server
CMD ["devflow-server"]
