# ============================================================================
# Multi-stage Dockerfile for RustAssistant
# ============================================================================
# Everything is COPY'd into the image - NO volume mounts required for
# templates, static assets, or config. Only data volume needed at runtime.
#
# Build:
#   docker build -f docker/Dockerfile -t rustassistant:latest .
#
# Run:
#   docker run -v rustassistant-data:/app/data -p 3001:3001 rustassistant:latest
# ============================================================================

# ----------------------------------------------------------------------------
# Stage 1: Chef - Prepare dependency recipe
# ----------------------------------------------------------------------------
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /build

# ----------------------------------------------------------------------------
# Stage 2: Planner - Generate recipe.json for dependency caching
# ----------------------------------------------------------------------------
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY askama.toml ./askama.toml
RUN cargo chef prepare --recipe-path recipe.json

# ----------------------------------------------------------------------------
# Stage 3: Builder - Compile dependencies (cached) then app
# ----------------------------------------------------------------------------
FROM chef AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

# Build dependencies first (this layer is cached until Cargo.toml/lock change)
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Now copy actual source and build
COPY Cargo.toml Cargo.lock askama.toml ./
COPY src ./src
COPY migrations ./migrations

# Build the binary
RUN cargo build --release --bin rustassistant-server && \
    strip /build/target/release/rustassistant-server

# ----------------------------------------------------------------------------
# Stage 4: Runtime - Minimal production image
# ----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    sqlite3 \
    git \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash rustassistant && \
    mkdir -p /app/data /app/config /app/repos /app/logs && \
    chown -R rustassistant:rustassistant /app

WORKDIR /app

# Copy binary from builder
COPY --from=builder --chown=rustassistant:rustassistant \
    /build/target/release/rustassistant-server /app/rustassistant-server

# Copy static assets INTO the image (no volume mounts needed)
COPY --chown=rustassistant:rustassistant static/ /app/static/
COPY --chown=rustassistant:rustassistant config/ /app/config/

# Switch to non-root user
USER rustassistant

# Environment defaults
ENV HOST=0.0.0.0 \
    PORT=3001 \
    DATABASE_PATH=/app/data/rustassistant.db \
    CACHE_DB_PATH=/app/data/rustassistant_cache.db \
    REPOS_PATH=/app/repos \
    RUST_LOG=info,rustassistant=debug \
    AUTO_SCAN_ENABLED=true \
    AUTO_SCAN_INTERVAL=60 \
    AUTO_SCAN_MAX_CONCURRENT=2

EXPOSE 3001

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3001/health || exit 1

ENTRYPOINT ["/app/rustassistant-server"]
