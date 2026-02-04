# Repo-Level Cache System Design

**Purpose:** Store analysis results and metadata at the repository level for faster access and portability

**Created:** February 3, 2026  
**Status:** Proposal

---

## Overview

Each repository will maintain its own cache directory containing:
- Analysis results
- Documentation snapshots
- Code metrics
- TODO summaries
- Refactoring suggestions
- Test coverage data

This allows:
- Faster re-analysis (no API calls for unchanged files)
- Git-tracked analysis history
- Portable cache between machines
- Per-repo configuration

---

## Directory Structure

```
<repo-root>/
├── .rustassistant/
│   ├── cache/
│   │   ├── analysis/          # File analysis results
│   │   │   ├── src_lib_rs.json
│   │   │   ├── src_main_rs.json
│   │   │   └── ...
│   │   ├── docs/              # Generated documentation
│   │   │   ├── modules/
│   │   │   │   ├── db.md
│   │   │   │   └── server.md
│   │   │   └── README.generated.md
│   │   ├── refactor/          # Refactoring suggestions
│   │   │   ├── src_server_rs.json
│   │   │   └── plans/
│   │   │       └── extract_method_plan.json
│   │   ├── todos/             # TODO scan results
│   │   │   └── summary.json
│   │   └── metrics/           # Code metrics
│   │       ├── complexity.json
│   │       ├── quality.json
│   │       └── trends.json
│   ├── config.toml           # Repo-specific config
│   ├── .gitignore            # Ignore cache if desired
│   └── README.md             # What this directory is
```

---

## Cache File Format

### Analysis Cache
**File:** `.rustassistant/cache/analysis/<file_path_hash>.json`

```json
{
  "version": "1.0",
  "file_path": "src/lib.rs",
  "file_hash": "sha256:abc123...",
  "analyzed_at": "2026-02-03T12:00:00Z",
  "analyzer": "grok-beta",
  "results": {
    "quality_score": 8.5,
    "complexity_score": 6.2,
    "security_issues": [],
    "suggestions": [
      {
        "type": "extract_method",
        "line": 45,
        "description": "Consider extracting this logic"
      }
    ]
  },
  "llm_tokens": 2500,
  "cost_usd": 0.0125
}
```

### Documentation Cache
**File:** `.rustassistant/cache/docs/modules/<module_name>.md`

Generated Markdown documentation with frontmatter:

```markdown
---
generated_at: 2026-02-03T12:00:00Z
file_path: src/db.rs
file_hash: sha256:abc123...
generator: rustassistant-docs-v0.2.0
---

# Module: db

Database operations and management...
```

### Refactoring Cache
**File:** `.rustassistant/cache/refactor/<file_path_hash>.json`

```json
{
  "version": "1.0",
  "file_path": "src/server.rs",
  "file_hash": "sha256:abc123...",
  "analyzed_at": "2026-02-03T12:00:00Z",
  "code_smells": [
    {
      "type": "LongFunction",
      "severity": "Medium",
      "line_start": 45,
      "line_end": 165,
      "description": "Function is 120 lines, consider breaking down"
    }
  ],
  "suggestions": [
    {
      "type": "ExtractMethod",
      "title": "Extract validation logic",
      "priority": "High",
      "effort": "Medium"
    }
  ]
}
```

---

## Cache Invalidation Strategy

### When to Invalidate

1. **File Changed** - Hash mismatch
   - Delete specific file cache
   - Regenerate on next analysis

2. **Manual Invalidation**
   ```bash
   rustassistant cache clear
   rustassistant cache clear --file src/lib.rs
   rustassistant cache clear --type docs
   ```

3. **Age-Based** (Optional)
   - Cache TTL in config
   - Auto-refresh after N days

### Hash Calculation

Use SHA-256 of file contents:
```rust
use sha2::{Sha256, Digest};

fn calculate_file_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}
```

---

## Configuration

### Global Config
**File:** `~/.config/rustassistant/config.toml`

```toml
[cache]
enabled = true
default_ttl_days = 30
max_size_mb = 500

[cache.types]
analysis = true
docs = true
refactor = true
todos = true
```

### Repo-Specific Config
**File:** `<repo>/.rustassistant/config.toml`

```toml
[repo]
name = "rustassistant"
exclude_patterns = [
    "target/**",
    "node_modules/**",
    "*.generated.rs"
]

[cache]
# Override global settings
ttl_days = 7  # More aggressive for active development
track_in_git = false  # Don't commit cache

[analysis]
run_on_save = true
auto_generate_docs = false
```

---

## Implementation Plan

### Phase 1: Basic Cache Structure (2 hours)

```rust
// src/cache/repo_cache.rs

pub struct RepoCache {
    repo_path: PathBuf,
    cache_dir: PathBuf,
}

impl RepoCache {
    pub fn new(repo_path: impl AsRef<Path>) -> Result<Self> {
        let repo_path = repo_path.as_ref().to_path_buf();
        let cache_dir = repo_path.join(".rustassistant/cache");
        
        // Create directory structure
        std::fs::create_dir_all(&cache_dir)?;
        
        Ok(Self { repo_path, cache_dir })
    }
    
    pub fn get_analysis(&self, file_path: &str) -> Result<Option<AnalysisCache>> {
        let hash = self.path_to_hash(file_path);
        let cache_path = self.cache_dir.join(format!("analysis/{}.json", hash));
        
        if !cache_path.exists() {
            return Ok(None);
        }
        
        let content = std::fs::read_to_string(cache_path)?;
        let cache: AnalysisCache = serde_json::from_str(&content)?;
        
        // Validate hash
        let file_content = std::fs::read_to_string(file_path)?;
        if cache.file_hash != calculate_file_hash(&file_content) {
            return Ok(None); // Invalid, file changed
        }
        
        Ok(Some(cache))
    }
    
    pub fn save_analysis(&self, file_path: &str, results: AnalysisResults) -> Result<()> {
        let hash = self.path_to_hash(file_path);
        let cache_dir = self.cache_dir.join("analysis");
        std::fs::create_dir_all(&cache_dir)?;
        
        let file_content = std::fs::read_to_string(file_path)?;
        let cache = AnalysisCache {
            version: "1.0".into(),
            file_path: file_path.to_string(),
            file_hash: calculate_file_hash(&file_content),
            analyzed_at: chrono::Utc::now(),
            results,
        };
        
        let cache_path = cache_dir.join(format!("{}.json", hash));
        let json = serde_json::to_string_pretty(&cache)?;
        std::fs::write(cache_path, json)?;
        
        Ok(())
    }
    
    fn path_to_hash(&self, path: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(path.as_bytes());
        format!("{:x}", hasher.finalize())[..16].to_string()
    }
}
```

