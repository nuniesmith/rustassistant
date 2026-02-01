#!/bin/bash
set -euo pipefail

# RustAssistant Quick Start Script
# This script helps you get started with RustAssistant quickly

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Print banner
echo ""
echo "╔═══════════════════════════════════════════╗"
echo "║    RustAssistant - Quick Start            ║"
echo "║    Developer Workflow & LLM Analysis      ║"
echo "╚═══════════════════════════════════════════╝"
echo ""

# Check for Rust
if ! command -v cargo &> /dev/null; then
    error "Rust/Cargo not found. Please install from https://rustup.rs/"
    exit 1
fi

RUST_VERSION=$(cargo --version | awk '{print $2}')
info "Using Rust version: $RUST_VERSION"

# Check for .env file
if [ ! -f .env ]; then
    warn ".env file not found. Creating from template..."
    if [ -f .env.example ]; then
        cp .env.example .env
        warn "Please edit .env and add your XAI_API_KEY for LLM features"
    else
        error ".env.example not found!"
        exit 1
    fi
fi

# Create directories
info "Creating workspace directories..."
mkdir -p workspace reports tasks

# Parse command
COMMAND=${1:-help}

case $COMMAND in
    build)
        info "Building RustAssistant..."
        cargo build --release
        success "Build complete! Binaries in target/release/"
        ;;

    server)
        info "Starting RustAssistant server..."
        info "API will be available at http://localhost:8080"
        cargo run --bin rustassistant-server
        ;;

    cli)
        shift
        info "Running CLI command: $*"
        cargo run --bin rustassistant -- "$@"
        ;;

    test)
        info "Running tests..."
        cargo test
        ;;

    audit)
        TARGET=${2:-.}
        info "Running quick audit on: $TARGET"
        cargo run --bin rustassistant -- analyze batch "$TARGET"
        ;;

    tags)
        TARGET=${2:-.}
        info "Scanning for tags in: $TARGET"
        cargo run --bin audit-cli -- tags "$TARGET" --format text
        ;;

    static)
        TARGET=${2:-.}
        info "Running static analysis on: $TARGET"
        cargo run --bin audit-cli -- static "$TARGET" --format text
        ;;

    tasks)
        TARGET=${2:-.}
        info "Generating tasks from: $TARGET"
        cargo run --bin audit-cli -- tasks "$TARGET" --format text
        ;;

    stats)
        TARGET=${2:-.}
        info "Showing statistics for: $TARGET"
        cargo run --bin audit-cli -- stats "$TARGET"
        ;;

    dev)
        info "Starting development server with auto-reload..."
        if command -v cargo-watch &> /dev/null; then
            RUST_LOG=debug cargo watch -x 'run --bin audit-server'
        else
            warn "cargo-watch not found. Install with: cargo install cargo-watch"
            info "Starting server without auto-reload..."
            RUST_LOG=debug cargo run --bin audit-server
        fi
        ;;

    docker)
        info "Building Docker image..."
        docker build -t rustassistant -f docker/Dockerfile .
        success "Docker image built: rustassistant"
        info "Run with: docker run -p 8080:8080 -e XAI_API_KEY=your-key rustassistant"
        ;;

    clean)
        info "Cleaning build artifacts..."
        cargo clean
        rm -rf workspace/* reports/* tasks/*
        success "Clean complete!"
        ;;

    install)
        info "Installing CLI tool globally..."
        cargo install --path . --bin rustassistant
        success "Installed! Run 'rustassistant --help'"
        ;;

    check)
        info "Running quick checks..."
        echo ""
        info "1. Checking Rust environment..."
        cargo --version
        rustc --version

        echo ""
        info "2. Checking .env configuration..."
        if grep -q "your-grok-api-key-here" .env 2>/dev/null; then
            warn "XAI_API_KEY not set in .env - LLM features will be disabled"
        else
            success "XAI_API_KEY is configured"
        fi

        echo ""
        info "3. Checking dependencies..."
        cargo check --quiet && success "Dependencies OK" || error "Dependency issues found"

        echo ""
        info "4. Running tests..."
        cargo test --quiet && success "Tests passing" || warn "Some tests failed"

        echo ""
        success "Environment check complete!"
        ;;

    help|--help|-h)
        echo "Usage: ./run.sh <command> [options]"
        echo ""
        echo "Commands:"
        echo "  build          Build the project in release mode"
        echo "  server         Start the web server"
        echo "  cli <args>     Run CLI with arguments"
        echo "  test           Run all tests"
        echo "  audit [path]   Quick audit of path (default: current dir)"
        echo "  tags [path]    Scan for audit tags"
        echo "  static [path]  Run static analysis"
        echo "  tasks [path]   Generate tasks from findings"
        echo "  stats [path]   Show codebase statistics"
        echo "  dev            Start dev server with auto-reload"
        echo "  docker         Build Docker image"
        echo "  clean          Remove build artifacts"
        echo "  install        Install CLI tool globally"
        echo "  check          Run environment checks"
        echo "  help           Show this help message"
        echo ""
        echo "Examples:"
        echo "  ./run.sh build                    # Build project"
        echo "  ./run.sh server                   # Start API server"
        echo "  ./run.sh audit /path/to/repo      # Analyze a repository"
        echo "  ./run.sh cli note add \"text\"      # Add a note"
        echo "  ./run.sh cli --help               # See all CLI options"
        echo "  ./run.sh dev                      # Development mode"
        echo ""
        echo "Environment:"
        echo "  Edit .env to configure API keys and settings"
        echo "  Set RUST_LOG=debug for verbose logging"
        echo ""
        ;;

    *)
        error "Unknown command: $COMMAND"
        echo "Run './run.sh help' for usage information"
        exit 1
        ;;
esac
