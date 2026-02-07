# RustAssistant Upgrade Guide

## What's Included

This upgrade package adds 5 major features to RustAssistant:

1. **Zero Volume Mount Docker** - Everything baked into the image, repos cloned at runtime
2. **Scan Progress & Observability** - Real-time progress bars, activity feed, scan events
3. **Ideas Capture** - Quick thought capture with tagging, filtering, status workflow
4. **Documents/Knowledge Base** - Research docs, reference material, FTS search, RAG-ready
5. **Scan Interval Editing** - Per-repo scan interval configuration from the web UI

---

## File Inventory

```
docker/
├── Dockerfile              # Rebuilt with cargo-chef, zero bind mounts
└── docker-compose.yml      # Named volumes only, no host paths

migrations/
└── 003_scan_docs_ideas.sql # New tables: scan_events, documents, ideas, tags

src/
├── db/
│   ├── scan_events.rs      # Scan event logging, progress tracking
│   └── documents.rs        # Document & idea CRUD, tag management, FTS
├── repo_manager.rs         # Git clone/pull management (replaces volume mounts)
├── web_ui_extensions.rs    # New web UI routes: ideas, docs, activity, settings
├── lib_additions.rs        # Module declarations to add to lib.rs
└── server_integration.rs   # How to wire it into server.rs
```

---

## Step-by-Step Integration

### Step 1: Run the Migration

```bash
sqlite3 data/rustassistant.db < migrations/003_scan_docs_ideas.sql
```

This adds:
- `scan_events` table with indexes
- `documents` table with FTS5 virtual table and sync triggers
- `ideas` table with priority/status/category
- `tags` table for centralized tag management
- New columns on `repositories`: scan_status, scan_progress, scan_files_*, etc.

### Step 2: Add New Source Files

```bash
# Copy new modules
cp src/db/scan_events.rs    /path/to/rustassistant/src/db/
cp src/db/documents.rs      /path/to/rustassistant/src/db/
cp src/repo_manager.rs      /path/to/rustassistant/src/
cp src/web_ui_extensions.rs /path/to/rustassistant/src/
```

### Step 3: Update Module Declarations

In `src/lib.rs`, add:
```rust
pub mod repo_manager;
pub mod web_ui_extensions;
```

In `src/db/mod.rs` (or wherever your db modules are declared), add:
```rust
pub mod documents;
pub mod scan_events;
```

### Step 4: Update Cargo.toml

If not already present:
```toml
[dependencies]
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
```

### Step 5: Merge the Router in server.rs

In your `src/bin/server.rs`, change your router creation to merge the extensions:

```rust
use rustassistant::web_ui_extensions::create_extension_router;

// Where you currently create the router:
let state = WebAppState { db: db.clone() };
let app = create_router(state.clone())
    .merge(create_extension_router(state));
```

### Step 6: Update Navigation in Existing Pages

In `src/web_ui.rs`, update the `<nav>` in every handler to add the new pages:

```html
<nav>
    <a href="/dashboard">Dashboard</a>
    <a href="/repos">Repos</a>
    <a href="/queue">Queue</a>
    <a href="/ideas">Ideas</a>
    <a href="/docs">Docs</a>
    <a href="/activity">Activity</a>
    {tz_selector}
</nav>
```

### Step 7: Add Scan Progress to Repos Page

In the repos handler, add this section to show real-time scan progress:

```html
<h3>Scan Progress</h3>
<div id="scan-progress" hx-get="/scan/progress" hx-trigger="every 5s" hx-swap="innerHTML">
    Loading...
</div>
<script src="https://unpkg.com/htmx.org@2.0.0"></script>
```

And add a settings button to each repo card:
```html
<a href="/repos/{id}/settings" class="btn-small btn-primary">⚙️ Settings</a>
```

### Step 8: Integrate Scan Events into Scanner

In your auto_scanner module, import and use the event logging:

```rust
use crate::db::scan_events::{
    mark_scan_started, update_scan_file_progress,
    mark_scan_complete, mark_scan_error
};

// Before scanning a repo:
let total = count_repo_files(&repo_path)?;
mark_scan_started(&pool, &repo.id, total as i32).await?;

// During file-by-file scanning:
for (i, file) in files.iter().enumerate() {
    update_scan_file_progress(&pool, &repo.id, file, (i + 1) as i32, total as i32).await?;
    // ... process file ...
}

// After successful scan:
mark_scan_complete(&pool, &repo.id, total as i32, issues, duration_ms).await?;

// On error:
mark_scan_error(&pool, &repo.id, &err.to_string()).await?;
```

### Step 9: Replace Volume Mounts with Repo Manager

In your scanner, replace direct path access with cloning:

```rust
use crate::repo_manager::ensure_repo;

// When adding a new repo (web UI), store git URL:
let git_url = format!("https://github.com/{}/{}.git", user, name);

// In scanner, before scanning:
let sync = ensure_repo(&pool, &repo.id, &repo.git_url.unwrap(), &repo.name).await
    .map_err(|e| { /* handle error */ })?;

let scan_path = sync.local_path;
// Now scan scan_path instead of a volume-mounted path
```

### Step 10: Update Docker Files

```bash
cp docker/Dockerfile docker/Dockerfile
cp docker/docker-compose.yml docker-compose.yml
```

Then rebuild:
```bash
docker compose build --no-cache
docker compose up -d
```

---

## New Web UI Pages

| Route | Purpose |
|-------|---------|
| `/ideas` | Quick thought capture with tags, priority, status filters |
| `/ideas/add` | POST endpoint for new ideas |
| `/docs` | Document library with search and type filters |
| `/docs/new` | New document form (markdown editor) |
| `/docs/{id}` | View a document |
| `/activity` | Real-time activity feed from scan events |
| `/repos/{id}/settings` | Edit scan interval per repo |
| `/scan/progress` | HTMX partial for scan progress bars |
| `/api/scan/progress` | JSON scan progress for all repos |
| `/api/tags` | Tag autocomplete API |

---

## Database Schema Additions

### scan_events
Logs every scanner action for the activity feed. Auto-prunable.

### documents  
Full knowledge base with FTS5 search. Ready for RAG embedding.
Types: research, reference, tutorial, architecture, decision, snippet, runbook, template.

### ideas
Quick thought capture. Status workflow: inbox → active → in_progress → done → archived.
Categories: feature, bug, improvement, research, question, random.

### tags
Centralized tag registry with usage counts. Powers autocomplete and prevents tag drift.

### repositories (new columns)
scan_status, scan_progress, scan_files_total, scan_files_done, scan_issues_found, scan_duration_ms, last_scan_error.

---

## Future: Docs Repo

When the docs collection grows, create a `nuniesmith/docs` repo:

```
docs-repo/
├── rust/
│   ├── async-patterns.md
│   ├── error-handling.md
│   └── trait-design.md
├── docker/
│   ├── multi-stage-builds.md
│   └── compose-patterns.md
├── deployment/
│   ├── pi-setup.md
│   └── github-actions.md
└── architecture/
    ├── rag-design.md
    └── queue-systems.md
```

RustAssistant scans this repo like any other, but indexes content into the RAG system instead of looking for TODOs. Same codebase, different scan mode.
