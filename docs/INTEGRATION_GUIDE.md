# Integration Guide: DB Explorer + Enhanced Scan Progress

## Overview

Two new web UI modules:
1. **DB Explorer** (`/db`) — Browse SQLite tables, view rows, run queries
2. **Scan Progress Dashboard** (`/scan/dashboard`) — Real-time `[42/936]` file counter with cost, ETA, cache stats

---

## Step 1: Add migration for new tracking columns

Create a new migration file (e.g., `migrations/010_scan_progress_enhanced.sql`):

```sql
-- Enhanced scan progress tracking columns
ALTER TABLE repositories ADD COLUMN scan_started_at INTEGER;
ALTER TABLE repositories ADD COLUMN scan_cost_accumulated REAL DEFAULT 0.0;
ALTER TABLE repositories ADD COLUMN scan_cache_hits INTEGER DEFAULT 0;
ALTER TABLE repositories ADD COLUMN scan_api_calls INTEGER DEFAULT 0;
```

Or run directly against the DB if you prefer:
```bash
docker compose exec rustassistant sqlite3 /app/data/rustassistant.db \
  "ALTER TABLE repositories ADD COLUMN scan_started_at INTEGER;
   ALTER TABLE repositories ADD COLUMN scan_cost_accumulated REAL DEFAULT 0.0;
   ALTER TABLE repositories ADD COLUMN scan_cache_hits INTEGER DEFAULT 0;
   ALTER TABLE repositories ADD COLUMN scan_api_calls INTEGER DEFAULT 0;"
```

## Step 2: Register modules in `src/lib.rs`

```rust
pub mod web_ui_db_explorer;
pub mod web_ui_scan_progress;
```

## Step 3: Merge routes in `src/bin/server.rs`

```rust
use rustassistant::web_ui_db_explorer::create_db_explorer_router;
use rustassistant::web_ui_scan_progress::create_scan_progress_router;

// After creating web_state:
let db_explorer_router = create_db_explorer_router(Arc::new(web_state.clone()));
let scan_progress_router = create_scan_progress_router(Arc::new(web_state.clone()));

let app = Router::new()
    .merge(web_router)
    .merge(extension_router)
    .merge(cache_viewer_router)
    .merge(db_explorer_router)        // NEW
    .merge(scan_progress_router)      // NEW
    .merge(api_router);
```

## Step 4: Update nav in existing modules

In `web_ui.rs`, `web_ui_extensions.rs`, and `web_ui_cache_viewer.rs`, add to the nav links:

```rust
("Scan Progress", "/scan/dashboard"),
("DB Explorer", "/db"),
```

## Step 5: Update `auto_scanner.rs` to populate enhanced columns

### 5a. At scan start (in `scan_repository()`):

After `start_scan()` call, also set the new columns:

```rust
// Mark scan start with timestamp for ETA calculation
sqlx::query(
    "UPDATE repositories SET scan_started_at = ?, scan_cost_accumulated = 0.0, scan_cache_hits = 0, scan_api_calls = 0 WHERE id = ?"
)
.bind(chrono::Utc::now().timestamp())
.bind(&repo.id)
.execute(&self.pool)
.await
.ok();
```

### 5b. After each file analysis (in `analyze_changed_files_with_progress()`):

**Replace** the existing periodic progress update block (the `if idx % progress_update_interval == 0` block
that calls `crate::db::core::update_scan_progress`) with an every-file update that also writes the new
columns. Add a mutable `api_calls` counter next to the existing `cache_hits` counter near the top of the function:

```rust
let mut api_calls = 0i64;  // ADD this next to the existing `let mut cache_hits = 0i64;`
```

Then **remove** the existing periodic progress update:

```rust
// DELETE this block:
//   if idx % progress_update_interval == 0 || idx == filtered_count - 1 {
//       if let Err(e) = crate::db::core::update_scan_progress(...) { ... }
//   }
```

And in the existing `Ok(file_result) => { ... }` arm (which already uses `file_result.issues_found`,
`file_result.cost_usd`, `file_result.was_cache_hit`), add the enhanced progress update **after** the
existing checkpoint save:

