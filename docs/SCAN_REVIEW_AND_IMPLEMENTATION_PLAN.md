# RustAssistant Service Log Review & Implementation Plan

## Log Analysis Summary

**Log period:** 2026-02-07 19:33 → 21:43 UTC (~2h 10m)
**Repo scanned:** `fks` — 936 changed files detected

### Key Metrics

| Metric | Value |
|--------|-------|
| Total files detected | 936 |
| API calls made | 269 |
| Cache hits | 16 (all post-panic resume) |
| Files skipped (deleted) | 28 |
| Completion | ~28.7% (still running when log captured) |
| Total API cost | $0.5314 |
| Avg cost per file | $0.0020 |
| Projected full scan cost | ~$1.87 |
| API errors | 1 (retry succeeded on attempt 2) |
| Panics | 1 (fatal — killed scan) |
| Avg time per file | ~28.5 seconds |
| Projected full scan time | ~7.4 hours |

---

## Issues Found

### 1. 🔴 PANIC: `refactor_assistant.rs:560:55` — "no entry found for key"

```
thread 'tokio-runtime-worker' (8) panicked at src/refactor_assistant.rs:560:55:
no entry found for key
```

This is a HashMap indexing panic in the JSON response parser. When Grok returns a JSON response missing an expected key, the code does an unwrapping index (`map[key]`) instead of a safe `.get(key)`. The file `src/janus/services/api/src/routes/health.rs` was the trigger — the API call succeeded (9376 tokens, $0.0017) but the response parsing panicked.

**Impact:** Killed the entire scan worker thread. The auto-scanner loop restarted on the next 1-minute tick, re-iterated all 936 files, and relied on cache hits to skip already-analyzed files. This works but wastes ~30s re-checking each cached file and re-running the full git diff.

**Fix:** In `refactor_assistant.rs` around line 560, replace any `map[key]` or `.unwrap()` on JSON field access with `.get(key)` or `.unwrap_or_default()`. The surrounding code already uses `unwrap_or` patterns for most fields, so this is likely one missed case.

### 2. 🟡 Cost/Token Data Not Propagating to auto_scanner

Every single `auto_scanner: Cached analysis` log line shows:
```
(cost: $0.0000, tokens: None)
```

Even though `grok_client` correctly reports costs like `$0.0014`, `$0.0023`, etc. The `analyze_file()` return path isn't propagating the actual cost and token data back to the auto_scanner logging. The `todo/auto_scanner_cost_accuracy_patch.rs` file in the project shows this was identified but may not be fully applied.

### 3. 🟡 No Progress Counter in Logs

Currently the log shows individual file names but no position indicator. With 936 files taking ~7 hours, there's no way to know "where are we?" without counting log lines. The `update_scan_progress` DB call exists but the console logging doesn't include it.

### 4. 🟡 No Resume Marker / Checkpoint

When the scan panicked and restarted, it had to re-enumerate all 936 files, re-run git diff, and check each against the cache. While cache hits are fast (~10ms each), iterating 270+ already-done files still adds latency. There's no persistent "last processed index" to skip directly to the next unprocessed file.

### 5. 🟢 No Final Review Step

After analyzing all 936 files individually, there's no aggregation step that synthesizes findings into a coherent task list. Each file gets its own isolated analysis, but there's no project-level view that understands cross-cutting concerns, dependency chains, or prioritized work items for the queue.

### 6. 🟢 API Retry Works Well

The single API failure at 20:41:11 was retried after 2s and succeeded. The retry logic is solid.

---

## Implementation Plan

### Phase A: Progressive Scan Logging & Checkpointing

**Goal:** Log `[42/936]` style progress on every file, checkpoint after each cache write, resume from last checkpoint on restart.

#### A1. Add progress counter to scan loop logging

In `auto_scanner.rs` `analyze_changed_files_with_progress()`, enhance the per-file log line:

```rust
// Before each file analysis
info!(
    "[{}/{}] {} Analyzing {}",
    idx + 1,
    filtered_count,
    if cache_hit { "📦 CACHE" } else { "🔍 API  " },
    rel_path
);
```

After the API call completes and cache is written:

