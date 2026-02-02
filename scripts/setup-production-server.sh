#!/bin/sh
# Production Server Setup Script for RustAssistant
# This script prepares a fresh Ubuntu/Debian/Raspberry Pi OS server for automated deployments
#
# Usage:
#   chmod +x setup-production-server.sh
#   sudo ./setup-production-server.sh
#
# This script will:
# - Install Docker and dependencies
# - Create/configure 'actions' user for CI/CD
# - Add 'jordan' and 'actions' users to docker group
# - Setup SSH for actions user
# - Optionally run generate-secrets.sh automatically
# - Optimize for Raspberry Pi if detected

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

# Check if running as root
if [ "$(id -u)" -ne 0 ]; then
    log_error "Please run this script with sudo"
    exit 1
fi

log_header "RustAssistant Production Server Setup"

# Detect system information
ARCH=$(uname -m)
IS_PI=false
if [ -f /proc/device-tree/model ]; then
    MODEL=$(cat /proc/device-tree/model 2>/dev/null || echo "")
    if echo "$MODEL" | grep -qi "raspberry"; then
        IS_PI=true
    fi
fi

log_info "System Architecture: $ARCH"
if [ "$IS_PI" = true ]; then
    log_success "Raspberry Pi detected!"
    log_info "Model: $MODEL"
fi

# Detect OS
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS_NAME=$NAME
    OS_VERSION=$VERSION_ID
    log_info "Operating System: $OS_NAME $OS_VERSION"
else
    log_error "Cannot detect OS version"
    exit 1
fi

printf "\n"

# =============================================================================
# Step 1: Update System
# =============================================================================
log_info "Step 1/7: Updating system packages..."

if command -v apt-get >/dev/null 2>&1; then
    # Debian/Ubuntu/Raspberry Pi OS
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get upgrade -y
    apt-get install -y \
        curl \
        wget \
        git \
        ca-certificates \
        gnupg \
        lsb-release \
        ufw \
        htop \
        vim \
        openssh-server
    log_success "System updated (Debian/Ubuntu)"
elif command -v dnf >/dev/null 2>&1; then
    # Fedora/RHEL/CentOS
    dnf update -y
    dnf install -y \
        curl \
        wget \
        git \
        ca-certificates \
        ufw \
        htop \
        vim \
        openssh-server
    log_success "System updated (Fedora/RHEL)"
else
    log_error "Unsupported package manager"
    exit 1
fi

printf "\n"

# =============================================================================
# Step 2: Install Docker
# =============================================================================
log_info "Step 2/7: Installing Docker..."

if command -v docker >/dev/null 2>&1; then
    log_warn "Docker already installed: $(docker --version)"
else
    # Install Docker using official script
    log_info "Downloading Docker installation script..."
    curl -fsSL https://get.docker.com -o /tmp/get-docker.sh

    log_info "Installing Docker..."
    sh /tmp/get-docker.sh

    rm /tmp/get-docker.sh
    log_success "Docker installed: $(docker --version)"
fi

# Start and enable Docker
systemctl start docker
systemctl enable docker
log_success "Docker service started and enabled"

# Install Docker Compose (if not already installed)
if ! command -v docker-compose >/dev/null 2>&1; then
    log_info "Installing Docker Compose plugin..."
    if command -v apt-get >/dev/null 2>&1; then
        apt-get install -y docker-compose-plugin
    elif command -v dnf >/dev/null 2>&1; then
        dnf install -y docker-compose-plugin
    fi
    log_success "Docker Compose installed"
else
    log_info "Docker Compose already installed: $(docker compose version)"
fi

# Raspberry Pi specific optimizations
if [ "$IS_PI" = true ]; then
    log_info "Applying Raspberry Pi optimizations..."

    # Enable cgroup memory (required for Docker)
    if ! grep -q "cgroup_memory=1 cgroup_enable=memory" /boot/cmdline.txt 2>/dev/null; then
        log_info "Enabling cgroup memory support..."
        # Backup cmdline.txt
        cp /boot/cmdline.txt /boot/cmdline.txt.backup
        # Add cgroup parameters
        sed -i '$ s/$/ cgroup_memory=1 cgroup_enable=memory/' /boot/cmdline.txt
        log_warn "Raspberry Pi requires reboot for cgroup changes to take effect"
    fi

    # Increase swap if low memory
    TOTAL_MEM=$(free -m | awk '/^Mem:/{print $2}')
    if [ "$TOTAL_MEM" -lt 2048 ]; then
        log_info "Increasing swap space for low memory system..."
        if [ -f /etc/dphys-swapfile ]; then
            sed -i 's/^CONF_SWAPSIZE=.*/CONF_SWAPSIZE=2048/' /etc/dphys-swapfile
            systemctl restart dphys-swapfile 2>/dev/null || true
            log_success "Swap increased to 2GB"
        fi
    fi
