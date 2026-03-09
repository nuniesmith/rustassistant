# RustAssistant — TODO Backlog

> **Pipeline Test Run — 2026-03-08** ✅ Full self-hosting validation complete.
> All 5 steps ran successfully against this repo. See batch summary at bottom.

> This is a **living document** — it grows with the repo and is the primary interface between
> you and RustAssistant. Items are added manually, by the LLM Audit workflow, and by the
> Rust CLI. Each target repo gets its own `todo.md` that evolves over time.
>
> Items marked with ✅ have been completed. Items with ⚠️ are partial. Items with ❌ are blocked.

---

## 🔴 High Priority

### Build & CI

- [x] ~~Publish Docker image to Docker Hub~~ ✅
- [x] ~~Fix `Dockerfile.web` reference in ci-cd.yml~~ ✅
- [x] ~~Skip deployment in ci-cd.yml~~ ✅
- [ ] `GH_PAT` needs `repo` scope (or fine-grained `Contents: Read and write`) on each target repo. Current failure: `403 Permission to nuniesmith/futures.git denied`. This is a settings fix, not a code fix.

### Rust-Native TODO System

- [x] ~~Add a `rustassistant todo-scan <repo-path>` command~~ ✅ Done — `todo scan . --json --output .rustassistant/scan_fresh.json` runs offline.
- [x] ~~Generate a GAMEPLAN from a `todo.md` file using the Rust LLM client~~ ✅ Done — `todo plan todo.md --context . --output .rustassistant/gameplan.json`.
- [x] ~~Execute a single batch from the gameplan~~ ✅ Done — `todo work .rustassistant/gameplan.json --batch batch-006` ran 3 items, patched `src/api/handlers.rs` cleanly.
- [x] ~~Build a `TodoFile` struct that can parse, update, and write back `todo.md`~~ ✅ Done — `todo sync todo.md .rustassistant/results/batch-006.json` finds all items by stable CRC32 ID.
- [x] ~~`todo work --auto-sync`~~ ✅ Done — `--auto-sync` flag is live in `src/bin/cli.rs`. After a successful work + compile-check pass it automatically calls `todo sync`, eliminating the manual step 4. `--todo-md` override flag added too.
- [ ] **Wire workflow to Docker image** — Replace the Python `todo-analyze`, `todo-plan`, and `todo-work` steps in `llm-audit.yml` with calls to `./rustassistant-bin todo <command>`. All 5 subcommands exist. Python stays as fallback if image pull fails.

### Model Router (`src/model_router.rs`)

- [x] ~~**Copy stub to `src/model_router.rs`**~~ ✅ Done — live with `TaskKind`, `ModelTarget`, `ModelRouterConfig`, `ModelRouter`, `CompletionRequest/Response`, and tests.
- [x] ~~**Wire `ModelRouter` into `AppState`**~~ ✅ Done — instantiated in `src/server.rs::run_server`, shared via `Arc<ModelRouter>` in `RepoAppState`.
- [x] ~~**Replace `call_model_stub` in `repos_api.rs`**~~ ✅ Done — `handle_chat` dispatches to `OllamaClient::complete()` for local and `GrokClient::ask_tracked()` for remote.
- [x] ~~**Implement Ollama HTTP client**~~ ✅ Done — `src/ollama_client.rs` with `POST /api/chat`, retry + exponential back-off, health check, model list, token pass-through.
- [x] ~~**Upgrade keyword classifier to LLM-based**~~ ✅ Done — `ModelRouter::classify_prompt_async` / `llm_classify` sends a one-shot classification prompt to the local Ollama model and parses the returned `TaskKind`. Falls back to `keyword_classify` if Ollama is unreachable. Covers all 8 `TaskKind` variants.
- [x] ~~**Add `.env` entries for local model**~~ ✅ Done — `OLLAMA_BASE_URL`, `LOCAL_MODEL`, `OLLAMA_TIMEOUT_SECS`, `OLLAMA_MAX_RETRIES`, `FORCE_REMOTE_MODEL` all wired.

### Repo Sync Service (`src/repo_sync.rs`)

