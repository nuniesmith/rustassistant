#!/bin/bash
# ============================================================================
# Rustassistant Run Script
# ============================================================================
# Handles environment setup and service management
#
# Usage:
#   ./run.sh                    # Interactive mode (asks for missing values)
#   ./run.sh --non-interactive  # CI/CD mode (uses env vars or defaults)
#   ./run.sh build              # Build containers
#   ./run.sh up                 # Start services
#   ./run.sh down               # Stop services
#   ./run.sh logs               # View logs
#   ./run.sh clean              # Clean up containers and volumes

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
ENV_FILE=".env"
COMPOSE_FILE="docker-compose.yml"
INTERACTIVE=true

# Parse arguments
for arg in "$@"; do
    case $arg in
        --non-interactive)
            INTERACTIVE=false
            shift
            ;;
        --prod|--production)
            COMPOSE_FILE="docker-compose.prod.yml"
            shift
            ;;
        build|up|down|start|stop|pull|logs|clean|restart|status)
            COMMAND=$arg
            shift
            ;;
    esac
done

# ============================================================================
# Helper Functions
# ============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

generate_secret() {
    openssl rand -hex 32 2>/dev/null || cat /dev/urandom | tr -dc 'a-zA-Z0-9' | fold -w 64 | head -n 1
}

# ============================================================================
# Environment Setup
# ============================================================================

setup_env() {
    log_info "Setting up environment..."

    # Check if .env exists
    if [ -f "$ENV_FILE" ]; then
        log_info "Found existing $ENV_FILE"
        source "$ENV_FILE"

        # Check if XAI_API_KEY is set
        if [ -z "$XAI_API_KEY" ]; then
            if [ "$INTERACTIVE" = true ]; then
                read -p "XAI API Key is missing. Enter your XAI API key: " XAI_API_KEY
                if [ -n "$XAI_API_KEY" ]; then
                    echo "XAI_API_KEY=$XAI_API_KEY" >> "$ENV_FILE"
                    log_success "Added XAI_API_KEY to $ENV_FILE"
                fi
            else
                log_warning "XAI_API_KEY not set. Server will start but API calls will fail."
            fi
        fi
    else
        log_info "Creating new $ENV_FILE..."

        # Get XAI API Key and GitHub Token
        if [ "$INTERACTIVE" = true ]; then
            echo ""
            echo "╔════════════════════════════════════════════════════════════════╗"
            echo "║           Rustassistant Environment Configuration             ║"
            echo "╚════════════════════════════════════════════════════════════════╝"
            echo ""
            read -p "Enter your XAI API key (or press Enter to skip): " XAI_API_KEY
            read -p "Enter your GitHub token (or press Enter to skip): " GITHUB_TOKEN
            echo ""
        else
            # In non-interactive mode, use environment variable or empty
            XAI_API_KEY="${XAI_API_KEY:-}"
            GITHUB_TOKEN="${GITHUB_TOKEN:-}"
        fi

        # Generate secrets
        log_info "Generating secure random secrets..."
        DB_ENCRYPTION_KEY=$(generate_secret)
        SESSION_SECRET=$(generate_secret)
        REDIS_PASSWORD=$(generate_secret)

        # Create .env file
        cat > "$ENV_FILE" << EOF
# ============================================================================
# Rustassistant Environment Configuration
# ============================================================================
# Generated on $(date)

# ----------------------------------------------------------------------------
# API Keys
# ----------------------------------------------------------------------------
XAI_API_KEY=${XAI_API_KEY}
XAI_BASE_URL=https://api.x.ai/v1

# ----------------------------------------------------------------------------
# GitHub Integration
# ----------------------------------------------------------------------------
GITHUB_TOKEN=${GITHUB_TOKEN}

# ----------------------------------------------------------------------------
# Database
# ----------------------------------------------------------------------------
DATABASE_URL=sqlite:/home/jordan/github/rustassistant/data/rustassistant.db
DB_ENCRYPTION_KEY=${DB_ENCRYPTION_KEY}

# ----------------------------------------------------------------------------
# Server Configuration
# ----------------------------------------------------------------------------
HOST=127.0.0.1
PORT=3000
RUST_LOG=info,rustassistant=debug

# ----------------------------------------------------------------------------
# Security
# ----------------------------------------------------------------------------
SESSION_SECRET=${SESSION_SECRET}

# ----------------------------------------------------------------------------
# Redis Cache (Optional)
# ----------------------------------------------------------------------------
REDIS_URL=redis://:${REDIS_PASSWORD}@localhost:6379
REDIS_PASSWORD=${REDIS_PASSWORD}

# ----------------------------------------------------------------------------
# Docker Configuration
# ----------------------------------------------------------------------------
COMPOSE_PROJECT_NAME=rustassistant
DOCKER_BUILDKIT=1
EOF

        log_success "Created $ENV_FILE with secure random secrets"

        if [ "$INTERACTIVE" = true ]; then
            echo ""
            log_info "Environment file created at: $ENV_FILE"
            if [ -z "$XAI_API_KEY" ]; then
                log_warning "XAI_API_KEY is not set. Add it to $ENV_FILE before using LLM features."
            fi
            echo ""
        fi
    fi

    # Create data directory if it doesn't exist
    mkdir -p data
    log_success "Data directory ready"
}