fi

printf "\n"

# =============================================================================
# Step 3: Create 'actions' User
# =============================================================================
log_info "Step 3/7: Creating 'actions' user for CI/CD..."

if id "actions" >/dev/null 2>&1; then
    log_warn "User 'actions' already exists"
else
    useradd -m -s /bin/bash -c "GitHub Actions CI/CD User" actions
    log_success "User 'actions' created"
fi

# Add to docker group
usermod -aG docker actions
log_success "User 'actions' added to docker group"

# Add jordan to docker group if exists
if id "jordan" >/dev/null 2>&1; then
    usermod -aG docker jordan
    log_success "User 'jordan' added to docker group"
fi

# Setup home directory
ACTIONS_HOME="/home/actions"
mkdir -p "$ACTIONS_HOME"
chown actions:actions "$ACTIONS_HOME"
chmod 755 "$ACTIONS_HOME"

# Create project directory
mkdir -p "$ACTIONS_HOME/rustassistant"
chown actions:actions "$ACTIONS_HOME/rustassistant"
log_success "Project directory created: $ACTIONS_HOME/rustassistant"

printf "\n"

# =============================================================================
# Step 4: Configure SSH
# =============================================================================
log_info "Step 4/7: Configuring SSH..."

# Ensure SSH directory exists
SSH_DIR="$ACTIONS_HOME/.ssh"
mkdir -p "$SSH_DIR"
chown actions:actions "$SSH_DIR"
chmod 700 "$SSH_DIR"

# Create authorized_keys if it doesn't exist
if [ ! -f "$SSH_DIR/authorized_keys" ]; then
    touch "$SSH_DIR/authorized_keys"
    chown actions:actions "$SSH_DIR/authorized_keys"
    chmod 600 "$SSH_DIR/authorized_keys"
    log_success "Created authorized_keys file"
fi

# Ensure SSH service is running
systemctl enable ssh 2>/dev/null || systemctl enable sshd 2>/dev/null || true
systemctl start ssh 2>/dev/null || systemctl start sshd 2>/dev/null || true
log_success "SSH service enabled and started"

printf "\n"

# =============================================================================
# Step 5: Configure Firewall
# =============================================================================
log_info "Step 5/7: Configuring firewall..."

if command -v ufw >/dev/null 2>&1; then
    # Reset UFW to default state
    ufw --force reset

    # Set default policies
    ufw default deny incoming
    ufw default allow outgoing

    # Allow SSH (detect current port)
    SSH_PORT=$(grep -E "^Port " /etc/ssh/sshd_config 2>/dev/null | awk '{print $2}')
    if [ -z "$SSH_PORT" ]; then
        SSH_PORT=22
    fi
    ufw allow "$SSH_PORT"/tcp comment 'SSH'

    # Allow Tailscale
    ufw allow in on tailscale0

    # Enable firewall
    ufw --force enable

    log_success "Firewall configured (SSH port $SSH_PORT allowed, Tailscale allowed)"
else
    log_warn "UFW not available, skipping firewall configuration"
fi

printf "\n"

# =============================================================================
# Step 6: Install Tailscale
# =============================================================================
log_info "Step 6/7: Installing Tailscale..."

if command -v tailscale >/dev/null 2>&1; then
    log_warn "Tailscale already installed: $(tailscale version)"

    # Check if connected
    if tailscale status >/dev/null 2>&1; then
        TAILSCALE_IP=$(tailscale ip -4 2>/dev/null || echo "")
        if [ -n "$TAILSCALE_IP" ]; then
            log_success "Tailscale is connected: $TAILSCALE_IP"
        else
            log_warn "Tailscale is installed but not connected"
            log_info "Run: sudo tailscale up"
        fi
    else
        log_warn "Tailscale is installed but not running"
        log_info "Run: sudo tailscale up"
    fi
else
    log_info "Installing Tailscale..."
    curl -fsSL https://tailscale.com/install.sh | sh
    log_success "Tailscale installed"
    log_warn "Run 'sudo tailscale up' to connect to your Tailscale network"
fi

printf "\n"

# =============================================================================
# Step 7: System Optimizations
# =============================================================================
log_info "Step 7/7: Applying system optimizations..."

