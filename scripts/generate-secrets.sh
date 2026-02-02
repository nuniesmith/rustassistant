#!/bin/sh
# Secrets Generation Script for RustAssistant
# This script generates secure credentials for GitHub Actions and production deployment
#
# Usage:
#   chmod +x generate-secrets.sh
#   sudo ./generate-secrets.sh
#
# This script will:
# - Generate SSH keys for the actions user
# - Detect Tailscale IP address
# - Generate secure passwords and API keys
# - Create a credentials file for GitHub Secrets with correct naming
# - Optionally append app secrets to .env file

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# Log functions
log_info() { printf "${BLUE}[INFO]${NC} %s\n" "$*"; }
log_success() { printf "${GREEN}[SUCCESS]${NC} %s\n" "$*"; }
log_warn() { printf "${YELLOW}[WARN]${NC} %s\n" "$*"; }
log_error() { printf "${RED}[ERROR]${NC} %s\n" "$*"; }
log_header() {
    printf "\n"
    printf "${BOLD}${CYAN}================================================================================${NC}\n"
    printf "${BOLD}${CYAN}  %s${NC}\n" "$*"
    printf "${BOLD}${CYAN}================================================================================${NC}\n"
    printf "\n"
}

# Function to generate secure password
generate_password() {
    openssl rand -base64 24 | tr -d "=+/" | cut -c1-24
}

# Function to generate hex secret (for API keys, encryption keys, etc.)
generate_hex_secret() {
    local length="${1:-32}"
    openssl rand -hex "$length"
}

# Function to generate base64 secret
generate_base64_secret() {
    local length="${1:-32}"
    openssl rand -base64 "$length" | tr -d "=+/" | head -c "$length"
}

# Check if running as root
if [ "$(id -u)" -ne 0 ]; then
    log_error "Please run this script with sudo"
    exit 1
fi

# Check if openssl is available
if ! command -v openssl >/dev/null 2>&1; then
    log_error "OpenSSL is not installed. Please install it first."
    exit 1
fi

ACTIONS_HOME="/home/actions"

# Check if actions user exists
if ! id "actions" >/dev/null 2>&1; then
    log_error "User 'actions' does not exist. Please run setup-production-server.sh first."
    exit 1
fi

log_header "RustAssistant Secrets Generation"

log_info "Generating secure credentials for RustAssistant deployment..."
printf "\n"

# =============================================================================
# Step 1: Detect Server Information
# =============================================================================
log_info "Step 1/4: Detecting server information..."

# Get server IP
SERVER_IP=$(hostname -I 2>/dev/null | awk '{print $1}' || echo "unknown")
log_info "Server IP: $SERVER_IP"

# Detect Tailscale IP if available
TAILSCALE_IP=""
if command -v tailscale >/dev/null 2>&1; then
    TAILSCALE_IP=$(tailscale ip -4 2>/dev/null || echo "")
    if [ -n "$TAILSCALE_IP" ]; then
        log_success "Tailscale IP detected: $TAILSCALE_IP"
    else
        log_warn "Tailscale is installed but not connected"
        log_warn "Run 'sudo tailscale up' to connect to your tailnet"
    fi
else
    log_warn "Tailscale not installed"
fi

# Detect SSH port
SSH_PORT=$(grep -E "^Port " /etc/ssh/sshd_config 2>/dev/null | awk '{print $2}')
if [ -z "$SSH_PORT" ]; then
    SSH_PORT=22
fi
log_info "SSH Port: $SSH_PORT"

# Detect hostname
HOSTNAME=$(hostname 2>/dev/null || echo "server")
log_info "Hostname: $HOSTNAME"

# Detect architecture (important for Raspberry Pi)
ARCH=$(uname -m)
log_info "Architecture: $ARCH"
if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    log_success "ARM64 architecture detected - perfect for Raspberry Pi!"
fi

printf "\n"