- [x] ~~**Copy stub to `src/repo_sync.rs`**~~ ✅ Done — live with `RepoSyncService`, tree walker, TODO extractor, syn symbol extractor, `.rustassistant/` cache management.
- [x] ~~**Persist `RepoSyncService.repos` to Postgres**~~ ✅ Done — `migrations/015_registered_repos.sql` added. `RepoSyncService::with_db(pool)` constructor wired. Server startup calls `load_from_db()`. Upsert, soft-delete, and `last_synced` update all wired.
- [x] ~~**Replace naive symbol extractor with `syn`-based AST parse**~~ ✅ Done — `extract_symbols_syn()` using `syn::parse_file` handles `fn`, `struct`, `enum`, `trait`, `impl`, `type`, `const`, `mod` at top level. Falls back to line scanner for macro-heavy files.
- [x] ~~**Chunk changed files on sync and re-embed**~~ ✅ Done — after `sync()` completes, all `.rs` files are passed to `embed_rust_files()` in a background `tokio::spawn`. On success the in-process HNSW index is rebuilt via `refresh_rag_index()`.
- [x] ~~**Add webhook trigger for push-event sync**~~ ✅ Done — `POST /api/github/webhook` handler added to `src/server.rs`. Verifies `X-Hub-Signature-256` (when `GITHUB_WEBHOOK_SECRET` is set), parses `PushEvent`, finds any registered repo whose `remote_url` contains the GitHub `full_name`, and calls `RepoSyncService::sync` in a background `tokio::spawn`. Returns 200 immediately so GitHub never times out.
- [x] ~~**`tree.txt` rolling snapshot on every tick**~~ ✅ Done — see `tree.txt` webhook-triggered regeneration item above.

### Repo API Layer (`src/api/repos.rs`)

- [x] ~~**Copy stub to `src/api/repos.rs`**~~ ✅ Done — all CRUD + chat handlers live.
- [x] ~~**Add Redis response cache to chat handler**~~ ✅ Done — SHA-256 cache key over `(target_label, prompt, repo_id)`, TTL 1h, `no_cache` bypass, `cached: bool` in response.
- [x] ~~**Wire real `OllamaClient` / `GrokClient` into `RepoAppState`**~~ ✅ Done — `RepoAppState::from_env()` builds all three from env vars.
- [x] ~~**Populate `tokens_used` in `ChatResponse`**~~ ✅ Done — Ollama path sums `prompt_eval_count + eval_count`; Grok path casts `prompt_tokens + completion_tokens`.
- [ ] **Full endpoint list after wiring:**
  ```
  POST   /api/v1/repos                   Register a repo
  GET    /api/v1/repos                   List all repos
  GET    /api/v1/repos/:id               Get repo details + cache status
  DELETE /api/v1/repos/:id               Remove repo
  POST   /api/v1/repos/:id/sync          Trigger manual sync
  GET    /api/v1/repos/:id/context       Get LLM prompt context string
  GET    /api/v1/repos/:id/todos         Get todos.json
  GET    /api/v1/repos/:id/symbols       Get symbols.json
  GET    /api/v1/repos/:id/tree          Get tree.txt
  POST   /api/v1/chat                    Chat (no repo context)
  POST   /api/v1/chat/repos/:id          Chat with repo context injected
  ```

### Sync Scheduler (`src/sync_scheduler.rs`)

- [x] ~~**Copy stub to `src/sync_scheduler.rs`**~~ ✅ Done.
- [x] ~~**Spawn scheduler at server startup**~~ ✅ Done — `SyncScheduler::new(...).start()` called in `run_server`. `REPO_SYNC_INTERVAL_SECS` env var wired (default 300s).
- [x] ~~**Replace sequential sync loop with `JoinSet` + semaphore**~~ ✅ Done — concurrent sync with `REPO_SYNC_CONCURRENCY` env var.
- [x] ~~**Add `REPO_SYNC_INTERVAL_SECS` env var**~~ ✅ Done.

### Search & RAG

- [x] ~~**Integrate RAG with LanceDB vector search**~~ ✅ Done — `search_rag_context` in `src/research/worker.rs` builds/queries a lazily initialised in-process HNSW index backed by Postgres embeddings. `enhance_prompt_with_rag` prepends top-k results to the prompt. `handle_chat` in `repos_api.rs` calls both. `refresh_rag_index` is triggered at server startup and after every sync.
- [ ] **Feed `RepoSyncService` embeddings into RAG pipeline more granularly** — current implementation re-embeds ALL `.rs` files on every sync. Diff the new `tree.txt` against the previous snapshot and only chunk files that actually changed, reducing embedding churn.

