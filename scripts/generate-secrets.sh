#!/usr/bin/env bash
# =============================================================================
# RustAssistant — .env Secrets Generator (Oryx Server)
# =============================================================================
#
# Generates a .env file with cryptographically random passwords for Postgres
# and Redis, and injects API keys from arguments, environment variables, or
# interactive prompts.
#
# Usage:
#   # Fully automated (CI/CD — secrets from env vars):
#   XAI_API_KEY=xai-... GITHUB_TOKEN=ghp_... ./scripts/generate-secrets.sh
#
#   # Interactive (prompts for missing keys):
#   ./scripts/generate-secrets.sh
#
#   # Explicit arguments override everything:
#   ./scripts/generate-secrets.sh \
#       --xai-key xai-abc123 \
#       --github-token ghp_xyz \
#       --repos-path /home/jordan/repos \
#       --force
#
# The generated .env file is written to the project root (next to
# docker-compose.yml). If a .env already exists, the script will refuse
# to overwrite it unless --force is passed.
#
# Port assignments (avoid FKS stack conflicts on oryx):
#   RA_PORT=3500          RustAssistant web UI + API
#   RA_POSTGRES_PORT=5433 PostgreSQL (FKS uses 5432)
#   RA_REDIS_PORT=6380    Redis      (FKS uses 6379)
#   RA_OLLAMA_PORT=11434  Ollama     (no conflict)
# =============================================================================

set -euo pipefail

# ── Colours ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()    { printf "${CYAN}[info]${NC}  %s\n" "$*"; }
ok()      { printf "${GREEN}[ok]${NC}    %s\n" "$*"; }
warn()    { printf "${YELLOW}[warn]${NC}  %s\n" "$*"; }
err()     { printf "${RED}[error]${NC} %s\n" "$*" >&2; }

# ── Resolve project root (directory containing docker-compose.yml) ───────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_FILE="$PROJECT_ROOT/.env"

# ── Defaults ─────────────────────────────────────────────────────────────────
FORCE=false
ARG_XAI_KEY=""
ARG_GITHUB_TOKEN=""
ARG_REPOS_PATH=""
ARG_GITHUB_OWNER=""
ARG_REMOTE_MODEL=""
ARG_LOCAL_MODEL=""

# ── Parse arguments ──────────────────────────────────────────────────────────
while [ $# -gt 0 ]; do
    case "$1" in
        --xai-key)         ARG_XAI_KEY="$2";        shift 2 ;;
        --github-token)    ARG_GITHUB_TOKEN="$2";    shift 2 ;;
        --repos-path)      ARG_REPOS_PATH="$2";      shift 2 ;;
        --github-owner)    ARG_GITHUB_OWNER="$2";    shift 2 ;;
        --remote-model)    ARG_REMOTE_MODEL="$2";    shift 2 ;;
        --local-model)     ARG_LOCAL_MODEL="$2";     shift 2 ;;
        --force|-f)        FORCE=true;               shift   ;;
        --help|-h)
            sed -n '2,/^# ====/{ /^# ====/d; s/^# \{0,1\}//; p; }' "$0"
            exit 0
            ;;
        *)
            err "Unknown option: $1"
            exit 1
            ;;
    esac
done

# ── Pre-flight checks ───────────────────────────────────────────────────────
if [ -f "$ENV_FILE" ] && [ "$FORCE" = false ]; then
    err ".env already exists at $ENV_FILE"
    err "Pass --force to overwrite, or delete it manually."
    exit 1
fi

if ! command -v openssl >/dev/null 2>&1; then
    err "openssl is required but not found. Install it first."
    exit 1
fi

# ── Helper: generate a random alphanumeric password ──────────────────────────
gen_password() {
    local length="${1:-32}"
    openssl rand -base64 48 | tr -d '/+=\n' | head -c "$length"
}

# ── Helper: resolve a value from arg → env var → interactive prompt ──────────
resolve_secret() {
    local arg_val="$1"
    local env_name="$2"
    local prompt_text="$3"
    local allow_empty="${4:-false}"

    # 1. Explicit argument
    if [ -n "$arg_val" ]; then
        printf '%s' "$arg_val"
        return
    fi

    # 2. Environment variable
    local env_val="${!env_name:-}"
    if [ -n "$env_val" ]; then
        printf '%s' "$env_val"
        return
    fi

    # 3. Interactive prompt (only if stdin is a terminal)
    if [ -t 0 ]; then
        printf "${YELLOW}?${NC} %s " "$prompt_text" >&2
        local reply=""
        read -r reply
        if [ -n "$reply" ]; then
            printf '%s' "$reply"
            return
        fi
    fi

    # 4. Empty is acceptable for optional secrets
    if [ "$allow_empty" = "true" ]; then
        printf ''
        return
    fi

    warn "$env_name not set — leaving blank (set it later in .env)"
    printf ''
}

# ── Generate secrets ─────────────────────────────────────────────────────────
info "Generating secrets for RustAssistant on oryx..."
echo

RA_POSTGRES_PASSWORD="$(gen_password 32)"
RA_REDIS_PASSWORD="$(gen_password 32)"
RA_SESSION_SECRET="$(openssl rand -hex 32)"
RA_ENCRYPTION_KEY="$(openssl rand -hex 32)"

ok "Postgres password generated (32 chars)"
ok "Redis password generated    (32 chars)"
ok "Session secret generated    (64 hex)"
ok "Encryption key generated    (64 hex)"
echo

# ── Resolve API keys ────────────────────────────────────────────────────────
XAI_KEY="$(resolve_secret "$ARG_XAI_KEY" "XAI_API_KEY" \
    "Enter xAI (Grok) API key [leave blank to skip]:" "true")"
