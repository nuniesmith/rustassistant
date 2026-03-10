#!/usr/bin/env bash
# =============================================================================
# fix-wsl2-nvidia.sh — Fix NVIDIA container toolkit for WSL2
#
# WSL2 exposes the GPU via /usr/lib/wsl/lib/ stubs rather than kernel modules.
# The default nvidia-container-runtime config has:
#   - load-kmods = true   (wrong: no kernel modules in WSL2)
#   - mode = "auto"       (wrong: auto tries legacy/cgroups path and crashes)
#   - no-cgroups = false  (wrong: WSL2 cgroup v1 GPU support is limited)
#
# This script patches /etc/nvidia-container-runtime/config.toml to use the
# correct WSL2 settings, then restarts Docker so the change takes effect.
#
# Usage:
#   chmod +x docker/fix-wsl2-nvidia.sh
#   sudo ./docker/fix-wsl2-nvidia.sh
#
# After running, bring Ollama back up with GPU:
#   docker compose up -d ollama
# =============================================================================

set -euo pipefail

CONFIG="/etc/nvidia-container-runtime/config.toml"
BACKUP="${CONFIG}.bak.$(date +%Y%m%d_%H%M%S)"

# ── Require root ────────────────────────────────────────────────────────────
if [[ $EUID -ne 0 ]]; then
    echo "ERROR: This script must be run as root." >&2
    echo "       Run: sudo $0" >&2
    exit 1
fi

# ── Verify we are inside WSL2 ───────────────────────────────────────────────
if ! grep -qi "microsoft" /proc/version 2>/dev/null; then
    echo "WARNING: /proc/version does not mention Microsoft — are you sure this is WSL2?"
    echo "         Continuing anyway, but double-check before rebooting Docker."
fi

# ── Check WSL GPU stubs are present ─────────────────────────────────────────
WSL_LIB_DIR="/usr/lib/wsl/lib"
if [[ ! -d "$WSL_LIB_DIR" ]]; then
    echo "ERROR: $WSL_LIB_DIR not found." >&2
    echo "       Your Windows NVIDIA driver may not be installed or WSL2 GPU support" >&2
    echo "       may not be enabled. Install the latest NVIDIA Game-Ready or Studio" >&2
    echo "       driver on Windows (>=545 recommended for WSL2 CUDA)." >&2
    exit 1
fi

NVML_STUB="$WSL_LIB_DIR/libnvidia-ml.so.1"
if [[ ! -f "$NVML_STUB" ]]; then
    echo "ERROR: $NVML_STUB not found — GPU stubs missing." >&2
    exit 1
fi

# Detect driver version from the versioned .so name in /usr/lib/wsl/lib/
DRIVER_VERSION=$(ls "$WSL_LIB_DIR"/libnvidia-gpucomp.so.* 2>/dev/null \
    | head -1 \
    | grep -oP '\d+\.\d+\.\d+' \
    || echo "unknown")
echo "Detected WSL2 NVIDIA driver version: ${DRIVER_VERSION}"

# ── Check config file exists ─────────────────────────────────────────────────
if [[ ! -f "$CONFIG" ]]; then
    echo "ERROR: $CONFIG not found." >&2
    echo "       Is nvidia-container-toolkit installed?" >&2
    echo "       Install: sudo apt-get install -y nvidia-container-toolkit" >&2
    exit 1
fi

# ── Backup original config ───────────────────────────────────────────────────
echo "Backing up $CONFIG → $BACKUP"
cp "$CONFIG" "$BACKUP"

# ── Apply WSL2 fixes ─────────────────────────────────────────────────────────
echo "Patching $CONFIG for WSL2..."

# 1. load-kmods = false  (no kernel modules in WSL2)
sed -i 's/^load-kmods\s*=\s*true/load-kmods = false/' "$CONFIG"

# 2. no-cgroups = true   (WSL2 cgroup GPU isolation is unreliable)
#    The line may be commented out as "#no-cgroups = false" — handle both forms.
if grep -qE '^#?no-cgroups' "$CONFIG"; then
    sed -i 's/^#*no-cgroups\s*=\s*.*/no-cgroups = true/' "$CONFIG"
else
    # Insert after [nvidia-container-cli] section header
    sed -i '/^\[nvidia-container-cli\]/a no-cgroups = true' "$CONFIG"
fi

# 3. mode = "csv"        (use the CSV host-files mode which works with WSL stubs)
sed -i '/^\[nvidia-container-runtime\]$/,/^\[/ s/^mode\s*=\s*.*/mode = "csv"/' "$CONFIG"

# 4. Ensure /usr/lib/wsl/lib is in the library search path for the CLI.
#    If there is no explicit `libraries` key we add one after [nvidia-container-cli].
if ! grep -q '^libraries\s*=' "$CONFIG"; then
    sed -i "/^\[nvidia-container-cli\]/a libraries = [\"$WSL_LIB_DIR\"]" "$CONFIG"
fi