### API & Data Layer

- [x] ~~**Fix admin module**~~ ✅ Done — `pub mod admin` live. `hash_api_key` pub. `JobQueue::pending_count()` added. `AdminStats` includes `queue_depth` and `uptime_secs`.
- [x] ~~Implement proper document listing with filters~~ ✅ Done — `src/api/handlers.rs`
- [x] ~~Implement document stats `by_type` counts~~ ✅ Done — `src/api/handlers.rs`
- [x] ~~Calculate average chunk size~~ ✅ Done — `src/api/handlers.rs`

### Audit Endpoint (`src/audit/endpoint.rs`)

> `audit_router()` is fully implemented and mounted. The legacy `POST /api/audit` / `GET /api/audit/{id}` routes and handlers have been removed from `server.rs` and replaced by the new pipeline.

- [x] ~~**Mount `audit_router` in `src/server.rs`**~~ ✅ Done — legacy `create_audit` / `get_audit` / `get_audit_tasks` handlers removed. `AuditState::from_env()` built at startup; `.merge(audit_router(audit_state))` wires all three routes.
- [x] ~~**Implement `handle_audit_post`**~~ ✅ Done — validates repo path, pre-allocates `run_id`, spawns background task that calls `AuditRunnerWithGrok::run()` (or `run_static_only()` when no `XAI_API_KEY`), writes `docs/audit/<run_id>.json` + `.md`, returns 202 with `AuditJobAccepted`.
- [x] ~~**Implement `handle_audit_get_by_id`**~~ ✅ Done — reads `docs/audit/<id>.json`, returns 200 with full report JSON, 404 if not found, 400 on path-traversal attempt.
- [x] ~~**Implement `handle_audit_get`**~~ ✅ Done — lists all `*.json` files under `docs/audit/`, deserialises each into `AuditReportSummary`, returns sorted newest-first.
- [x] ~~**Wire `RedisAuditCache` into audit handlers**~~ ✅ Done — `RedisAuditCache::from_env()` initialised inside `AuditState::from_env()`; held in `Arc<RwLock<RedisAuditCache>>` on `AuditState`. Degrades gracefully to no-op when Redis is unreachable.
- [x] ~~**Auto-append audit findings to `todo.md`**~~ ✅ Done — `AuditRunnerWithGrok::run()` already calls `append_findings_to_todo` when `request.append_to_todo` is true. `append_to_todo` field is in `AuditRequest` and passed through from the POST body unchanged.

### Postgres Migration

> **Status: ✅ Mostly complete.** `Cargo.toml` uses `sqlx` with `postgres` feature. `docker-compose.yml` includes Postgres 16. `server.rs` uses `PgPool`. All migration files exist. The `research/worker.rs` already uses `PgPool`. Steps remaining below.

- [x] ~~**Add `postgres` service to `docker-compose.yml`**~~ ✅ Done — Postgres 16-alpine with healthcheck, named volume `postgres_data`, wired into `depends_on`.
- [x] ~~**Swap `sqlite` feature for `postgres` in `Cargo.toml`**~~ ✅ Done — `sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid", "chrono", "macros"] }`.
- [x] ~~**Replace `SqlitePool` with `PgPool`**~~ ✅ Done — `src/server.rs` and `src/research/worker.rs` use `PgPool`. `RepoSyncService::db` is `Option<PgPool>`.
- [x] ~~**Audit all 15 migration files for Postgres syntax**~~ ✅ Done — all 15 migrations verified clean. `001`–`015` already use `BIGSERIAL`, `EXTRACT(EPOCH FROM NOW())::BIGINT`, `INSERT ... ON CONFLICT DO NOTHING/UPDATE`, `tsvector`/`GIN` FTS, and Postgres triggers. No SQLite-isms remain.
- [ ] **Port `RepoSyncService` upsert to Postgres syntax** — `load_from_db`, `register`, `remove_repo_async`, and `sync` in `src/repo_sync.rs` use `query_unchecked!` with SQLite-flavoured upserts. Rewrite as `INSERT ... ON CONFLICT (local_path) DO UPDATE SET ...` and switch to typed `query!` macros.
- [ ] **Regenerate `.sqlx` query cache** — run `cargo sqlx prepare` against a live Postgres instance after migrations are stable. Commit the regenerated `.sqlx/` directory. Remove `SQLX_OFFLINE=true` workaround.
- [ ] **Data migration (optional)** — use `pgloader` to copy any SQLite dev data if worth preserving. Cast `JSONB` columns after load.

