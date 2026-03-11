# ============================================================================
# Multi-stage Dockerfile for RustAssistant API & Web UI
# ============================================================================
# Build Arguments:
#   SERVICE_TYPE: "api" or "web" (default: "web")
#   SERVICE_PORT: Service port (default: 3500)
#
# Target: Oryx server (AMD64, CUDA GPU, bare-metal Linux)
#
# Examples:
#   docker build -f docker/Dockerfile -t rustassistant:web .
#   docker build -f docker/Dockerfile --build-arg SERVICE_PORT=3500 -t rustassistant:web .
# ============================================================================

# ----------------------------------------------------------------------------
# Stage 1: Builder - Compile RustAssistant
# ----------------------------------------------------------------------------
# Use debian:trixie-slim (glibc 2.41) as the builder base to satisfy ort-sys/ONNX Runtime
# prebuilt binaries which require glibc 2.38+ (__isoc23_strtoll etc.).
# debian:bookworm only ships glibc 2.36 which causes undefined symbol linker errors.
FROM debian:trixie-slim AS builder

# Install Rust toolchain + build dependencies
RUN apt-get update && apt-get install -y \
    curl \
    pkg-config \
    libssl-dev \
    ca-certificates \
    git \
    g++ \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --no-modify-path --profile minimal --default-toolchain 1.92.0 && \
    chmod -R a+w /usr/local/rustup /usr/local/cargo

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
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin rustassistant-server

# Remove dummy files and rustassistant build artifacts to force rebuild
RUN rm -rf src target/release/rustassistant* target/release/.fingerprint/rustassistant*

# Copy actual source code and configuration
COPY src ./src
COPY askama.toml ./askama.toml
COPY static ./static
COPY migrations ./migrations

# Build the actual binary with optimizations
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin rustassistant-server && \
    strip /build/target/release/rustassistant-server

# ----------------------------------------------------------------------------
# Stage 2: Runtime - Minimal production image
# ----------------------------------------------------------------------------
# Runtime must also have glibc 2.38+ since the binary was linked against trixie (glibc 2.41)
# and ort-sys embeds ONNX Runtime symbols requiring __isoc23_strtoll etc.
FROM debian:trixie-slim

# Build arguments for service configuration
ARG SERVICE_TYPE=web
ARG SERVICE_PORT=3500

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3t64 \
    curl \
    wget \
    sqlite3 \
    git \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -m -u 1000 -s /bin/bash rustassistant && \
    mkdir -p /app/data /app/config /app/static /app/repos && \
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
    DATABASE_URL=postgres://rustassistant:changeme@localhost:5432/rustassistant \
    CACHE_ENABLED=true \
    SERVICE_TYPE=${SERVICE_TYPE}

# Expose service port (default from build arg)
EXPOSE ${SERVICE_PORT}

# Dynamic health check based on service type
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD if [ "$SERVICE_TYPE" = "api" ]; then \
    curl -f http://localhost:${PORT:-3500}/health || exit 1; \
    else \
    wget --no-verbose --tries=1 --spider http://localhost:${PORT:-3500}/health || exit 1; \
    fi

# Run the server
CMD ["rustassistant-server"]