if [ -n "$XAI_KEY" ]; then
    ok "Grok API key configured"
else
    warn "XAI_API_KEY not set — Grok features will be disabled"
fi

GITHUB_TOKEN="$(resolve_secret "$ARG_GITHUB_TOKEN" "GITHUB_TOKEN" \
    "Enter GitHub token (PAT) [leave blank to skip]:" "true")"
if [ -n "$GITHUB_TOKEN" ]; then
    ok "GitHub token configured"
else
    warn "GITHUB_TOKEN not set — repo sync from GitHub will be disabled"
fi

GITHUB_OWNER="$(resolve_secret "$ARG_GITHUB_OWNER" "GITHUB_OWNER" \
    "Enter GitHub owner/org [nuniesmith]:" "true")"
GITHUB_OWNER="${GITHUB_OWNER:-nuniesmith}"

REPOS_PATH="$(resolve_secret "$ARG_REPOS_PATH" "REPOS_BASE_PATH" \
    "Enter host repos mount path [/home/jordan/repos]:" "true")"
REPOS_PATH="${REPOS_PATH:-/home/jordan/repos}"

REMOTE_MODEL="$(resolve_secret "$ARG_REMOTE_MODEL" "REMOTE_MODEL" "" "true")"
REMOTE_MODEL="${REMOTE_MODEL:-grok-4-1-fast-reasoning}"

LOCAL_MODEL="$(resolve_secret "$ARG_LOCAL_MODEL" "LOCAL_MODEL" "" "true")"
LOCAL_MODEL="${LOCAL_MODEL:-qwen2.5-coder:7b}"

echo

# ── Write .env ───────────────────────────────────────────────────────────────
info "Writing $ENV_FILE ..."

cat > "$ENV_FILE" <<ENVFILE
# =============================================================================
# RustAssistant — Environment Configuration (Oryx Server)
# =============================================================================
# Auto-generated by scripts/generate-secrets.sh on $(date -u '+%Y-%m-%d %H:%M:%S UTC')
#
# Port mapping (avoids FKS stack conflicts):
#   3500  → RustAssistant web + API
#   5433  → PostgreSQL
#   6380  → Redis
#   11434 → Ollama
# =============================================================================

# ── Ports ────────────────────────────────────────────────────────────────────
RA_PORT=3500
RA_POSTGRES_PORT=5433
RA_REDIS_PORT=6380
RA_OLLAMA_PORT=11434

# ── Generated Secrets (DO NOT COMMIT) ────────────────────────────────────────
RA_POSTGRES_PASSWORD=${RA_POSTGRES_PASSWORD}
RA_REDIS_PASSWORD=${RA_REDIS_PASSWORD}
RA_SESSION_SECRET=${RA_SESSION_SECRET}
RA_ENCRYPTION_KEY=${RA_ENCRYPTION_KEY}

# ── AI Provider Keys ────────────────────────────────────────────────────────
XAI_API_KEY=${XAI_KEY}
XAI_BASE_URL=https://api.x.ai/v1
REMOTE_MODEL=${REMOTE_MODEL}
LOCAL_MODEL=${LOCAL_MODEL}
FORCE_REMOTE_MODEL=false

# ── GitHub ───────────────────────────────────────────────────────────────────
GITHUB_TOKEN=${GITHUB_TOKEN}
GITHUB_OWNER=${GITHUB_OWNER}

# ── Repo Mounting ────────────────────────────────────────────────────────────
REPOS_BASE_PATH=${REPOS_PATH}

# ── Scanner ──────────────────────────────────────────────────────────────────
AUTO_SCAN_ENABLED=true
AUTO_SCAN_INTERVAL=60
AUTO_SCAN_MAX_CONCURRENT=3
REPO_SYNC_INTERVAL_SECS=300

# ── Logging ──────────────────────────────────────────────────────────────────
RUST_LOG=info,rustassistant=debug
ENVFILE

chmod 600 "$ENV_FILE"
ok ".env written to $ENV_FILE (mode 600)"
echo

# ── Summary ──────────────────────────────────────────────────────────────────
printf "${BOLD}${GREEN}=== RustAssistant .env generated ===${NC}\n"
echo
printf "  %-28s %s\n" "File:"        "$ENV_FILE"
printf "  %-28s %s\n" "App port:"    "3500"
printf "  %-28s %s\n" "Postgres port:" "5433"
printf "  %-28s %s\n" "Redis port:"  "6380"
printf "  %-28s %s\n" "Ollama port:" "11434"
printf "  %-28s %s\n" "Postgres password:" "${RA_POSTGRES_PASSWORD:0:4}...${RA_POSTGRES_PASSWORD: -4}"
printf "  %-28s %s\n" "Redis password:" "${RA_REDIS_PASSWORD:0:4}...${RA_REDIS_PASSWORD: -4}"
printf "  %-28s %s\n" "Grok API key:" "$([ -n "$XAI_KEY" ] && echo "configured" || echo "not set")"
printf "  %-28s %s\n" "GitHub token:" "$([ -n "$GITHUB_TOKEN" ] && echo "configured" || echo "not set")"
printf "  %-28s %s\n" "Repos mount:"  "$REPOS_PATH"
echo

printf "${BOLD}Next steps:${NC}\n"
echo "  1. Review: ${CYAN}cat $ENV_FILE${NC}"
echo "  2. Start:  ${CYAN}cd $PROJECT_ROOT && docker compose up -d${NC}"
echo "  3. Pull model: ${CYAN}docker compose exec ra-ollama ollama pull qwen2.5-coder:7b${NC}"
echo "  4. Open:   ${CYAN}http://\$(hostname -I | awk '{print \$1}'):3500/dashboard${NC}"
echo
warn "Never commit .env to version control!"
echo