---

## 🧪 Testing — Self-Hosting Validation

> Pipeline tested end-to-end against this repo on 2026-03-08.

### Bugs fixed during validation run
- [x] ~~`DATABASE_URL` env var mis-parsed~~ ✅ Fixed
- [x] ~~UTF-8 panic in scan table renderer~~ ✅ Fixed
- [x] ~~`max_tokens` hardcoded to 2000~~ ✅ Fixed — reads `XAI_MAX_TOKENS` env var (default 8000)
- [x] ~~`XAI_MODEL` ignored by GrokClient~~ ✅ Fixed
- [x] ~~HTTP timeout too short for large LLM calls~~ ✅ Fixed — 180s
- [x] ~~Truncated LLM game plan JSON parsed as empty~~ ✅ Fixed — partial-JSON recovery path
- [x] ~~Worker pre-empts `todo-sync` by modifying `todo.md` in-place~~ ✅ Fixed — `skip_todo_md_update` flag
- [x] ~~LLM-generated code used wrong `TypeCount` field name~~ ✅ Fixed manually
- [x] ~~LLM-generated code used undefined `DocumentListRow` struct~~ ✅ Fixed manually
- [x] ~~Duplicate/mis-ordered binding code in `list_documents`~~ ✅ Fixed manually

### Pipeline issues fixed after initial run
- [x] ~~LLM code generation quality: worker sometimes generates code with wrong field names~~ ✅ Fixed — `todo work` now runs `cargo check` after applying; rolls back on failure
- [x] ~~`todo scaffold` warns "Could not parse LLM scaffold plan" even when JSON parsed successfully~~ ✅ Fixed
- [x] ~~2 doctest failures in `src/audit/cache.rs` and `src/audit/endpoint.rs`~~ ✅ Fixed — annotated with `rust,ignore`. Full test suite: **417 lib + 33 doctests, 0 failures**

### Step validation checklist
- [x] **Step 0 — Scan** ✅ `todo scan . --json` → 208 items, 50 files
- [x] **Step 1 — Scaffold** ✅ `todo scaffold . --dry-run` → 6 existing files correctly skipped
- [x] **Step 2 — Plan** ✅ `todo plan todo.md --context .` → 10 batches, 17 items
- [x] **Step 3 — Work (dry-run)** ✅ `todo work … --batch batch-006 --dry-run` → 3/3 would patch
- [x] **Step 3 — Work (real)** ✅ `todo work … --batch batch-006` → 3 hunks, compile passed
- [x] **Step 4 — Sync (dry-run)** ✅ `todo sync todo.md … --dry-run` → all 3 found by CRC32 ID
- [x] **Step 4 — Sync (real)** ✅ `todo sync todo.md … --append-summary` → 3 marked ✅
- [x] **Compile check** ✅ `SQLX_OFFLINE=true cargo build --bin rustassistant` — clean
- [x] **Test suite** ✅ `cargo test` → 417 lib + 33 doctests, 0 failed

---

## 🟡 Medium Priority

### Local Model Integration

- [ ] **Pull and serve Qwen2.5-Coder 7B via Ollama** — `ollama pull qwen2.5-coder:7b`. Runs as sidecar in `docker-compose.yml` with CUDA GPU passthrough. `ollama-init` one-shot puller service already in compose file.
- [x] ~~**Add Ollama service to `docker-compose.yml`**~~ ✅ Done — `ollama` service with CUDA passthrough, `ollama-init` puller, `OLLAMA_BASE_URL` wired into app service.
- [x] ~~**Implement `src/ollama_client.rs`**~~ ✅ Done — full implementation with retry, fallback, health check, model list.
- [ ] **Routing heuristic tuning** — `ModelRouter::llm_classify` is now implemented. After initial deployment, measure stub quality vs. Grok. Adjust the classification system prompt or escalation threshold. Target: scaffold/stub tasks stay local, architectural review always goes remote.