# =============================================================================
# Step 2: Generate SSH Keys for Actions User
# =============================================================================
log_info "Step 2/4: Generating SSH keys for 'actions' user..."

SSH_DIR="$ACTIONS_HOME/.ssh"

# Ensure .ssh directory exists
sudo -u actions mkdir -p "$SSH_DIR"
chmod 700 "$SSH_DIR"

REGENERATE_KEY=false
if [ -f "$SSH_DIR/id_ed25519" ]; then
    log_warn "SSH key already exists for actions user"
    printf "Do you want to regenerate it? This will invalidate the old key. (y/N) "
    read -r reply
    if [ "$reply" = "y" ] || [ "$reply" = "Y" ]; then
        REGENERATE_KEY=true
        sudo -u actions rm -f "$SSH_DIR/id_ed25519" "$SSH_DIR/id_ed25519.pub"
    else
        log_info "Using existing SSH key"
    fi
fi

if [ ! -f "$SSH_DIR/id_ed25519" ]; then
    sudo -u actions ssh-keygen -t ed25519 -f "$SSH_DIR/id_ed25519" -N "" -C "actions@$HOSTNAME-$(date +%Y%m%d)"
    chmod 600 "$SSH_DIR/id_ed25519"
    chmod 644 "$SSH_DIR/id_ed25519.pub"
    log_success "SSH key generated"
fi

# Add public key to authorized_keys
if [ ! -f "$SSH_DIR/authorized_keys" ]; then
    sudo -u actions touch "$SSH_DIR/authorized_keys"
    chmod 600 "$SSH_DIR/authorized_keys"
fi

# Add key to authorized_keys if not already present
PUB_KEY=$(cat "$SSH_DIR/id_ed25519.pub")
if ! grep -qF "$PUB_KEY" "$SSH_DIR/authorized_keys" 2>/dev/null; then
    echo "$PUB_KEY" >> "$SSH_DIR/authorized_keys"
    log_success "Public key added to authorized_keys"
else
    log_info "Public key already in authorized_keys"
fi

printf "\n"

# =============================================================================
# Step 3: Generate Application Secrets
# =============================================================================
log_info "Step 3/4: Generating application secrets..."

# Generate RustAssistant-specific secrets
RUSTASSISTANT_API_KEY=$(generate_base64_secret 32)
JWT_SECRET=$(generate_hex_secret 64)
ENCRYPTION_KEY=$(generate_hex_secret 32)
SESSION_SECRET=$(generate_hex_secret 32)
ADMIN_PASSWORD=$(generate_password)

log_success "Application secrets generated"
printf "\n"

# =============================================================================
# Step 4: Output Credentials
# =============================================================================
log_info "Step 4/4: Saving credentials..."

# Read SSH private key
SSH_PRIVATE_KEY=$(cat "$SSH_DIR/id_ed25519")
SSH_PUBLIC_KEY=$(cat "$SSH_DIR/id_ed25519.pub")

# Create credentials file
CREDENTIALS_FILE="/tmp/rustassistant_credentials_$(date +%s).txt"
touch "$CREDENTIALS_FILE"
chmod 600 "$CREDENTIALS_FILE"

cat > "$CREDENTIALS_FILE" <<EOF
# =============================================================================
# RustAssistant Server Credentials
# Generated: $(date)
# Hostname: $HOSTNAME
# Architecture: $ARCH
# Server IP: $SERVER_IP
# Tailscale IP: ${TAILSCALE_IP:-Not configured}
# SSH Port: $SSH_PORT
# =============================================================================

# =============================================================================
# SSH & DEPLOYMENT SECRETS
# =============================================================================

# Tailscale IP address (use this for SSH connections)
PROD_TAILSCALE_IP=${TAILSCALE_IP:-CONFIGURE_TAILSCALE_FIRST}

# SSH port
PROD_SSH_PORT=$SSH_PORT

# SSH username for deployments
PROD_SSH_USER=actions

# SSH Private Key (copy entire block including BEGIN and END lines)
PROD_SSH_KEY:
$SSH_PRIVATE_KEY