# ============================================================================
# Docker Commands
# ============================================================================

docker_build() {
    log_info "Building Docker containers..."
    docker compose -f "$COMPOSE_FILE" build
    log_success "Build complete"
}

docker_pull() {
    log_info "Pulling Docker images..."
    docker compose -f "$COMPOSE_FILE" pull
    log_success "Pull complete"
}

docker_up() {
    log_info "Starting services..."
    docker compose -f "$COMPOSE_FILE" up -d --remove-orphans
    log_success "Services started"
    docker compose -f "$COMPOSE_FILE" ps
    echo ""
    log_info "API server available at: http://localhost:${PORT:-3000}"
    log_info "Health check: curl http://localhost:${PORT:-3000}/health"
}

docker_start() {
    log_info "Starting services (pull + up)..."
    docker_pull
    docker compose -f "$COMPOSE_FILE" up -d --remove-orphans
    log_success "Services started"
    docker compose -f "$COMPOSE_FILE" ps
}

docker_down() {
    log_info "Stopping services..."
    docker compose -f "$COMPOSE_FILE" down
    log_success "Services stopped"
}

docker_stop() {
    docker_down
}

docker_logs() {
    log_info "Showing logs (Ctrl+C to exit)..."
    docker compose -f "$COMPOSE_FILE" logs -f
}

docker_restart() {
    log_info "Restarting services..."
    docker compose -f "$COMPOSE_FILE" restart
    log_success "Services restarted"
}

docker_status() {
    log_info "Service status:"
    docker compose -f "$COMPOSE_FILE" ps
}

docker_clean() {
    log_warning "This will remove all containers, networks, and volumes."
    if [ "$INTERACTIVE" = true ]; then
        read -p "Are you sure? (yes/no): " confirm
        if [ "$confirm" != "yes" ]; then
            log_info "Cancelled"
            exit 0
        fi
    fi

    log_info "Cleaning up..."
    docker compose -f "$COMPOSE_FILE" down -v --remove-orphans
    log_success "Cleanup complete"
}

# ============================================================================
# CLI Usage
# ============================================================================

show_usage() {
    cat << EOF

╔════════════════════════════════════════════════════════════════╗
║                    Rustassistant Run Script                    ║
╚════════════════════════════════════════════════════════════════╝

Usage: ./run.sh [OPTIONS] [COMMAND]

OPTIONS:
    --non-interactive    Run in non-interactive mode (CI/CD)
    --prod, --production Use production docker-compose.prod.yml

COMMANDS:
    build       Build Docker containers
    pull        Pull Docker images from registry
    up          Start services in detached mode
    start       Pull images and start services (production)
    down        Stop services
    stop        Stop services (alias for down)
    logs        Show and follow service logs
    restart     Restart all services
    status      Show service status
    clean       Remove containers, networks, and volumes

EXAMPLES:
    # First time setup (interactive)
    ./run.sh

    # Start services (development)
    ./run.sh up

    # Production deployment (pull and start)
    ./run.sh --prod start

    # CI/CD mode
    XAI_API_KEY=\${{ secrets.XAI_API_KEY }} ./run.sh --non-interactive --prod start

    # View logs
    ./run.sh logs

    # Stop everything
    ./run.sh down

ENVIRONMENT VARIABLES (non-interactive mode):
    XAI_API_KEY         Your XAI API key (required for LLM features)
    PORT                Server port (default: 3000)
    RUST_LOG            Log level (default: info,rustassistant=debug)

EOF
}

# ============================================================================
# Main Execution
# ============================================================================

main() {
    echo ""
    echo "╔════════════════════════════════════════════════════════════════╗"
    echo "║                      Rustassistant                             ║"
    echo "╚════════════════════════════════════════════════════════════════╝"
    echo ""

    # Show which compose file we're using
    if [ "$COMPOSE_FILE" = "docker-compose.prod.yml" ]; then
        log_info "Using production configuration: $COMPOSE_FILE"
    else
        log_info "Using development configuration: $COMPOSE_FILE"
    fi
    echo ""

    # Setup environment
    setup_env

    # Execute command
    case "${COMMAND:-up}" in
        build)
            docker_build
            ;;
        pull)
            docker_pull
            ;;
        up)
            docker_build
            docker_up
            ;;
        start)
            docker_start
            ;;
        down|stop)
            docker_down
            ;;
        logs)
            docker_logs
            ;;
        restart)
            docker_restart
            ;;
        status)
            docker_status
            ;;
        clean)
            docker_clean
            ;;
        help|--help|-h)
            show_usage
            ;;
        *)
            # Default: start services
            docker_build
            docker_up
            ;;
    esac

    echo ""
}

# Run main function
main "$@"
