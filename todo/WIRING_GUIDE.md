# RustAssistant — New Module Wiring Guide

## Files Generated

| File | Destination in repo | Purpose |
|------|---------------------|---------|
| `model_router.rs` | `src/model_router.rs` | Task classifier + local/remote model routing |
| `repo_sync.rs` | `src/repo_sync.rs` | Repo registry, tree walker, TODO+symbol extractor |
| `repos_api.rs` | `src/api/repos.rs` | Axum REST endpoints + chat handler |
| `sync_scheduler.rs` | `src/sync_scheduler.rs` | Background sync loop |

---

## Cargo.toml — Add These Dependencies

```toml
[dependencies]
# Already likely present — verify versions match
axum = { version = "0.7", features = ["macros"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
anyhow = "1"

# New additions
async-recursion = "1"          # for recursive async dir walker in repo_sync.rs
```

---

## src/main.rs or src/lib.rs — Register New Modules

```rust
mod model_router;
mod repo_sync;
mod sync_scheduler;
// inside src/api/mod.rs:
pub mod repos;
```

---

## AppState Wiring

In your existing AppState or wherever you build the Axum router:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::model_router::{ModelRouter, ModelRouterConfig};
use crate::repo_sync::RepoSyncService;
use crate::sync_scheduler::{SyncScheduler, SyncSchedulerConfig};
use crate::api::repos::{repo_router, RepoAppState};

// In your startup fn:
let sync_service = Arc::new(RwLock::new(RepoSyncService::new()));
let model_router = Arc::new(ModelRouter::new(ModelRouterConfig {
    remote_api_key: std::env::var("XAI_API_KEY").unwrap_or_default(),
    ..Default::default()
}));

let repo_state = RepoAppState {
    sync_service: Arc::clone(&sync_service),
    model_router: Arc::clone(&model_router),
};

// Start background scheduler
SyncScheduler::new(SyncSchedulerConfig::default(), Arc::clone(&sync_service)).start();

// Mount routes (add to your existing router)
let app = your_existing_router
    .nest("/api/v1", repo_router(repo_state));
```

---

## .env Additions

```env
# Local model (Ollama)
OLLAMA_BASE_URL=http://localhost:11434
LOCAL_MODEL=qwen2.5-coder:7b

# Force remote for all tasks (debug mode)
FORCE_REMOTE_MODEL=false

# Sync interval in seconds
REPO_SYNC_INTERVAL_SECS=300
```

---

## API Endpoints After Wiring

```
POST   /api/v1/repos                     Register a repo
GET    /api/v1/repos                     List all repos
GET    /api/v1/repos/:id                 Get repo details
DELETE /api/v1/repos/:id                 Remove repo
POST   /api/v1/repos/:id/sync            Trigger manual sync
GET    /api/v1/repos/:id/context         Get LLM prompt context string
GET    /api/v1/repos/:id/todos           Get todos.json
GET    /api/v1/repos/:id/symbols         Get symbols.json
GET    /api/v1/repos/:id/tree            Get tree.txt

POST   /api/v1/chat                      Chat (no repo context)
POST   /api/v1/chat/repos/:id            Chat with repo context injected
```

---

## What's Still TODO After This Scaffold

1. **`call_model_stub`** in `repos_api.rs` — replace with real Ollama + xAI HTTP clients
2. **Redis caching** in chat handler — hash prompt+repo_id → check cache → store response
3. **SQLite persistence** for `RepoSyncService.repos` — currently in-memory only
4. **`syn`-based symbol extraction** — replace naive line scanner with proper AST parsing
5. **Webhook trigger** — GitHub push → `/api/v1/repos/:id/sync` for real-time tree refresh
6. **Embedding pipeline** — chunk changed files on sync → re-embed → update HNSW index

---

## Commit Message Suggestion

```
feat(repo-assistant): add RepoSyncService, ModelRouter, chat API, sync scheduler

- .rustassistant/ cache dir per repo (tree, todos, symbols, context, manifest)
- ModelRouter classifies prompts → routes local (Qwen) vs remote (Grok)
- /api/v1/repos CRUD + /chat endpoints with repo context injection
- Background SyncScheduler (5min interval, configurable)
- TODO: wire real Ollama/xAI clients, Redis cache, SQLite persistence
```