```rust
info!(
    "[{}/{}] ✅ Cached {} (cost: ${:.4}, tokens: {}, cumulative: ${:.4})",
    idx + 1,
    filtered_count,
    rel_path,
    file_cost,
    tokens_used.unwrap_or(0),
    cumulative_cost
);
```

#### A2. Persist scan checkpoint after each file

Add a `scan_checkpoints` table or extend the existing `scan_progress`:

```sql
CREATE TABLE IF NOT EXISTS scan_checkpoints (
    repo_id TEXT NOT NULL,
    scan_id TEXT NOT NULL,        -- unique per scan run
    last_completed_index INTEGER NOT NULL,
    last_completed_file TEXT NOT NULL,
    files_analyzed INTEGER NOT NULL,
    files_cached INTEGER NOT NULL,  -- cache hits (no API cost)
    cumulative_cost REAL NOT NULL,
    total_files INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (repo_id, scan_id)
);
```

After each successful `analyze_file()` + cache write:

```rust
// Update checkpoint atomically with cache write
sqlx::query(
    "INSERT OR REPLACE INTO scan_checkpoints 
     (repo_id, scan_id, last_completed_index, last_completed_file,
      files_analyzed, files_cached, cumulative_cost, total_files, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
)
.bind(&repo_id)
.bind(&scan_id)
.bind(idx as i64)
.bind(&rel_path)
.bind(files_analyzed)
.bind(cache_hits)
.bind(cumulative_cost)
.bind(filtered_count as i64)
.bind(now_unix)
.execute(&self.pool)
.await?;
```

#### A3. Resume from checkpoint on restart

At the start of `analyze_changed_files_with_progress()`, check for an existing checkpoint:

```rust
// Check for existing checkpoint to resume from
let checkpoint = sqlx::query_as::<_, ScanCheckpoint>(
    "SELECT * FROM scan_checkpoints WHERE repo_id = ? ORDER BY updated_at DESC LIMIT 1"
)
.bind(&repo_id)
.fetch_optional(&self.pool)
.await?;

let start_index = if let Some(cp) = &checkpoint {
    // Verify the file list hasn't changed (same total_files)
    if cp.total_files == filtered_count as i64 {
        info!(
            "📍 Resuming scan from checkpoint: [{}/{}] (${:.4} spent so far)",
            cp.last_completed_index + 1,
            cp.total_files,
            cp.cumulative_cost
        );
        cumulative_cost = cp.cumulative_cost;
        files_analyzed = cp.files_analyzed as usize;
        (cp.last_completed_index + 1) as usize
    } else {
        info!("⚠️  File list changed since last checkpoint, restarting scan");
        0
    }
} else {
    0
};

// Skip to start_index in the file iteration
for (idx, file) in filtered_files.iter().enumerate() {
    if idx < start_index {
        continue; // Skip already-completed files
    }
    // ... existing analysis logic
}
```

This is strictly better than the current approach which relies on cache hits — the checkpoint skip is O(1) instead of O(n) cache lookups.

#### A4. Fix the panic in refactor_assistant.rs

The "no entry found for key" panic needs to be caught. Wrap the JSON parsing in the `analyze_content` response handler:

```rust
// Instead of: json["some_key"][sub_key]  which panics if missing
// Use: json.get("some_key").and_then(|v| v.get(sub_key))

// Or wrap the entire parse in catch_unwind as a safety net:
match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    self.parse_json_response(&response_text, file_path)
})) {
    Ok(result) => result,
    Err(_) => {
        warn!("JSON parse panicked for {}, returning empty analysis", file_path);
        Ok(RefactoringAnalysis::empty(file_path))
    }
}
```

Ideally, audit all `[]` indexing on `serde_json::Value` in `refactor_assistant.rs` and replace with safe access. The `Value` type returns `Value::Null` for missing keys when using `[]`, but if any code then calls `.as_object().unwrap()` or similar on the result, that's where the panic occurs.

Also add `RUST_BACKTRACE=1` to the Docker environment so future panics show the full stack trace.

---

### Phase B: Fix Cost/Token Propagation

**Goal:** Make auto_scanner log lines show actual API cost and token count.