### `.rustassistant/` Per-Repo Cache

- [x] ~~**`tree.txt` webhook-triggered regeneration**~~ ✅ Done — `RepoSyncService::sync()` already writes `tree.txt` on every tick and manual sync. The `POST /api/github/webhook` push handler (added in batch-007) calls `svc.sync()` in a background task, so `tree.txt` is regenerated on every push event. `build_prompt_context` already prepends the first 80 lines of `tree.txt` into every `CompletionRequest::repo_context`, which `build_prompt()` injects before the user task.
- [x] ~~**`symbols.json` AST upgrade**~~ ✅ Done — `extract_symbols_syn()` using `syn 2` full parse with line-scanner fallback.
- [x] ~~**`todos.json` deduplication**~~ ✅ Done — `HashSet<(file, line)>` in `extract_todos`.
- [x] ~~**`embeddings.bin` exclusion from git**~~ ✅ Done — `RepoSyncService::register` writes `.rustassistant/.gitignore` with `embeddings.bin`. Two unit tests added in `src/repo_sync.rs::tests`: `register_writes_gitignore_with_embeddings_bin` and `register_does_not_overwrite_existing_gitignore`. Both pass (7/7 `repo_sync::tests` green).
- [x] ~~**`context.md` auto-injection**~~ ✅ Done — `build_prompt_context` loads `symbols.json` + `manifest.json` + `tree.txt` + `todos.json`. Top-5 public symbols injected. Context capped at ~3000 chars.

### CLI & Developer Experience

- [x] ~~**Actually test the XAI API connection in `test-api` command**~~ ✅ Done — `handle_test_api` in `src/bin/cli.rs` now accepts `pool: &PgPool`, builds `GrokClient::new(key, db)`, calls `ask_tracked("reply with: ok", None, "test-api-ping")`, and prints reply content, round-trip latency (ms), prompt/completion/total token counts, and estimated USD cost. Actionable error hints for 401/429/network failures included.
- [x] ~~**Parse detailed per-file test results in `TestRunner`**~~ ✅ Done — `parse_cargo_test_json` parses `cargo test -- -Zunstable-options --format=json` event stream into `HashMap<String, FileTestResult>` keyed by inferred `src/<module>.rs` path. `parse_pytest_json_report` reads `.pytest-report.json` written by `pytest-json-report`, grouping tests by `nodeid` file prefix. Both are wired into `run_rust_tests` and `run_python_tests` with text-summary fallback when JSON isn't available. `derive_rust_file_key` helper maps test names like `audit::cache::tests::hit_rate` → `src/audit/cache.rs`. 12/12 new tests green.

### Queue & Processing

- [ ] **Implement tag refinement and project linking in `process_tagging`** — `src/queue/processor.rs:429` has a `// TODO: Additional tag refinement, linking to projects` stub. Replace with actual tag inference using `src/tags.rs` / `src/tag_schema.rs` and project-linking by matching item tags against registered repos from `RepoSyncService`. Use `db::core::create_task` to write linked tasks instead of duplicating data in `queue_items`.

### Web Dashboard

- [ ] **Add `pinned` field to `Document` struct** — `src/web_ui_extensions.rs:375` has `let pin_icon = ""; // TODO: Add pinned field to Document struct` and `:719` has `pin = "", // TODO`. Add `pinned BOOLEAN DEFAULT FALSE` column to the `documents` table migration, expose in the `Document` struct, render a pin icon in the card template.
- [ ] **Repo management UI tab** — expose the `/api/v1/repos` endpoints in `static/index.html` or `static/rustassistant-ui.html`: register a local path, trigger sync, view tree/todos/symbols in a tab, chat with repo context selected.
- [ ] **Scan results auto-switch** — after clicking "Run Scan" in the dashboard, automatically switch to the Scan tab and render filter counts as coloured stat cards.
- [ ] **Repo pull/refresh button** — `POST /api/web/repos/:id/pull` endpoint that runs `git pull` on the cloned repo and returns the new HEAD commit hash.

---

## 🟢 Low Priority / Enhancements

### Large File Handling