# SSH Public Key (add to any servers you need to access FROM this server)
SSH_PUBLIC_KEY:
$SSH_PUBLIC_KEY

# =============================================================================
# RUSTASSISTANT APPLICATION SECRETS
# =============================================================================

RUSTASSISTANT_API_KEY=$RUSTASSISTANT_API_KEY
JWT_SECRET=$JWT_SECRET
ENCRYPTION_KEY=$ENCRYPTION_KEY
SESSION_SECRET=$SESSION_SECRET
ADMIN_PASSWORD=$ADMIN_PASSWORD

# =============================================================================
# GITHUB SECRETS FORMAT
# =============================================================================
# Copy these to your GitHub repository secrets:
# https://github.com/YOUR_USERNAME/YOUR_REPO/settings/secrets/actions
#
# SECRET NAME              | VALUE
# -------------------------|--------------------------------------------------
# PROD_TAILSCALE_IP        | ${TAILSCALE_IP:-YOUR_TAILSCALE_IP}
# PROD_SSH_KEY             | (entire SSH_PRIVATE_KEY above)
# PROD_SSH_PORT            | $SSH_PORT
# PROD_SSH_USER            | actions
# DISCORD_WEBHOOK_ACTIONS  | (your Discord webhook URL for CI/CD notifications)
# DOCKER_USERNAME          | (your Docker Hub username)
# DOCKER_TOKEN             | (your Docker Hub access token)
# RUSTASSISTANT_API_KEY    | $RUSTASSISTANT_API_KEY
# =============================================================================
#
# REQUIRED for CI/CD:
#   - PROD_TAILSCALE_IP, PROD_SSH_KEY, PROD_SSH_PORT, PROD_SSH_USER
#   - TAILSCALE_OAUTH_CLIENT_ID, TAILSCALE_OAUTH_SECRET
#   - DOCKER_USERNAME, DOCKER_TOKEN
#
# OPTIONAL:
#   - DISCORD_WEBHOOK_ACTIONS (for Discord notifications)
#   - RUSTASSISTANT_API_KEY (if your app needs an API key)
# =============================================================================

EOF

log_success "Credentials saved to: $CREDENTIALS_FILE"
printf "\n"

# =============================================================================
# Summary and Instructions
# =============================================================================
log_header "Credentials Generated Successfully!"

printf "${BOLD}${YELLOW}⚠  IMPORTANT: Secure the credentials file${NC}\n"
printf "   Location: ${CYAN}%s${NC}\n\n" "$CREDENTIALS_FILE"

printf "${BOLD}${GREEN}View credentials:${NC}\n"
printf "   ${CYAN}cat %s${NC}\n\n" "$CREDENTIALS_FILE"

log_header "GitHub Secrets Setup"

printf "${BOLD}1. Go to your GitHub repository settings:${NC}\n"
printf "   ${CYAN}https://github.com/YOUR_USERNAME/rustassistant/settings/secrets/actions${NC}\n\n"

printf "${BOLD}2. Add these REQUIRED secrets for production deployment:${NC}\n\n"

printf "   ${YELLOW}PROD_TAILSCALE_IP${NC}\n"
printf "   Value: ${CYAN}%s${NC}\n\n" "${TAILSCALE_IP:-⚠️ CONFIGURE TAILSCALE FIRST}"

printf "   ${YELLOW}PROD_SSH_KEY${NC}\n"
printf "   Value: (entire private key from credentials file)\n\n"

printf "   ${YELLOW}PROD_SSH_PORT${NC}\n"
printf "   Value: ${CYAN}%s${NC}\n\n" "$SSH_PORT"

printf "   ${YELLOW}PROD_SSH_USER${NC}\n"
printf "   Value: ${CYAN}actions${NC}\n\n"

printf "   ${YELLOW}DOCKER_USERNAME${NC}\n"
printf "   Value: (your Docker Hub username)\n\n"