The `grok_client` correctly calculates cost. The issue is in `auto_scanner.rs`'s `analyze_file()` — it needs to return the actual cost from the API response rather than `0.0`:

```rust
// In analyze_file(), after the grok API call:
let (analysis, actual_cost) = match self.refactor_assistant.analyze_content_tracked(&rel_path, &content).await {
    Ok(result) => result,
    Err(e) => return Err(e),
};

// The grok_client already logs the cost, but we need to capture it
// Option 1: Have analyze_content return (RefactoringAnalysis, f64)
// Option 2: Have grok_client store last_call_cost in an atomic
// Option 3: Extract cost from the tokens_used field on the analysis

// The simplest fix: use the tokens_used from the analysis to compute cost
let actual_cost = analysis.tokens_used
    .map(|t| (t as f64 / 1_000_000.0) * 0.35)  // avg of input/output pricing
    .unwrap_or(0.0);
```

The `auto_scanner_cost_accuracy_patch.rs` TODO file already describes this fix — it just needs to be applied.

---

### Phase C: Final Project Review & Task Generation

**Goal:** After all files are individually analyzed, run one final LLM call with the full project context to generate a prioritized, actionable task list for the queue.

#### C1. Collect all analysis results

After the file-by-file scan completes:

```rust
// After analyze_changed_files_with_progress completes (not budget halted)
if !budget_halted {
    info!("📊 All {} files analyzed. Starting final project review...", files_analyzed);
    
    let tasks = self.generate_project_review(&repo, &repo_path, &cache).await?;
    
    info!("📋 Generated {} tasks from project review", tasks.len());
}
```

#### C2. Build the project review prompt

This is where Grok's 2M context window becomes powerful. Gather all cached analysis summaries and send them as one context:

```rust
async fn generate_project_review(
    &self,
    repo: &Repository,
    repo_path: &Path,
    cache: &RepoCacheSql,
) -> Result<Vec<Task>> {
    // 1. Collect all cached analyses
    let all_entries = cache.get_all_entries().await?;
    
    // 2. Build a condensed project summary
    let mut project_context = String::new();
    let mut total_issues = 0;
    let mut all_smells: Vec<(String, String)> = Vec::new(); // (file, smell_summary)
    
    for entry in &all_entries {
        if let Some(analysis) = &entry.refactor_result {
            // Parse stored JSON analysis
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(analysis) {
                let file = &entry.file_path;
                let smells = parsed["code_smells"].as_array().map(|a| a.len()).unwrap_or(0);
                let suggestions = parsed["suggestions"].as_array().map(|a| a.len()).unwrap_or(0);
                let complexity = parsed["complexity_score"].as_f64().unwrap_or(50.0);
                
                total_issues += smells + suggestions;
                
                // Only include files with issues for the review prompt
                if smells > 0 || suggestions > 0 || complexity > 70.0 {
                    project_context.push_str(&format!(
                        "\n## {}\n- Complexity: {:.0}\n- Issues: {}\n- Analysis: {}\n",
                        file, complexity, smells + suggestions,
                        // Include truncated analysis to fit context
                        &analysis[..analysis.len().min(2000)]
                    ));
                    
                    all_smells.push((file.clone(), analysis.clone()));
                }
            }
        }
    }
    
    // 3. Build the final review prompt
    let prompt = format!(
        r#"You are reviewing a complete codebase analysis for the "{repo_name}" project.
        
{file_count} files were analyzed. {issue_count} total issues were found.

Below is a summary of every file that had issues. Your job is to:

1. Identify CROSS-CUTTING CONCERNS — patterns that appear across multiple files
   (e.g., "error handling is inconsistent across 12 service files")
2. Identify DEPENDENCY CHAINS — where fixing file A should happen before file B
3. Group related issues into ACTIONABLE TASKS that can each be completed in 1-4 hours
4. Prioritize by: Critical (security/crashes) > High (correctness) > Medium (quality) > Low (style)
5. For each task, specify:
   - Title (clear, actionable)
   - Description (what to do, not what's wrong)
   - Files affected (list)
   - Priority (critical/high/medium/low)
   - Estimated effort (small/medium/large)
   - Dependencies (which tasks must complete first)

Respond in JSON format:
{{
  "summary": "Brief overview of project health",
  "cross_cutting_concerns": ["..."],
  "tasks": [
    {{
      "title": "...",
      "description": "...",
      "files": ["..."],
      "priority": "critical|high|medium|low",
      "effort": "small|medium|large",
      "dependencies": [],
      "category": "security|error-handling|performance|testing|refactoring|documentation"
    }}
  ]
}}

=== FILE ANALYSES ===
{project_context}"#,
        repo_name = repo.name,
        file_count = all_entries.len(),
        issue_count = total_issues,
        project_context = project_context
    );
    
    // 4. Call Grok with the full context
    let response = self.grok_client.ask(&prompt).await?;
    
    // 5. Parse response into tasks and insert into queue
    let tasks = self.parse_review_into_tasks(&response, &repo.id).await?;
    
    Ok(tasks)
}
```

