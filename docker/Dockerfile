# ============================================================================
# Multi-stage Dockerfile for RustAssistant API & Web UI
# ============================================================================
# Build Arguments:
#   SERVICE_TYPE: "api" or "web" (default: "web")
#   PORT: Service port (default: 3001 for web, 3000 for api)
#
# Examples:
#   docker build -f docker/Dockerfile --build-arg SERVICE_TYPE=api -t rustassistant:api .
#   docker build -f docker/Dockerfile --build-arg SERVICE_TYPE=web -t rustassistant:web .
# ============================================================================

# ----------------------------------------------------------------------------
# Stage 1: Builder - Compile RustAssistant
# ----------------------------------------------------------------------------
FROM rust:1.92-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

# Create build directory
WORKDIR /build

# Copy dependency files first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source structure to build dependencies
RUN mkdir -p src/bin src/templates/layouts src/templates/pages && \
    echo "fn main() {}" > src/bin/server.rs && \
    echo "fn main() {}" > src/main.rs && \
    touch src/lib.rs && \
    touch src/templates/layouts/base.html src/templates/pages/dashboard.html

# Build dependencies only (this layer will be cached)
RUN cargo build --release --bin rustassistant-server

# Remove dummy files and rustassistant build artifacts to force rebuild
RUN rm -rf src target/release/rustassistant* target/release/.fingerprint/rustassistant*

# Copy actual source code and configuration
COPY src ./src
COPY askama.toml ./askama.toml
COPY static ./static
COPY migrations ./migrations

# Build the actual binary with optimizations
RUN cargo build --release --bin rustassistant-server && \
    strip /build/target/release/rustassistant-server

# ----------------------------------------------------------------------------
# Stage 2: Runtime - Minimal production image
# ----------------------------------------------------------------------------
FROM debian:bookworm-slim

# Build arguments for service configuration
ARG SERVICE_TYPE=web
ARG SERVICE_PORT=3001

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    wget \
    sqlite3 \
    git \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -m -u 1000 -s /bin/bash rustassistant && \
    mkdir -p /app/data /app/config /app/static && \
    chown -R rustassistant:rustassistant /app

# Set working directory
WORKDIR /app

# Copy binary from builder
COPY --from=builder --chown=rustassistant:rustassistant \
    /build/target/release/rustassistant-server \
    /usr/local/bin/rustassistant-server

# Copy static assets (for web UI)
COPY --from=builder --chown=rustassistant:rustassistant \
    /build/static ./static

# Copy config directory template
COPY --chown=rustassistant:rustassistant config ./config

# Switch to non-root user
USER rustassistant

# Environment variables with defaults
# These can be overridden by docker-compose or docker run
ENV HOST=0.0.0.0 \
    PORT=${SERVICE_PORT} \
    RUST_LOG=info,rustassistant=debug \
    DATABASE_URL=sqlite:/app/data/rustassistant.db \
    CACHE_DB_PATH=/app/data/rustassistant_cache.db \
    SERVICE_TYPE=${SERVICE_TYPE}

# Expose service port (default from build arg)
EXPOSE ${SERVICE_PORT}

# Dynamic health check based on service type
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD if [ "$SERVICE_TYPE" = "api" ]; then \
    curl -f http://localhost:${PORT:-3000}/health || exit 1; \
    else \
    wget --no-verbose --tries=1 --spider http://localhost:${PORT:-3001}/health || exit 1; \
    fi

# Run the server
CMD ["rustassistant-server"]