printf "   ${YELLOW}DOCKER_TOKEN${NC}\n"
printf "   Value: (your Docker Hub access token)\n\n"

printf "   ${YELLOW}TAILSCALE_OAUTH_CLIENT_ID${NC}\n"
printf "   Value: (from Tailscale Admin Console)\n\n"

printf "   ${YELLOW}TAILSCALE_OAUTH_SECRET${NC}\n"
printf "   Value: (from Tailscale Admin Console)\n\n"

printf "${BOLD}3. Add these OPTIONAL secrets:${NC}\n\n"

printf "   ${YELLOW}DISCORD_WEBHOOK_ACTIONS${NC}\n"
printf "   Value: (Discord webhook URL for CI/CD notifications)\n\n"

printf "   ${YELLOW}RUSTASSISTANT_API_KEY${NC}\n"
printf "   Value: ${CYAN}%s${NC}\n\n" "$RUSTASSISTANT_API_KEY"

log_header "Quick Commands"

printf "View full credentials file:\n"
printf "   ${CYAN}cat %s${NC}\n\n" "$CREDENTIALS_FILE"

printf "View SSH private key for GitHub:\n"
printf "   ${CYAN}cat %s/.ssh/id_ed25519${NC}\n\n" "$ACTIONS_HOME"

printf "View SSH public key:\n"
printf "   ${CYAN}cat %s/.ssh/id_ed25519.pub${NC}\n\n" "$ACTIONS_HOME"

printf "Get Tailscale IP:\n"
printf "   ${CYAN}tailscale ip -4${NC}\n\n"

printf "Test SSH connection (from another machine via Tailscale):\n"
printf "   ${CYAN}ssh -p %s actions@%s${NC}\n\n" "$SSH_PORT" "${TAILSCALE_IP:-TAILSCALE_IP}"

printf "Check Docker architecture (should be arm64/aarch64 on Raspberry Pi):\n"
printf "   ${CYAN}docker info | grep Architecture${NC}\n\n"

log_header "Raspberry Pi Deployment Notes"

if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    printf "${GREEN}✓${NC} ARM64 architecture detected - perfect for Raspberry Pi!\n"
    printf "${GREEN}✓${NC} The CI/CD pipeline will build multi-arch images (amd64 + arm64)\n"
    printf "${GREEN}✓${NC} Docker will automatically pull the correct ARM64 image\n"
else
    printf "${YELLOW}⚠${NC} This doesn't appear to be an ARM64 system\n"
    printf "   Current architecture: ${CYAN}%s${NC}\n" "$ARCH"
    printf "   The CI/CD pipeline will still build ARM64 images for Raspberry Pi\n"
fi
printf "\n"

log_header "Security Best Practices"

printf "${YELLOW}✓${NC} Keep the credentials file secure\n"
printf "${YELLOW}✓${NC} Delete the credentials file after copying to GitHub Secrets:\n"
printf "   ${CYAN}sudo rm %s${NC}\n" "$CREDENTIALS_FILE"
printf "${YELLOW}✓${NC} Never commit secrets to version control\n"
printf "${YELLOW}✓${NC} Rotate secrets periodically\n"
printf "${YELLOW}✓${NC} Use Tailscale VPN for secure SSH access (no public exposure)\n"
printf "${YELLOW}✓${NC} Keep your Raspberry Pi and Docker images up to date\n"
printf "\n"

log_header "Next Steps"

printf "1. ${CYAN}Copy secrets to GitHub${NC}\n"
printf "2. ${CYAN}Push your code to trigger the CI/CD pipeline${NC}\n"
printf "3. ${CYAN}Watch the deployment in GitHub Actions${NC}\n"
printf "4. ${CYAN}Monitor via Discord notifications (if configured)${NC}\n"
printf "5. ${CYAN}Delete this credentials file when done${NC}\n"
printf "\n"

log_success "Secrets generation complete!"
printf "\n"

exit 0