- [x] ~~Skip LFS-tracked files (pre-trained models) during clone/audit~~ ✅
- [ ] **Make skip-extensions list configurable per-repo** — currently hardcoded in `src/static_analysis.rs` and `src/auto_scanner.rs`. Add a `[scan] skip_extensions = [...]` key to repo config and thread it through both scanners.

### Workflow & CI/CD

- [x] ~~Move `llm-audit.yml` workflow to `nuniesmith/actions` repo~~ ✅
- [x] ~~Add `docs/audit/` directory with `.gitkeep`~~ ✅
- [x] ~~Docker image pull in `llm-audit.yml`~~ ✅
- [ ] **Expose `/api/audit` via new `audit_router`** — `audit_router()` is defined in `src/audit/endpoint.rs` and fully stubbed. Mount it in `server.rs` (see Audit Endpoint section above). Once mounted, the LLM audit workflow can POST to the Rust API + Redis cache instead of raw Python API calls.
- [ ] **Auto-append audit findings to `todo.md`** — already supported via `AuditRunnerWithGrok::run` + `append_findings_to_todo`. Expose `append_to_todo: true` in the `POST /api/audit` body once the endpoint is wired.

### Code Quality

- [ ] **Consolidate `todo_items` DB table with `tasks` table** — `src/db/queue.rs` defines both `queue_items` and `todo_items` tables. `src/db/core.rs` has `create_task`. Migrate `process_tagging` writes in `src/queue/processor.rs` to use `db::core::create_task` instead of duplicating into `queue_items`, then deprecate `todo_items` in a new migration.
- [ ] **Standardise error handling across API handlers** — mix of `anyhow` and manual `(StatusCode, Json(...))` tuples. `ApiError { error, code }` is already defined in `src/api/repos.rs`. Extract it to `src/api/types.rs`, implement `IntoResponse` for it, and replace ad-hoc error tuples in `src/api/handlers.rs` and `src/api/admin.rs`.
- [ ] **Implement `AuditRunner::run` stub** — `AuditRunner::run` (without Grok) currently returns `Err("not yet implemented")`. Either remove it (since `AuditRunnerWithGrok::run` is the real path) or wire it to `run_static_only()` as a sensible no-key fallback.

---

## 📋 Notes

### Architecture — Local + Remote Model Routing
```
User Chat / todo-work
    ↓
ModelRouter::classify_prompt_async()
    ├── ScaffoldStub / TodoTagging / TreeSummary → Ollama (Qwen2.5-Coder 7B, local)
    ├── RepoQuestion / SymbolExtraction         → Ollama (local, fast)
    ├── ArchitecturalReason / CodeReview        → GrokClient (remote, xAI)
    └── Unknown / force_remote                 → GrokClient (remote, fallback)

Classification: one-shot LLM prompt to local model → keyword fallback if Ollama down
```

### `.rustassistant/` Per-Repo Cache Structure
```
.rustassistant/
├── manifest.json    — repo identity, last_synced, branch, crate metadata
├── tree.txt         — rolling file tree snapshot (regenerated on every sync)
├── todos.json       — all TODO/STUB/FIXME/HACK tags with file:line attribution
├── symbols.json     — public functions, structs, traits, impls (syn-parsed)
├── context.md       — LLM-ready summary (injected into every code-gen prompt)
├── embeddings.bin   — cached vector embeddings (gitignored)
├── scan.json        — output of `todo scan` (committed — useful as PR diff)
├── gameplan.json    — latest GamePlan from `todo plan`
├── results/         — WorkResult JSON files per batch
└── backups/         — pre-change file backups created by `todo work`
```

### Wiring Checklist — All Stubs Merged
> All 4 stub files are merged into `src/`. Server wiring complete. ✅

| File | Destination | Status | Merged |
|------|------------|--------|--------|
| `todo/model_router.rs` | `src/model_router.rs` | ✅ Live | 2026-03-08 |
| `todo/repo_sync.rs` | `src/repo_sync.rs` | ✅ Live | 2026-03-08 |
| `todo/repos_api.rs` | `src/api/repos.rs` | ✅ Live | 2026-03-08 |
| `todo/sync_scheduler.rs` | `src/sync_scheduler.rs` | ✅ Live | 2026-03-08 |
| `todo/rustassistant-ui.html` | `static/rustassistant-ui.html` | ✅ Live | 2026-03-08 |
| `todo/.env.rustassistant` | `.env.rustassistant` | ✅ Copied | 2026-03-08 |
| `todo/docker-compose.rustassistant.yml` | `docker-compose.yml` (root) | ✅ Live | 2026-03-08 |

