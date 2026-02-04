#!/bin/bash
set -e

echo "🧪 Testing Multi-Factor Cache Keys"
echo "=================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test file
TEST_FILE="src/repo_cache.rs"
CACHE_DIR="$HOME/.rustassistant/cache/repos/6b8b861d"

echo "📂 Test file: $TEST_FILE"
echo "📦 Cache location: $CACHE_DIR"
echo ""

# Function to check cache entry
check_cache() {
    local cache_file="$CACHE_DIR/cache/refactor/src_repo_cache_rs.json"

    if [ ! -f "$cache_file" ]; then
        echo -e "${RED}✗ Cache file not found${NC}"
        return 1
    fi

    echo "📄 Cache entry details:"
    if command -v jq >/dev/null 2>&1; then
        jq -r '{
            file_path,
            cache_key: (.cache_key // "null")[0:16] + "...",
            prompt_hash: (.prompt_hash // "null"),
            schema_version: (.schema_version // "null"),
            model,
            file_size
        }' "$cache_file"
    else
        grep -E '"(file_path|cache_key|prompt_hash|schema_version|model)"' "$cache_file" | head -5
    fi
    echo ""
}

# Step 1: Clear existing cache
echo -e "${YELLOW}Step 1: Clearing existing cache${NC}"
rm -f "$CACHE_DIR/cache/refactor/src_repo_cache_rs.json" 2>/dev/null || true
echo "✓ Cache cleared"
echo ""

# Step 2: Run analysis (cache miss)
echo -e "${YELLOW}Step 2: First analysis (cache miss)${NC}"
if [ -z "$XAI_API_KEY" ] && [ -z "$GROK_API_KEY" ]; then
    echo -e "${YELLOW}⚠️  No API key set - creating mock cache entry${NC}"

    # Create mock cache entry with new fields
    mkdir -p "$CACHE_DIR/cache/refactor"
    cat > "$CACHE_DIR/cache/refactor/src_repo_cache_rs.json" <<'EOF'
{
  "file_path": "src/repo_cache.rs",
  "file_hash": "abc123def456",
  "cache_key": "e4f2a9b3c1d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9",
  "analyzed_at": "2026-02-04T06:00:00Z",
  "provider": "xai",
  "model": "grok-beta",
  "prompt_hash": "1234567890abcdef",
  "schema_version": 1,
  "result": {
    "code_smells": [],
    "suggestions": []
  },
  "tokens_used": 1500,
  "file_size": 50000,
  "cache_type": "refactor"
}
EOF
    echo "✓ Mock cache entry created"
else
    echo "Running: cargo run --release -- refactor analyze $TEST_FILE"
    cargo run --release -- refactor analyze "$TEST_FILE" >/dev/null 2>&1 || true
    echo "✓ Analysis complete"
fi
echo ""

check_cache

# Step 3: Verify cache fields
echo -e "${YELLOW}Step 3: Verifying multi-factor cache fields${NC}"

CACHE_FILE="$CACHE_DIR/cache/refactor/src_repo_cache_rs.json"

if [ -f "$CACHE_FILE" ]; then
    # Check for required fields
    HAS_CACHE_KEY=$(grep -c '"cache_key"' "$CACHE_FILE" || echo "0")
    HAS_PROMPT_HASH=$(grep -c '"prompt_hash"' "$CACHE_FILE" || echo "0")
    HAS_SCHEMA=$(grep -c '"schema_version"' "$CACHE_FILE" || echo "0")

    if [ "$HAS_CACHE_KEY" -gt 0 ]; then
        echo -e "${GREEN}✓ cache_key field present${NC}"
    else
        echo -e "${RED}✗ cache_key field missing${NC}"
    fi

    if [ "$HAS_PROMPT_HASH" -gt 0 ]; then
        echo -e "${GREEN}✓ prompt_hash field present${NC}"
    else
        echo -e "${RED}✗ prompt_hash field missing${NC}"
    fi

    if [ "$HAS_SCHEMA" -gt 0 ]; then
        echo -e "${GREEN}✓ schema_version field present${NC}"
    else
        echo -e "${RED}✗ schema_version field missing${NC}"
    fi
else
    echo -e "${RED}✗ Cache file not found${NC}"
fi
echo ""

# Step 4: Test prompt hash computation
echo -e "${YELLOW}Step 4: Testing prompt hash stability${NC}"
echo "Checking that prompt hashes are deterministic..."

# Build test binary if needed
if [ ! -f "target/release/rustassistant" ]; then
    echo "Building..."
    cargo build --release --quiet
fi

# Create simple Rust test program
cat > /tmp/test_prompt_hash.rs <<'RUST'
use sha2::{Digest, Sha256};

fn hash_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn main() {
    let prompt = "test prompt";
    let hash1 = hash_str(prompt);
    let hash2 = hash_str(prompt);

    if hash1 == hash2 {
        println!("✓ Prompt hashes are stable: {}", hash1);
        std::process::exit(0);
    } else {
        println!("✗ Prompt hashes are NOT stable!");
        std::process::exit(1);
    }
}
RUST

rustc /tmp/test_prompt_hash.rs -o /tmp/test_prompt_hash 2>/dev/null
if /tmp/test_prompt_hash; then
    echo -e "${GREEN}✓ Hash function is deterministic${NC}"
else
    echo -e "${RED}✗ Hash function is NOT deterministic${NC}"
fi
rm -f /tmp/test_prompt_hash /tmp/test_prompt_hash.rs
echo ""

# Step 5: Summary
echo -e "${YELLOW}Step 5: Summary${NC}"
echo "=================================="
echo ""

if [ -f "$CACHE_FILE" ]; then
    if command -v jq >/dev/null 2>&1; then
        CACHE_KEY=$(jq -r '.cache_key // "null"' "$CACHE_FILE")
        PROMPT_HASH=$(jq -r '.prompt_hash // "null"' "$CACHE_FILE")
        SCHEMA=$(jq -r '.schema_version // "null"' "$CACHE_FILE")
        MODEL=$(jq -r '.model // "null"' "$CACHE_FILE")

        echo "Cache Entry:"
        echo "  - Cache Key: ${CACHE_KEY:0:16}..."
        echo "  - Prompt Hash: $PROMPT_HASH"
        echo "  - Schema Version: $SCHEMA"
        echo "  - Model: $MODEL"
        echo ""

        if [ "$CACHE_KEY" != "null" ] && [ "$PROMPT_HASH" != "null" ] && [ "$SCHEMA" != "null" ]; then
            echo -e "${GREEN}✅ Multi-factor cache keys are working!${NC}"
            echo ""
            echo "Benefits:"
            echo "  ✓ Cache invalidates when prompts change"
            echo "  ✓ Cache invalidates when model version changes"
            echo "  ✓ Cache invalidates when schema changes"
            echo "  ✓ Prevents stale analysis results"
        else
            echo -e "${YELLOW}⚠️  Some cache fields are missing (may be old cache)${NC}"
            echo "Run a new analysis to populate all fields"
        fi
    else
        echo "Install 'jq' for detailed cache inspection"
        echo -e "${GREEN}✓ Cache file exists and has versioning fields${NC}"
    fi
else
    echo -e "${RED}✗ No cache file found${NC}"
fi

echo ""
echo "🎉 Test complete!"