```rust
match self
    .analyze_file(repo_path, file, &cache, idx, filtered_count)
    .await
{
    Ok(file_result) => {
        files_analyzed += 1;
        issues_found += file_result.issues_found;
        cumulative_cost += file_result.cost_usd;
        if file_result.was_cache_hit {
            cache_hits += 1;
        } else {
            api_calls += 1;
        }

        // Log with progress counter
        info!(
            "[{}/{}] {} {} (cost: ${:.4}, cumulative: ${:.4})",
            idx + 1,
            filtered_count,
            if file_result.was_cache_hit { "📦 CACHE" } else { "✅ DONE " },
            rel_path,
            file_result.cost_usd,
            cumulative_cost,
        );

        // ... (keep existing cost milestone logging + checkpoint save) ...

        // UPDATE DB progress on every file (replaces the old periodic update).
        // This is what the HTMX frontend reads for the live progress bar.
        sqlx::query(
            "UPDATE repositories SET
                scan_files_processed = ?,
                scan_current_file = ?,
                scan_cost_accumulated = ?,
                scan_cache_hits = ?,
                scan_api_calls = ?
            WHERE id = ?"
        )
        .bind((idx + 1) as i64)
        .bind(&rel_path)
        .bind(cumulative_cost)
        .bind(cache_hits)
        .bind(api_calls)
        .bind(repo_id)
        .execute(&self.pool)
        .await
        .ok();
    }
    Err(e) => {
        error!(
            "[{}/{}] ❌ Failed to analyze {}: {}",
            idx + 1, filtered_count, file.display(), e
        );
    }
}
```

Note: updating the DB on every file is fine for SQLite since these are fast single-row UPDATEs. The HTMX frontend polls every 2 seconds so the progress bar updates smoothly. This **replaces** both the old `update_scan_progress` call (every 5 files) and the progress counter — the checkpoint save should still remain as-is.

### 5c. Fix the panic (while you're in there):

In `refactor_assistant.rs` around line 560, the "no entry found for key" panic is almost certainly a `HashMap::index` on a missing key from a Grok JSON response. Find any bare `map[key]` and change to:

```rust
// Instead of:
map[key]  // panics if key missing

// Use:
map.get(key).cloned().unwrap_or_default()
```

Also add to `docker-compose.yml`:
```yaml
environment:
  - RUST_BACKTRACE=1
```

---

## What you get

### DB Explorer (`/db`)

```
🗄️ Database Explorer
┌──────────┬──────────┬──────────┬──────────┐
│ 12       │ 4        │ 15,847   │ 8.2 MB   │
│ Tables   │ Views    │ Rows     │ DB Size  │
└──────────┴──────────┴──────────┴──────────┘

Tables & Views                    [🔍 Run Query]
├─ documents          TABLE   15 columns  127 rows
├─ ideas              TABLE   12 columns   34 rows
├─ llm_usage          TABLE    8 columns  269 rows
├─ notes              TABLE    7 columns   12 rows
├─ queue_items        TABLE   10 columns   47 rows
├─ repositories       TABLE   18 columns    1 rows
├─ scan_events        TABLE    6 columns  542 rows
├─ active_scans       VIEW     3 columns    0 rows
└─ repository_health  VIEW     5 columns    1 rows

Click any table → browse rows with sortable columns + pagination
Click "Run Query" → write SELECT/PRAGMA queries with quick-query buttons
```

### Scan Progress (`/scan/dashboard`)

```
📊 Scan Progress Dashboard
┌──────────┬──────────┬──────────┬──────────┐
│ 1        │ 1        │ $0.5314  │ 5.6%     │
│ Active   │ Repos    │ Cost     │ Cache    │
└──────────┴──────────┴──────────┴──────────┘

┌─ fks ─────────────────── 🔄 Scanning ─┐
│                                         │
│   287 / 936  files      ⏱ ETA: 1h 12m │
│   [████████░░░░░░░░░░░░░░] 30.7%      │
│   src/janus/crates/health/src/lib.rs   │
│                                         │
│   💰 $0.5314  🔍 269 API  📦 16 cache │
└─────────────────────────────────────────┘

Auto-refreshes every 2 seconds via HTMX.
No page reload needed.
```