# Increase file descriptors for Docker
if ! grep -q "fs.file-max" /etc/sysctl.conf; then
    cat >> /etc/sysctl.conf <<EOF

# Docker optimizations
fs.file-max = 65536
fs.inotify.max_user_watches = 524288
EOF
    sysctl -p
    log_success "Increased file descriptor limits"
fi

# Enable Docker log rotation
if [ ! -f /etc/docker/daemon.json ]; then
    mkdir -p /etc/docker
    cat > /etc/docker/daemon.json <<EOF
{
  "log-driver": "json-file",
  "log-opts": {
    "max-size": "10m",
    "max-file": "3"
  }
}
EOF
    systemctl restart docker
    log_success "Docker log rotation configured"
fi

# Set timezone (optional)
if [ -f /etc/timezone ]; then
    CURRENT_TZ=$(cat /etc/timezone)
    log_info "Current timezone: $CURRENT_TZ"
else
    log_info "Timezone not set"
fi

printf "\n"

# =============================================================================
# Generate Secrets
# =============================================================================
log_header "Server Setup Complete!"

printf "${BOLD}Next Steps:${NC}\n\n"

printf "${GREEN}1.${NC} Connect Tailscale:\n"
printf "   ${CYAN}sudo tailscale up${NC}\n\n"

printf "${GREEN}2.${NC} Generate secrets for GitHub Actions:\n"
printf "   ${CYAN}sudo ./generate-secrets.sh${NC}\n\n"

printf "${GREEN}3.${NC} Copy secrets to GitHub:\n"
printf "   Go to: ${CYAN}https://github.com/YOUR_USERNAME/rustassistant/settings/secrets/actions${NC}\n\n"

printf "${GREEN}4.${NC} Required GitHub Secrets:\n"
printf "   • ${YELLOW}PROD_TAILSCALE_IP${NC} - Tailscale IP of this server\n"
printf "   • ${YELLOW}PROD_SSH_KEY${NC} - SSH private key for 'actions' user\n"
printf "   • ${YELLOW}PROD_SSH_PORT${NC} - SSH port (default: 22)\n"
printf "   • ${YELLOW}PROD_SSH_USER${NC} - Username: actions\n"
printf "   • ${YELLOW}TAILSCALE_OAUTH_CLIENT_ID${NC} - From Tailscale admin console\n"
printf "   • ${YELLOW}TAILSCALE_OAUTH_SECRET${NC} - From Tailscale admin console\n"
printf "   • ${YELLOW}DOCKER_USERNAME${NC} - Your Docker Hub username\n"
printf "   • ${YELLOW}DOCKER_TOKEN${NC} - Your Docker Hub access token\n"
printf "   • ${YELLOW}DISCORD_WEBHOOK_ACTIONS${NC} - (Optional) Discord webhook for notifications\n\n"

if [ "$IS_PI" = true ]; then
    printf "${BOLD}${CYAN}Raspberry Pi Specific Notes:${NC}\n\n"
    printf "• ${GREEN}✓${NC} ARM64 architecture detected\n"
    printf "• ${GREEN}✓${NC} Docker will pull ARM64 images automatically\n"
    printf "• ${GREEN}✓${NC} CI/CD builds multi-arch images (amd64 + arm64)\n"

    if grep -q "cgroup_memory=1" /boot/cmdline.txt 2>/dev/null; then
        printf "• ${GREEN}✓${NC} Cgroup memory already enabled\n"
    else
        printf "• ${YELLOW}⚠${NC} ${BOLD}REBOOT REQUIRED${NC} for cgroup memory changes\n"
        printf "  Run: ${CYAN}sudo reboot${NC}\n"
    fi
    printf "\n"
fi

printf "${BOLD}${GREEN}Run generate-secrets.sh now?${NC} (y/N) "
read -r reply

if [ "$reply" = "y" ] || [ "$reply" = "Y" ]; then
    printf "\n"
    SCRIPT_DIR=$(dirname "$0")
    if [ -f "$SCRIPT_DIR/generate-secrets.sh" ]; then
        exec "$SCRIPT_DIR/generate-secrets.sh"
    elif [ -f "./generate-secrets.sh" ]; then
        exec ./generate-secrets.sh
    else
        log_error "generate-secrets.sh not found in current directory"
        log_info "Please run it manually after downloading"
    fi
else
    log_info "Skipped secrets generation"
    printf "\n"
    log_success "Setup complete! Run generate-secrets.sh when ready."
fi

printf "\n"
exit 0