echo ""
echo "Patched config:"
echo "──────────────────────────────────────────────────────"
grep -E "^(load-kmods|no-cgroups|mode|libraries)" "$CONFIG" || true
echo "──────────────────────────────────────────────────────"

# ── Generate CDI spec so Docker can find the GPU by device name ──────────────
echo ""
echo "Generating CDI spec (nvidia-ctk cdi generate)..."
if nvidia-ctk cdi generate --output=/etc/cdi/nvidia.yaml 2>/dev/null; then
    echo "CDI spec written to /etc/cdi/nvidia.yaml"
    nvidia-ctk cdi list 2>/dev/null || true
else
    echo "WARNING: CDI spec generation failed — this is non-fatal; CSV mode will be used."
fi

# ── Configure Docker runtime ─────────────────────────────────────────────────
echo ""
echo "Configuring Docker to use nvidia runtime..."
nvidia-ctk runtime configure --runtime=docker 2>/dev/null \
    && echo "Docker runtime configured." \
    || echo "WARNING: nvidia-ctk runtime configure failed — Docker daemon.json may need manual update."

# ── Verify /usr/lib/wsl/lib is on the ldconfig path ─────────────────────────
echo ""
if ! ldconfig -p | grep -q "libcuda.so.1"; then
    echo "Adding $WSL_LIB_DIR to /etc/ld.so.conf.d/wsl-nvidia.conf ..."
    echo "$WSL_LIB_DIR" > /etc/ld.so.conf.d/wsl-nvidia.conf
    ldconfig
    echo "ldconfig updated."
else
    echo "libcuda.so.1 already visible to ldconfig — no ld.so.conf change needed."
fi

# ── Restart Docker ───────────────────────────────────────────────────────────
echo ""
echo "Restarting Docker daemon..."
if command -v systemctl &>/dev/null && systemctl is-active --quiet docker 2>/dev/null; then
    systemctl restart docker
    echo "Docker restarted via systemctl."
elif command -v service &>/dev/null; then
    service docker restart
    echo "Docker restarted via service."
else
    echo "WARNING: Could not restart Docker automatically."
    echo "         Please restart it manually: sudo service docker restart"
fi

# ── Quick smoke test ─────────────────────────────────────────────────────────
echo ""
echo "Waiting 3s for Docker to come back up..."
sleep 3

echo "Smoke test — running nvidia-smi inside a container:"
if docker run --rm --gpus all nvidia/cuda:12.3.0-base-ubuntu22.04 nvidia-smi 2>/dev/null; then
    echo ""
    echo "✅  GPU is accessible inside containers!"
else
    echo ""
    echo "⚠️   Container GPU test failed. Trying with device flag instead of --gpus all..."
    docker run --rm \
        -e NVIDIA_VISIBLE_DEVICES=all \
        -e NVIDIA_DRIVER_CAPABILITIES=compute,utility \
        --device=/dev/dxg \
        -v /usr/lib/wsl:/usr/lib/wsl \
        nvidia/cuda:12.3.0-base-ubuntu22.04 nvidia-smi 2>/dev/null \
        && echo "✅  GPU accessible via --device=/dev/dxg mount!" \
        || echo "❌  Both methods failed. See troubleshooting notes below."
fi

# ── Print next steps ─────────────────────────────────────────────────────────
cat <<'EOF'

═══════════════════════════════════════════════════════════════
 Next steps
═══════════════════════════════════════════════════════════════

1. If the smoke test passed, re-enable GPU in docker-compose.yml:

   a) In the `ollama` service, change:
        OLLAMA_NUM_GPU=0
      to:
        OLLAMA_NUM_GPU=1

   b) Uncomment the deploy block:
        deploy:
          resources:
            reservations:
              devices:
                - driver: nvidia
                  count: all
                  capabilities: [gpu]

   c) Bring Ollama back up:
        docker compose up -d ollama

2. If the smoke test failed with "unknown device":

   WSL2 GPU passthrough uses /dev/dxg (DirectX Graphics) rather than
   /dev/nvidia0. Some older docker-compose + toolkit versions need the
   device mounted explicitly. Add to the ollama service in compose:

        devices:
          - /dev/dxg:/dev/dxg
        volumes:
          - /usr/lib/wsl:/usr/lib/wsl:ro

   Then re-run: docker compose up -d ollama

3. If nvidia-smi still segfaults on the HOST (outside containers):

   This is expected on WSL2 — nvidia-smi is a Windows-side tool and does
   not exist inside the WSL2 Linux environment. Use it from Windows:

        Win+R → cmd → nvidia-smi

   or PowerShell:
        nvidia-smi

   The absence of nvidia-smi in WSL2 is NORMAL and does NOT mean the GPU
   is unavailable to containers.

4. To undo all changes made by this script:
        sudo cp <backup_path> /etc/nvidia-container-runtime/config.toml
        sudo service docker restart

   Backup was written to: BACKUP_PLACEHOLDER
═══════════════════════════════════════════════════════════════
EOF

# Patch the backup path into the output (sed on the here-doc isn't possible,
# so just echo it separately)
echo "   Backup: $BACKUP"
echo ""