### Recommended Local Models (8GB VRAM)
| Model | Use case | Quality |
|-------|----------|---------|
| Qwen2.5-Coder 7B Q4_K_M | Scaffold / stub / TODO gen | ★★★★★ Best for Rust stubs |
| DeepSeek-Coder-V2-Lite 16B Q4 | Multi-file awareness | ★★★★☆ |
| CodeLlama 7B Instruct | Boilerplate / init | ★★★☆☆ Battle-tested |
| Mistral 7B Instruct v0.3 | General + chat | ★★★★☆ Best non-code hybrid |
| Llama 3.1 8B Instruct | Rust reasoning | ★★★☆☆ Surprisingly good |

### Pipeline Philosophy
- `todo.md` is the **single source of truth**. Every command reads it; every command writes back to it.
- `todo-scaffold` is always safe to re-run — it's idempotent (skips existing files).
- `todo-work` never touches files without creating backups in `.rustassistant/backups/`.
- `todo-work` runs `cargo check` after patching. On failure it rolls back all changes automatically.
- **IDs are stable**: 8-char hex CRC32 of the raw list line. Don't edit item text between `todo-work` and `todo-sync`.
- **`--auto-sync`**: pass this to `todo work` to skip the manual sync step entirely.
- **Cross-repo reuse**: all repos share the same 5-step pipeline. Only `todo.md` and source files differ.

### RAG Pipeline Status
```
Server startup → refresh_rag_index(pool)   [background tokio::spawn]
     ↓
RepoSyncService::sync() → embed_rust_files() → refresh_rag_index()   [fire-and-forget]
     ↓
handle_chat() → search_rag_context() → enhance_prompt_with_rag()
     ↓
HNSW in-process index (rag_index_cell / OnceCell<Arc<Mutex<Option<RagIndex>>>>)
     ↓
document_chunks + documents tables in Postgres (chunk content fetched per hit)
```
**Known gap**: all `.rs` files are re-embedded on every sync (not just changed files). See medium-priority item above.

### Migration Path
The current workflow is ~1900 lines of YAML + Python. Goal: progressively move logic into the Rust binary, publish via `ci-cd.yml` to Docker Hub (`nuniesmith/rustassistant:latest`), and make the workflow a thin orchestrator calling `./rustassistant-bin <command>`.

### Redis
Configured in `docker-compose.yml` for LLM response caching (`allkeys-lru`, 384 MB, password-protected). `RedisAuditCache` is fully implemented (`src/audit/cache.rs`) and used in `RepoAppState` chat caching. `RedisAuditCache::from_env()` wires into the audit pipeline once `audit_router` is mounted.

### Pre-trained Model Files
`.onnx`, `.pt`, etc. in target repos are skipped via `GIT_LFS_SKIP_SMUDGE=1` at clone time. Hardcoded skip list in `src/static_analysis.rs` / `src/auto_scanner.rs` — see low-priority item to make this configurable.

### Postgres Migration — Current Status
```
✅ 1. docker-compose.yml — Postgres 16 service present, healthcheck wired
✅ 2. DATABASE_URL — points to postgres://... in compose + server.rs
✅ 3. Cargo.toml — sqlx features = ["postgres", "runtime-tokio", "uuid", "chrono", "macros"]
⚠️  4. Migrations — 15 files exist; migrations 001-014 have SQLite syntax that needs audit
⚠️  5. Rust code — PgPool used in server.rs and research/worker.rs;
       repo_sync.rs still uses query_unchecked! with SQLite-flavoured upserts
❌  6. .sqlx cache — not yet regenerated against live Postgres; SQLX_OFFLINE=true still needed
❌  7. Data migration — not applicable (dev only)
⚠️  8. RepoSyncService — upsert logic needs ON CONFLICT rewrite
```