#### C3. Insert tasks into the queue

```rust
async fn parse_review_into_tasks(
    &self,
    response: &str,
    repo_id: &str,
) -> Result<Vec<Task>> {
    let json = serde_json::from_str::<serde_json::Value>(
        &self.extract_json(response)
    )?;
    
    let mut tasks = Vec::new();
    
    if let Some(task_array) = json["tasks"].as_array() {
        for (i, t) in task_array.iter().enumerate() {
            let task = Task {
                id: format!("review-{}-{:03}", repo_id, i),
                title: t["title"].as_str().unwrap_or("Untitled").to_string(),
                description: Some(t["description"].as_str().unwrap_or("").to_string()),
                priority: self.parse_priority_str(t["priority"].as_str().unwrap_or("medium")),
                status: TaskStatus::Pending,
                file_path: t["files"].as_array()
                    .and_then(|f| f.first())
                    .and_then(|f| f.as_str())
                    .map(String::from),
                source: TaskSource::ProjectReview,
                repo_id: Some(repo_id.to_string()),
                // ... other fields
            };
            
            // Insert into DB
            crate::db::tasks::create_task(&self.pool, &task).await?;
            tasks.push(task);
        }
    }
    
    info!("📋 Inserted {} tasks into queue from project review", tasks.len());
    Ok(tasks)
}
```

#### C4. Web UI: Show review status

Add a scan status indicator to the repo detail page:

```
Scan Progress: [████████░░░░░░░░] 287/936 (30.7%)
Current file: src/janus/services/api/src/routes/health.rs
Cost so far: $0.5314 / $3.00 budget
Status: Analyzing...

[When complete:]
✅ Scan complete: 936 files analyzed, $1.87 total cost
📊 Final review: 47 tasks generated → View Queue
```

---

## Implementation Priority

| Order | Task | Effort | Impact |
|-------|------|--------|--------|
| 1 | Fix panic in refactor_assistant.rs (A4) | Small | Critical — prevents scan crashes |
| 2 | Add RUST_BACKTRACE=1 to Docker env | Trivial | Debug — better panic diagnostics |
| 3 | Add progress counter logging (A1) | Small | Quality of life — know where you are |
| 4 | Fix cost/token propagation (Phase B) | Small | Accuracy — real cost tracking |
| 5 | Add checkpoint persistence (A2+A3) | Medium | Resilience — true resume capability |
| 6 | Final project review (Phase C) | Large | Value — actionable task queue |

Items 1-4 are quick wins (~1-2 hours each). Item 5 is a solid half-day. Item 6 is a full day of work but delivers the most value by turning 936 isolated file analyses into a coherent, prioritized work plan.

---

## Quick Wins from the Log

A few things the log reveals that are worth noting:

- **28 deleted files** are checked every scan. These are old scripts (`scripts/*.sh`, `docs/DEPLOY_FIXES.sh`) that exist in git history but not on disk. Consider adding a pre-filter that checks file existence before adding to the analysis queue.

- **Avg ~28s per file** includes some very fast small files (~5s) and some large files (~60s for 88K+ char prompts). The 109K char `config.rs` file took 18s and cost $0.0060 — consider setting a file size cap or splitting very large files.

- **The cache works.** After the panic restart, 16 cache hits were recorded in the log before new API calls resumed. The architecture is sound; it just needs the checkpoint layer on top.
