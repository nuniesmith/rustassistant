#!/bin/sh
# =============================================================================
# wsl2-entrypoint.sh — WSL2-aware Ollama entrypoint wrapper
#
# On WSL2, /usr/lib/wsl is bind-mounted from the host at runtime (not at
# build time), so the ldconfig cache inside the container is stale on first
# start.  This wrapper:
#
#   1. Runs ldconfig to register /usr/lib/wsl/lib in the dynamic linker cache
#      (the ld.so.conf.d fragment was written by the Dockerfile).
#   2. Creates any missing libcuda / libcudart version symlinks that Ollama's
#      CUDA prober expects to find (it looks for libcuda.so.1 specifically).
#   3. Execs the real Ollama binary with all original arguments.
#
# If /usr/lib/wsl/lib is not mounted (bare-metal Linux, CI, etc.) the ldconfig
# call is harmless and no symlinks are needed — the wrapper is a transparent
# pass-through in that case.
# =============================================================================

set -e

WSL_LIB="/usr/lib/wsl/lib"

# ── 1. Refresh dynamic linker cache ─────────────────────────────────────────
# ldconfig reads /etc/ld.so.conf.d/wsl-nvidia.conf (written by Dockerfile)
# and registers whatever is now present in /usr/lib/wsl/lib.
if [ -d "$WSL_LIB" ]; then
    echo "[wsl2-entrypoint] Refreshing ldconfig for WSL2 NVIDIA libs..."
    ldconfig 2>/dev/null || true
else
    echo "[wsl2-entrypoint] /usr/lib/wsl/lib not found — skipping ldconfig refresh (non-WSL2 host)"
fi

# ── 2. Ensure critical CUDA symlinks exist ───────────────────────────────────
# Ollama's GPU discovery calls dlopen("libcuda.so.1") — if the .so.1 symlink
# is missing (sometimes only .so.1.1 is present from the WSL driver package)
# the probe fails and Ollama falls back to CPU.
ensure_symlink() {
    local dir="$1"
    local target="$2"   # e.g. libcuda.so.1.1
    local link="$3"     # e.g. libcuda.so.1

    if [ -f "${dir}/${target}" ] && [ ! -e "${dir}/${link}" ]; then
        echo "[wsl2-entrypoint] Creating symlink: ${dir}/${link} -> ${target}"
        ln -sf "${target}" "${dir}/${link}" 2>/dev/null || true
    fi
}

if [ -d "$WSL_LIB" ]; then
    # libcuda.so.1 — required by CUDA runtime and Ollama CUDA prober
    ensure_symlink "$WSL_LIB" "libcuda.so.1.1"   "libcuda.so.1"
    ensure_symlink "$WSL_LIB" "libcuda.so.1"     "libcuda.so"

    # libnvidia-ml.so.1 — required by NVML (GPU enumeration)
    ensure_symlink "$WSL_LIB" "libnvidia-ml.so.1" "libnvidia-ml.so"

    # Log what we found so failures are diagnosable
    echo "[wsl2-entrypoint] WSL2 CUDA libs present:"
    ls "${WSL_LIB}"/libcuda*.so* "${WSL_LIB}"/libnvidia-ml*.so* 2>/dev/null \
        | sed 's/^/  /' \
        || echo "  (none found — GPU will not be available)"
fi

# ── 3. Hand off to Ollama ────────────────────────────────────────────────────
# The Dockerfile sets CMD ["serve"], so by default this runs `ollama serve`.
# Any arguments passed to `docker run` / compose override CMD and are passed
# through here unchanged.
echo "[wsl2-entrypoint] Starting Ollama: ollama $*"
exec /usr/bin/ollama "$@"
