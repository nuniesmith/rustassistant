# Token Tracking Quick Reference

**Status:** ✅ Production Ready (Phase 0.3 Complete)

---

## Quick Commands

### View Cache Status with Budget
```bash
rustassistant cache status
```

**Example Output:**
```
📦 Repository Cache Summary
  Location: ~/.rustassistant/cache/repos/abc123/

  refactor cache:
    Entries: 42
    Tokens: 125,000
    Estimated cost: $0.1250
    Total file size: 48,192 bytes

  Total entries: 42
  Total tokens: 125,000
  Total estimated cost: $0.1250

💰 Budget Status:
  ✅ Budget OK: $0.13 / $3.00 (4.2%)
  Remaining: $2.87
  Estimated tokens remaining: ~28,700,000
```

### Analyze with Token Tracking
```bash
rustassistant refactor analyze src/main.rs
# Shows: "💾 Analysis cached (tokens used: 5420)"
```

---

## Token Pricing (per 1M tokens)

| Provider | Input | Output | Notes |
|----------|-------|--------|-------|
| **Grok** | $5.00 | $15.00 | Default |
| GPT-4 | $30.00 | $60.00 | OpenAI |
| GPT-3.5 | $0.50 | $1.50 | OpenAI |
| Claude Sonnet | $3.00 | $15.00 | Anthropic |
| Gemini Pro | $0.50 | $1.50 | Google |

---

## Budget Thresholds

| Status | Emoji | Threshold | Action |
|--------|-------|-----------|--------|
| **OK** | ✅ | < 75% | Normal operation |
| **Warning** | ⚠️ | 75-90% | Monitor usage |
| **Alert** | 🔶 | 90-100% | Review spending |
| **Exceeded** | 🚨 | > 100% | Immediate action |

**Default Budget:** $3.00/month

---

## Programmatic Usage

### Check Budget Status
```rust
use rustassistant::{BudgetConfig, TokenPricing};

let budget = BudgetConfig::new(10.0); // $10/month
let pricing = TokenPricing::grok();
let current_spend = 7.50;

match budget.check_spending(current_spend) {
    BudgetStatus::Warning { percentage, .. } => {
        println!("⚠️  At {}%!", percentage * 100.0);
    }
    BudgetStatus::Exceeded { .. } => {
        println!("🚨 Over budget!");
    }
    _ => println!("✅ OK"),
}
```

### Track Token Usage
```rust
use rustassistant::TokenStats;

let mut stats = TokenStats::new();
stats.add_usage("xai", "grok-beta", 5000);

println!("{}", stats.format());
// Output:
// Total tokens: 5000
// Estimated cost: $0.0500
// API calls: 1
```

### Calculate Costs
```rust
use rustassistant::TokenPricing;

let pricing = TokenPricing::grok();

// Estimate (50/50 split)
let cost = pricing.estimate_cost(10_000);
println!("Est: ${:.4}", cost); // $0.1000

// Exact (separate input/output)
let cost = pricing.calculate_cost(8000, 2000);
println!("Exact: ${:.4}", cost); // $0.0700
```

---

## Data Structures

### TokenPricing
```rust
pub struct TokenPricing {
    pub input_per_million: f64,
    pub output_per_million: f64,
}

// Get pricing for provider
TokenPricing::grok()
TokenPricing::gpt4()
TokenPricing::claude_sonnet()
TokenPricing::for_provider("xai", "grok-beta")
```

### BudgetConfig
```rust
pub struct BudgetConfig {
    pub monthly_budget: f64,        // Default: $3.00
    pub warning_threshold: f64,     // Default: 0.75 (75%)
    pub alert_threshold: f64,       // Default: 0.90 (90%)
}

// Create with defaults
let config = BudgetConfig::default();

// Create with custom budget
let config = BudgetConfig::new(25.0); // $25/month
```

### TokenStats
```rust
pub struct TokenStats {
    pub total_tokens: usize,
    pub estimated_cost: f64,
    pub api_calls: usize,
    pub by_model: HashMap<String, ModelTokenStats>,
}

let mut stats = TokenStats::new();
stats.add_usage("xai", "grok-beta", 1000);
```

---

## Cache Integration

### Token Storage
Tokens are automatically stored in cache entries:
```rust
cache.set(CacheSetParams {
    cache_type: CacheType::Refactor,
    file_path: &file,
    content: &file_content,
    provider: "xai",
    model: "grok-beta",
    result: result_json,
    tokens_used: Some(5420),  // ← Tracked!
    prompt_hash: None,
    schema_version: None,
})?;
```

### Reading Stats
```rust
let repo_cache = RepoCache::new(&repo_path)?;
let stats = repo_cache.stats(CacheType::Refactor)?;

println!("Total tokens: {}", stats.total_tokens);
```

---

## Cost Examples

### 1K tokens (typical single file analysis)
```
Grok:   $0.01
GPT-4:  $0.045
Claude: $0.009
```

### 100K tokens (large codebase scan)
```
Grok:   $1.00
GPT-4:  $4.50
Claude: $0.90
```

### 1M tokens (full repo analysis)
```
Grok:   $10.00
GPT-4:  $45.00
Claude: $9.00
```

**Assumption:** 50/50 input/output split (actual may vary)

---

## Budget Planning

### $3/month budget allows:
- **~300K tokens** with Grok
- **~66K tokens** with GPT-4
- **~333K tokens** with Claude

### Typical usage:
- File analysis: ~1-5K tokens
- Refactor suggestions: ~5-10K tokens
- Doc generation: ~3-8K tokens
- Code review: ~2-6K tokens

**~300K tokens** = 60-300 file analyses/month

---

## Environment Variables

None required. Token tracking works automatically when:
- LLM APIs return usage data
- Cache is enabled
- Analysis functions populate `tokens_used`

---

## Configuration File (Future)

Coming in Phase 1:
```toml
# ~/.rustassistant/config.toml
[budget]
monthly_limit = 10.0      # $10/month
warning_threshold = 0.75  # 75%
alert_threshold = 0.90    # 90%

[pricing]
provider = "xai"
model = "grok-beta"
```

---

## Troubleshooting

### No token data showing?
- Check that you're using a supported LLM provider
- Verify API responses include usage data
- Ensure cache is enabled

### Costs seem wrong?
- Current estimates use 50/50 input/output split
- Actual costs vary by prompt/response length
- Future: Store separate input/output counts

### Budget not enforcing?
- Current implementation is informational only
- Phase 1 will add enforcement
- Manually check before expensive operations

---

## Related Documentation

- [Phase 0.3 Implementation](PHASE_0_3_TOKEN_TRACKING_COMPLETE.md)
- [Session Summary](SESSION_2024_TOKEN_TRACKING.md)
- [Cache Roadmap](CACHE_IMPLEMENTATION_ROADMAP.md)
- [Cache Quick Start](CACHE_QUICK_START.md)

---

## Support

For issues or questions:
1. Check diagnostics: `rustassistant cache status`
2. Review logs for token counts
3. Verify provider API responses
4. Consult documentation above

**Status:** All 124 tests passing ✅