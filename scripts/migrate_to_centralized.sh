#!/bin/bash
set -e

echo "🔄 Migrating to centralized cache..."
echo ""

# Create centralized cache directory
mkdir -p ~/.rustassistant/cache/repos

# Function to compute repo hash (matches Rust implementation)
compute_hash() {
    local repo_path="$1"
    local canonical_path=$(realpath "$repo_path" 2>/dev/null || echo "$repo_path")
    echo -n "$canonical_path" | sha256sum | cut -c1-8
}

# Function to migrate one repo
migrate_repo() {
    local repo_path="$1"
    local cache_dir="$repo_path/.rustassistant/cache"

    if [ ! -d "$cache_dir" ]; then
        return
    fi

    echo "📦 Migrating: $repo_path"

    # Compute hash
    local canonical_path=$(realpath "$repo_path" 2>/dev/null || echo "$repo_path")
    local hash=$(compute_hash "$repo_path")

    # Create centralized location
    local dest="$HOME/.rustassistant/cache/repos/$hash"
    mkdir -p "$dest"

    # Copy cache files
    if [ -d "$cache_dir/refactor" ]; then
        mkdir -p "$dest/cache/refactor"
        cp -r "$cache_dir/refactor"/* "$dest/cache/refactor/" 2>/dev/null || true
        echo "  ✓ Migrated refactor cache"
    fi

    if [ -d "$cache_dir/docs" ]; then
        mkdir -p "$dest/cache/docs"
        cp -r "$cache_dir/docs"/* "$dest/cache/docs/" 2>/dev/null || true
        echo "  ✓ Migrated docs cache"
    fi

    if [ -d "$cache_dir/analysis" ]; then
        mkdir -p "$dest/cache/analysis"
        cp -r "$cache_dir/analysis"/* "$dest/cache/analysis/" 2>/dev/null || true
        echo "  ✓ Migrated analysis cache"
    fi

    if [ -d "$cache_dir/todos" ]; then
        mkdir -p "$dest/cache/todos"
        cp -r "$cache_dir/todos"/* "$dest/cache/todos/" 2>/dev/null || true
        echo "  ✓ Migrated todos cache"
    fi

    # Create meta.json
    cat > "$dest/meta.json" <<EOF
{
  "path": "$canonical_path",
  "hash": "$hash",
  "schema_version": 1,
  "migrated_at": "$(date -Iseconds)"
}
EOF

    echo "  ✓ Created meta.json"
    echo "  ✓ Hash: $hash"
    echo ""
}

# Migrate known repos
if [ -d ~/github/rustassistant/.rustassistant ]; then
    migrate_repo ~/github/rustassistant
fi

if [ -d ~/github/fks/.rustassistant ]; then
    migrate_repo ~/github/fks
fi

# Find and migrate any other repos with cache
echo "🔍 Searching for other repos with cache..."
find ~/github -maxdepth 2 -type d -name ".rustassistant" 2>/dev/null | while read cache_path; do
    repo_path=$(dirname "$cache_path")
    # Skip if already migrated
    if [ "$repo_path" != "$HOME/github/rustassistant" ] && [ "$repo_path" != "$HOME/github/fks" ]; then
        migrate_repo "$repo_path"
    fi
done

echo ""
echo "✅ Migration complete!"
echo ""
echo "📊 Cache location summary:"
if [ -d ~/.rustassistant/cache/repos ]; then
    for dir in ~/.rustassistant/cache/repos/*/; do
        if [ -f "$dir/meta.json" ]; then
            hash=$(basename "$dir")
            path=$(grep -o '"path": "[^"]*"' "$dir/meta.json" | cut -d'"' -f4)
            size=$(du -sh "$dir" 2>/dev/null | cut -f1)
            echo "  $hash -> $path ($size)"
        fi
    done
else
    echo "  (empty)"
fi
echo ""
echo "⚠️  To remove old local caches after verifying:"
echo "   1. Test that cache hits work: rustassistant cache status"
echo "   2. Run: find ~/github -name '.rustassistant' -type d -exec rm -rf {} +"
echo ""
echo "💡 Next steps:"
echo "   - Verify cache works: cargo run --release -- cache status"
echo "   - Test analysis: cargo run --release -- refactor analyze src/main.rs"
echo "   - Check centralized location: ls -la ~/.rustassistant/cache/repos/"
