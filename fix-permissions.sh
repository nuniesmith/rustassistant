#!/bin/bash
# ============================================================================
# RustAssistant - Quick Fix for Database Permissions
# ============================================================================
# Fixes the "unable to open database file" error on Raspberry Pi
#
# Usage:
#   chmod +x fix-permissions.sh
#   ./fix-permissions.sh
# ============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo ""
echo "╔════════════════════════════════════════════════════════════════╗"
echo "║         RustAssistant - Database Permissions Fix              ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""

# Check if we're in the right directory
if [ ! -f "docker-compose.prod.yml" ]; then
    echo -e "${RED}Error: docker-compose.prod.yml not found${NC}"
    echo "Please run this script from the rustassistant directory"
    echo ""
    echo "Example:"
    echo "  cd ~/rustassistant"
    echo "  ./fix-permissions.sh"
    exit 1
fi

echo -e "${BLUE}[1/5]${NC} Stopping containers..."
docker compose -f docker-compose.prod.yml down 2>/dev/null || true
echo -e "${GREEN}✓${NC} Containers stopped"
echo ""

echo -e "${BLUE}[2/5]${NC} Creating required directories..."
mkdir -p data config
echo -e "${GREEN}✓${NC} Directories created"
echo ""

echo -e "${BLUE}[3/5]${NC} Setting permissions..."
# Get current user's UID and GID
USER_UID=$(id -u)
USER_GID=$(id -g)

# Set ownership
chown -R ${USER_UID}:${USER_GID} data config
chmod -R 755 data config

echo -e "${GREEN}✓${NC} Permissions set"
echo "  - Owner: ${USER_UID}:${USER_GID}"
echo "  - Permissions: 755"
echo ""

echo -e "${BLUE}[4/5]${NC} Checking .env file..."
if [ ! -f ".env" ]; then
    echo -e "${YELLOW}!${NC} .env file not found, creating default..."
    cat > .env <<EOF
# RustAssistant Environment Configuration
RUST_LOG=info
XAI_API_KEY=
EOF
    chmod 600 .env
    echo -e "${GREEN}✓${NC} Created default .env file"
    echo -e "${YELLOW}!${NC} Remember to add your XAI_API_KEY to .env"
else
    echo -e "${GREEN}✓${NC} .env file exists"
fi
echo ""

echo -e "${BLUE}[5/5]${NC} Starting containers..."
docker compose -f docker-compose.prod.yml up -d
echo -e "${GREEN}✓${NC} Containers started"
echo ""

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║                    Fix Complete!                               ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""
echo "Checking container status..."
echo ""
docker compose -f docker-compose.prod.yml ps
echo ""

echo "Waiting 5 seconds for containers to initialize..."
sleep 5
echo ""

echo "Recent logs from rustassistant-web:"
echo "─────────────────────────────────────────────────────────────────"
docker logs rustassistant-web --tail=10
echo "─────────────────────────────────────────────────────────────────"
echo ""

# Check if container is running
if docker ps | grep -q rustassistant-web; then
    STATUS=$(docker inspect rustassistant-web --format='{{.State.Status}}')
    if [ "$STATUS" = "running" ]; then
        echo -e "${GREEN}✅ Container is running successfully!${NC}"
        echo ""
        echo "Next steps:"
        echo "  - Check logs: docker logs rustassistant-web -f"
        echo "  - Access UI: http://localhost:3001"
        echo "  - Check health: curl http://localhost:3001/"
    else
        echo -e "${YELLOW}⚠️  Container status: $STATUS${NC}"
        echo "Check logs with: docker logs rustassistant-web"
    fi
else
    echo -e "${RED}❌ Container is not running${NC}"
    echo "Check logs with: docker logs rustassistant-web"
fi
echo ""