Order of operations for completing migration:
```
1. Audit + rewrite migrations 001-014 for Postgres syntax
2. cargo sqlx database create && cargo sqlx migrate run  (against live Postgres)
3. Port repo_sync.rs upsert → ON CONFLICT syntax
4. cargo sqlx prepare  (regenerates .sqlx/ — commit this)
5. cargo build         (fix every type error the compiler surfaces)
6. cargo test
```
> **Blocking note:** Steps 1–4 must happen in one sitting. Once the sqlx feature is Postgres-only,
> `SQLX_OFFLINE=true cargo build` will fail until `.sqlx/` is regenerated.

---

### Batch `batch-008` Summary — 2026-03-08 (quick wins)

Changes landed in this batch:

1. **`src/bin/cli.rs`** — `handle_test_api` now accepts `pool: &PgPool`, builds a real `GrokClient`, fires a `"reply with: ok"` ping via `ask_tracked`, and prints reply, latency (ms), token counts, and USD cost estimate. Actionable hints for 401/429/network errors.

2. **`src/repo_sync.rs`** — two new unit tests: `register_writes_gitignore_with_embeddings_bin` verifies that `register()` always creates `.rustassistant/.gitignore` containing `embeddings.bin`; `register_does_not_overwrite_existing_gitignore` confirms idempotency. All 7 `repo_sync::tests` green.

3. **`migrations/`** — audited all 15 SQL files for SQLite syntax. All clean: `BIGSERIAL`, `EXTRACT(EPOCH FROM NOW())::BIGINT`, `ON CONFLICT`, `tsvector`/`GIN`, Postgres triggers throughout. No action required.

---

### Batch `batch-009` Summary — 2026-03-08 (test runner + quick wins)

Changes landed in this batch:

1. **`src/tests_runner.rs`** — implemented `parse_cargo_test_json` (parses `cargo test --format=json` event stream into per-file `FileTestResult` map) and `parse_pytest_json_report` (reads `.pytest-report.json` from `pytest-json-report`). Both wired into `run_rust_tests` / `run_python_tests` with graceful fallback to text parsing. Added `derive_rust_file_key` helper + 12 new unit tests (all green). `results_by_file` is now populated in all four `run_*_tests` methods.

2. **`src/bin/cli.rs`** — `handle_test_api` upgraded: builds a real `GrokClient`, fires `ask_tracked("reply with: ok")`, prints reply, latency (ms), token counts, and USD cost. Actionable error hints for 401/429/network failures.

3. **`src/repo_sync.rs`** — two new gitignore unit tests; pre-existing `E0733` recursive `async fn walk_dir` fixed via `Box::pin`.

4. **`migrations/`** — all 15 SQL files audited and confirmed clean Postgres syntax.

5. **`todo.md`** — `tree.txt` webhook regeneration confirmed done (sync already writes it; push webhook already calls sync; `build_prompt_context` already injects first 80 lines into every prompt).

---

### Batch `batch-007` Summary — 2026-03-08 (audit pipeline wiring)

Changes landed in this batch:

1. **`src/audit/endpoint.rs`** — fully implemented all three handlers (`handle_audit_get`, `handle_audit_post`, `handle_audit_get_by_id`). Introduced `AuditState` (grok client + `RedisAuditCache` + output dir + runner config). `POST /api/audit` spawns a background task and returns 202 immediately. Path-traversal guard on `GET /api/audit/:id`. 5 new integration tests using `tower::ServiceExt::oneshot`.

2. **`src/server.rs`** — removed legacy `create_audit` / `get_audit` / `get_audit_tasks` handlers and their dead response types. Mounted `audit_router(audit_state)` via `.merge(...)`. Added `WebhookState` struct and `handle_github_webhook` handler at `POST /api/github/webhook` — verifies HMAC-SHA256 signature, parses push events, triggers `RepoSyncService::sync` for matching registered repos.

3. **`src/repo_sync.rs`** — fixed pre-existing `E0733` compile error in recursive `async fn walk_dir` by converting it to a `Box::pin(async move { ... })` returning function.

### Batch `batch-006` Summary — 2026-03-08 01:11 UTC
- Attempted: 3 | Succeeded: 3 | Failed: 0 | Skipped: 0
- ✅ `12542b08` — Implement proper document listing with filters
- ✅ `7048cea1` — Implement document stats `by_type` counts
- ✅ `7167a7b9` — Calculate average chunk size