### Phase 2: Integration with Existing Features (3 hours)

Update existing analyzers to check cache first:

```rust
// In src/refactor_assistant.rs
pub async fn analyze_file(&self, file_path: impl AsRef<Path>) -> Result<RefactoringAnalysis> {
    let file_path = file_path.as_ref();
    
    // Try cache first
    let cache = RepoCache::new(".")?;
    if let Some(cached) = cache.get_refactor_analysis(file_path)? {
        println!("✓ Using cached analysis ({})", cached.analyzed_at.format("%Y-%m-%d"));
        return Ok(cached.results);
    }
    
    // Cache miss - analyze with LLM
    let content = std::fs::read_to_string(file_path)?;
    let analysis = self.analyze_with_llm(&content).await?;
    
    // Save to cache
    cache.save_refactor_analysis(file_path, &analysis)?;
    
    Ok(analysis)
}
```

### Phase 3: CLI Commands (1 hour)

```bash
# Initialize cache for repo
rustassistant cache init

# Show cache status
rustassistant cache status

# Clear cache
rustassistant cache clear
rustassistant cache clear --file src/lib.rs
rustassistant cache clear --type docs

# Show cache stats
rustassistant cache stats
```

---

## Benefits

### 1. Performance
- **No API calls** for cached results
- **Instant** analysis on unchanged files
- **70%+ cache hit** on typical workflows

### 2. Cost Savings
- Avoid re-analyzing unchanged code
- Only pay for new/modified files
- Track costs per repository

### 3. Portability
- **Git-trackable** if desired
- Share analysis with team
- CI/CD can use cached results

### 4. Offline Work
- View past analysis offline
- Generate reports without API
- Review suggestions anytime

### 5. History Tracking
- See analysis over time
- Track metric improvements
- Compare before/after refactoring

---

## Migration Path

### For Existing Users

1. **Auto-migrate** on first use
   - Detect repos without `.rustassistant/`
   - Initialize structure
   - Move to per-repo cache

2. **Preserve global cache**
   - Keep SQLite database for cross-repo data
   - Use for queue, notes, tasks
   - Repo cache supplements, doesn't replace

3. **Gradual adoption**
   - Cache is optional
   - Falls back to API if disabled
   - No breaking changes

---

## Example Usage

```bash
# Initialize cache for rustassistant repo
cd ~/github/rustassistant
rustassistant cache init

# Analyze files (creates cache)
rustassistant refactor analyze src/lib.rs
# → Calls API, saves to .rustassistant/cache/refactor/...

# Re-analyze same file (uses cache)
rustassistant refactor analyze src/lib.rs
# → Instant! No API call.

# Modify file
echo "// comment" >> src/lib.rs

# Re-analyze (cache miss, new analysis)
rustassistant refactor analyze src/lib.rs
# → Hash changed, calls API, updates cache

# Generate docs for all modules
rustassistant docs module src/lib.rs -o .rustassistant/cache/docs/modules/lib.md
rustassistant docs module src/server.rs -o .rustassistant/cache/docs/modules/server.md

# View cache stats
rustassistant cache stats
# Output:
# 📊 Cache Statistics
#   Total cached files: 15
#   Disk usage: 2.3 MB
#   Cache hits (7 days): 142 (85%)
#   Cost saved: $7.10
```

---

## Security Considerations

### 1. Sensitive Data
- **Don't cache** API keys, secrets
- Add `.rustassistant/cache/` to `.gitignore` by default
- Sanitize file paths in cache

### 2. Git Tracking
- Cache is **opt-in** for git
- Default: local only
- Config option: `track_in_git = true`

### 3. Permissions
- Cache inherits repo permissions
- 700 permissions on cache directory
- Validate file hashes before use

---

## Future Enhancements

### 1. Distributed Cache
- Share cache across team via remote storage
- S3/GCS backend support
- Cache server for organizations

### 2. Smart Prefetching
- Analyze related files proactively
- Predict what user will need
- Background cache warming

### 3. Cache Analytics
- Track hit rates per file type
- Identify frequently analyzed files
- Optimize cache eviction

### 4. Compression
- Compress old cache entries
- Archive historical data
- Reduce disk usage

---

## Summary

**Repo-level caching provides:**
- ✅ Faster analysis (no API calls)
- ✅ Lower costs (cache reuse)
- ✅ Offline capability
- ✅ Portable results
- ✅ History tracking
- ✅ Team sharing (optional)

**Implementation effort:** ~6 hours
**Impact:** High (major UX improvement)
**Risk:** Low (opt-in, backwards compatible)

**Recommendation:** Implement in v0.2.1 after v0.2.0 release

---

**Status:** Ready for implementation  
**Next Step:** Create `src/cache/repo_cache.rs` module