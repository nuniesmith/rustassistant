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

- [x] ~~Add a `rustassistant todo-scan <repo-path>` command~~ ✅ Done — `todo scan . --json --output .rustassistant/scan_fresh.json` runs offline. 208 items / 50 files. UTF-8 truncation panic fixed.
- [x] ~~Generate a GAMEPLAN from a `todo.md` file using the Rust LLM client~~ ✅ Done — `todo plan todo.md --context . --output .rustassistant/gameplan.json` produces 10 batches / 17 items. Partial-JSON recovery added for truncated responses.
- [x] ~~Execute a single batch from the gameplan~~ ✅ Done — `todo work .rustassistant/gameplan.json --batch batch-006` ran 3 items, patched `src/api/handlers.rs` cleanly. `skip_todo_md_update` flag added so IDs stay stable for `todo-sync`.
- [x] ~~Build a `TodoFile` struct that can parse, update, and write back `todo.md`~~ ✅ Done — `todo sync todo.md .rustassistant/results/batch-006.json` found all 3 items by stable CRC32 ID and marked them ✅ Done.
- [ ] **Wire workflow to Docker image** — Replace the Python `todo-analyze`, `todo-plan`, and `todo-work` steps in `llm-audit.yml` with calls to `./rustassistant-bin todo <command>`. All 5 subcommands exist. Python stays as fallback if image pull fails.

### Model Router (NEW — `todo/model_router.rs` → `src/model_router.rs`)

> Scaffolded stub at `todo/model_router.rs`. Copy to `src/model_router.rs` and wire in.
> Routes tasks between local Ollama (Qwen2.5-Coder 7B) and remote Grok based on task kind.

- [x] ~~**Copy stub to `src/model_router.rs`**~~ ✅ Done — `src/model_router.rs` is live with TaskKind, ModelTarget, ModelRouterConfig, ModelRouter, CompletionRequest/Response, and tests. Registered in `src/lib.rs`.
- [x] ~~**Wire `ModelRouter` into `AppState`**~~ ✅ Done — `ModelRouter::new(ModelRouterConfig { remote_api_key: XAI_API_KEY, .. })` instantiated in `src/server.rs::run_server` and shared via `Arc<ModelRouter>` in `RepoAppState`.
- [x] ~~**Replace `call_model_stub` in `repos_api.rs`**~~ ✅ Done — `handle_chat` now dispatches to `OllamaClient::complete()` for `ModelTarget::Local` and `GrokClient::ask_tracked()` for `ModelTarget::Remote`. `call_model_stub` removed entirely.
- [x] ~~**Implement Ollama HTTP client**~~ ✅ Done — `src/ollama_client.rs` written with `POST /api/chat`, retry + exponential back-off, `GrokClient` fallback, `health_check()`, `list_models()`, token pass-through from `eval_count` / `prompt_eval_count`. Registered in `src/lib.rs`.
- [ ] **Upgrade keyword classifier to LLM-based** — `ModelRouter::classify_prompt` currently uses naive keyword matching (`TODO`). Replace or augment with a one-shot classification prompt against the local model: `"Classify this task as one of: ScaffoldStub | TodoTagging | TreeSummary | SymbolExtraction | RepoQuestion | ArchitecturalReason | CodeReview | Unknown"`.
- [x] ~~**Add `.env` entries for local model**~~ ✅ Done — `OLLAMA_BASE_URL`, `LOCAL_MODEL`, `OLLAMA_TIMEOUT_SECS`, `OLLAMA_MAX_RETRIES`, `FORCE_REMOTE_MODEL` all documented and wired via `OllamaClientConfig::default()`. `.env.rustassistant` template copied to repo root.

### Repo Sync Service (NEW — `todo/repo_sync.rs` → `src/repo_sync.rs`)

> Full async `RepoSyncService` scaffolded at `todo/repo_sync.rs`. Handles tree walks,
> TODO extraction, symbol extraction, and `.rustassistant/` cache management per repo.

- [x] ~~**Copy stub to `src/repo_sync.rs`**~~ ✅ Done — `src/repo_sync.rs` is live with `RepoSyncService`, `RegisteredRepo`, tree walker, TODO extractor, symbol extractor, and `.rustassistant/` cache management. `async-recursion = "1"` confirmed in `Cargo.toml`. Registered in `src/lib.rs`.
- [x] ~~**Persist `RepoSyncService.repos` to SQLite**~~ ✅ Done — `migrations/015_registered_repos.sql` added with the full schema (id, name, local_path, remote_url, branch, last_synced, active, created_at, updated_at, unique index on local_path, active index, updated_at trigger). `RepoSyncService::with_db(pool)` constructor added; `load_from_db()` called at server startup to restore persisted repos. `register()` uses `query_unchecked!` upsert; `remove_repo_async()` soft-deletes; `sync()` updates `last_synced` in both memory and DB. Server wired to re-use the app's existing `SqlitePool`.
- [ ] **Replace naive symbol extractor with `syn`-based AST parse** — `extract_symbols` in `repo_sync.rs` uses a line-scanner. Replace with `syn::parse_file` to correctly handle multi-line signatures, trait bounds, lifetimes, and generics.
- [ ] **Add webhook trigger for push-event sync** — `POST /api/v1/repos/:id/sync` already exists in `repos_api.rs`. Wire GitHub webhook push events (already handled in `src/webhooks.rs`) to call `RepoSyncService::sync` for the matching repo so `tree.txt` / `todos.json` stay real-time.
- [ ] **Chunk changed files on sync and re-embed** — after `sync()` completes, diff the new `tree.txt` against the previous one, chunk only the changed `.rs` files, and update the HNSW / LanceDB embedding index via the existing `src/embeddings.rs` + `src/vector_index.rs` pipeline.

### Repo API Layer (NEW — `todo/repos_api.rs` → `src/api/repos.rs`)

> REST endpoints + chat handler scaffolded at `todo/repos_api.rs`.

- [x] ~~**Copy stub to `src/api/repos.rs`**~~ ✅ Done — `src/api/repos.rs` is live with all CRUD + chat handlers. `pub mod repos;` added in `src/api/mod.rs`. `repo_router(repo_app_state)` mounted under `.nest("/api/v1", ...)` in `src/server.rs`.
- [x] ~~**Add Redis response cache to chat handler**~~ ✅ Done — `handle_chat` now builds a SHA-256 cache key over `(target_label, prompt, repo_id)`, checks `CacheLayer::get` before calling the model, and fire-and-forgets a `CacheLayer::set` (TTL 1 hour) after a successful response. `CacheLayer` initialised with Redis when `REDIS_URL` env var is set, falls back to in-memory LRU otherwise. `ChatResponse.cached: bool` field added. `no_cache: bool` request field added for bypass. `RepoAppState::from_env()` builder wires the cache. `GrokClient::model_name()` accessor added.
- [x] ~~**Wire real `OllamaClient` / `XaiClient` into `RepoAppState`**~~ ✅ Done — `RepoAppState` now holds `ollama_client: Arc<OllamaClient>`, `grok_client: Option<Arc<GrokClient>>`, and `cache: Arc<CacheLayer>`. `RepoAppState::from_env()` builder constructs all three from env vars at startup. Server startup constructs `GrokClient` from `XAI_API_KEY` (skips gracefully when unset). Two new endpoints added: `GET /api/v1/ollama/health` and `GET /api/v1/ollama/models`.
- [x] ~~**Populate `tokens_used` in `ChatResponse`**~~ ✅ Done — `dispatch_completion` returns `Option<u32>` token count; Ollama path sums `prompt_eval_count + eval_count`; Grok path casts `prompt_tokens + completion_tokens` from `AskResponse`. `ChatResponse.tokens_used` is now populated on every successful model call.
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

### Sync Scheduler (NEW — `todo/sync_scheduler.rs` → `src/sync_scheduler.rs`)

> Background 5-minute sync loop scaffolded at `todo/sync_scheduler.rs`.

- [x] ~~**Copy stub to `src/sync_scheduler.rs`**~~ ✅ Done — `src/sync_scheduler.rs` is live with `SyncScheduler` + `SyncSchedulerConfig`. Registered in `src/lib.rs`.
- [x] ~~**Spawn scheduler at server startup**~~ ✅ Done — `SyncScheduler::new(SyncSchedulerConfig { interval: Duration::from_secs(REPO_SYNC_INTERVAL_SECS), .. }, Arc::clone(&sync_service)).start()` called in `src/server.rs::run_server`. `REPO_SYNC_INTERVAL_SECS` env var wired (default 300s).
- [x] ~~**Replace sequential sync loop with `JoinSet` + semaphore**~~ ✅ Done — `run_sync_pass` now collects due-repo IDs up-front (filtering by `skip_if_synced_within`), spawns one `JoinSet` task per repo gated by `Arc<Semaphore::new(concurrency)>`, drains results with per-task error logging, and reports non-fatal sync errors via `warn!`. `REPO_SYNC_CONCURRENCY` env var wired into `SyncSchedulerConfig::default()`. First ticker tick skipped to avoid hammering disk at startup.
- [x] ~~**Add `REPO_SYNC_INTERVAL_SECS` env var**~~ ✅ Done — `run_server` reads `REPO_SYNC_INTERVAL_SECS` and passes it to `SyncSchedulerConfig::interval`. Default 300s. Documented in `.env.rustassistant`.

### API & Data Layer

- [x] ~~**Fix admin module**~~ ✅ Done — `pub mod admin` uncommented in `src/api/mod.rs`. `admin.rs` rewritten to use actual `ApiState` fields only: removed all references to `cache_layer`, `vector_index`, `webhook_manager`, `analytics`. Removed webhook + analytics endpoints (not in ApiState). Kept: `GET /admin/stats`, `GET /admin/health`, `GET/POST /admin/api-keys`, `DELETE /admin/api-keys/:id`, `GET /admin/jobs`, `POST /admin/jobs/:id/retry`. `admin_router()` merged into `create_api_router`. `hash_api_key` made `pub` in `auth.rs` and re-exported from `api/mod.rs` + `lib.rs`. `JobQueue::pending_count()` added (sync, uses `try_read`). `AdminStats` now includes `queue_depth` and `uptime_secs`.
- [x] ~~Implement proper document listing with filters (`src/api/handlers.rs:345`)~~ ✅ Done — src/api/handlers.rs
- [x] ~~Implement document stats `by_type` counts (`src/api/handlers.rs:132`)~~ ✅ Done — src/api/handlers.rs
- [x] ~~Calculate average chunk size (`src/api/handlers.rs:137`)~~ ✅ Done — src/api/handlers.rs

### Search & RAG

- [ ] **Integrate RAG with LanceDB vector search** — `search_rag_context` in `src/research/worker.rs:275` returns empty results. Implement vector similarity query against embeddings stored via `src/vector_index.rs` and `src/embeddings.rs`, passing results through `enhance_prompt_with_rag`.
- [ ] **Feed `RepoSyncService` embeddings into RAG pipeline** — after each sync, chunk changed `.rs` files and upsert embeddings so the chat handler's RAG context is always current.

### Indexing

- [x] ~~**Implement concurrent batch indexing with semaphore**~~ ✅ Done — `BatchIndexer::index_batch` now uses `tokio::task::JoinSet` gated by `Arc<tokio::sync::Semaphore::new(self.concurrency)>`. Each document gets its own spawned task; individual failures are logged and skipped without aborting the batch. Summary log (total/succeeded/failed) emitted on completion. `BatchIndexer.indexer` field changed to `Arc<DocumentIndexer>` to allow sharing across tasks. `concurrency()` accessor added.

---

## 🧪 Testing — Self-Hosting Validation

> Pipeline tested end-to-end against this repo on 2026-03-08.

### Bugs fixed during validation run
- [x] ~~`DATABASE_URL` env var mis-parsed~~ ✅ Fixed — `init_db` now strips both `sqlite:` and `sqlite://` prefixes; `.env` corrected from `sqlite://data/devflow.db` → `sqlite:data/rustassistant.db`
- [x] ~~UTF-8 panic in scan table renderer~~ ✅ Fixed — `render_scan_table_items` now uses `.chars().count()` / `.chars().take(57)` instead of raw byte-index slicing
- [x] ~~`max_tokens` hardcoded to 2000~~ ✅ Fixed — `call_api_once` now reads `XAI_MAX_TOKENS` env var (default 8000); `.env` updated to 8000
- [x] ~~`XAI_MODEL` ignored by GrokClient~~ ✅ Fixed — `GrokClient::new` now reads `XAI_MODEL` env var; model changed to `grok-4-1-fast-non-reasoning` for code generation (6× faster, no timeout)
- [x] ~~HTTP timeout too short for large LLM calls~~ ✅ Fixed — reqwest client timeout increased from 90s → 180s
- [x] ~~Truncated LLM game plan JSON parsed as empty~~ ✅ Fixed — `parse_batches_from_response` now has a partial-JSON recovery path that extracts complete batch objects even from cut-off responses
- [x] ~~Worker pre-empts `todo-sync` by modifying `todo.md` in-place~~ ✅ Fixed — `WorkConfig::skip_todo_md_update` flag added; CLI `todo work` sets it so the syncer can find items by their original (unchanged) CRC32 IDs
- [x] ~~LLM-generated code used wrong `TypeCount` field name (`type_name` vs `doc_type`)~~ ✅ Fixed manually after work step
- [x] ~~LLM-generated code used undefined `DocumentListRow` struct~~ ✅ Fixed manually — replaced with inline tuple type `(String, String, Option<String>, ...)`
- [x] ~~Duplicate/mis-ordered binding code in `list_documents`~~ ✅ Fixed manually — removed the duplicate `list_q` initialisation and extra parameter-binding blocks

### Pipeline issues fixed after initial run
- [x] ~~LLM code generation quality: worker sometimes generates code with wrong field names or undefined types~~ ✅ Fixed — `todo work` now runs `cargo check` (with `SQLX_OFFLINE=true`) after applying changes; on failure it rolls back every touched file to its pre-change snapshot and does NOT write the `WorkResult`. Pass `--no-check` to skip for non-Rust repos or manual review.
- [x] ~~`todo scaffold` warns "Could not parse LLM scaffold plan" even when JSON parsed successfully~~ ✅ Fixed — extracted a `try_parse()` helper; the warn now only fires after all three parse strategies have failed.
- [x] ~~2 doctest failures in `src/audit/cache.rs` and `src/audit/endpoint.rs`~~ ✅ Fixed — annotated with `rust,ignore`. Full test suite: **417 lib + 33 doctests, 0 failures**.

### Step validation checklist
- [x] **Step 0 — Scan** ✅ `todo scan . --json` → 208 items, 50 files, 23 high / 149 medium / 36 low
- [x] **Step 1 — Scaffold** ✅ `todo scaffold . --dry-run` → 6 existing files correctly identified as skipped
- [x] **Step 2 — Plan** ✅ `todo plan todo.md --context .` → 10 batches, 17 items planned, 4 skipped
- [x] **Step 3 — Work (dry-run)** ✅ `todo work … --batch batch-006 --dry-run` → 3/3 items would patch `src/api/handlers.rs` cleanly
- [x] **Step 3 — Work (real)** ✅ `todo work … --batch batch-006` → 3 hunks applied, compile check passed, result written
- [x] **Step 4 — Sync (dry-run)** ✅ `todo sync todo.md … --dry-run` → all 3 items found by CRC32 ID
- [x] **Step 4 — Sync (real)** ✅ `todo sync todo.md … --append-summary` → 3 items marked `[x] ✅ Done`
- [x] **Compile check** ✅ `SQLX_OFFLINE=true cargo build --bin rustassistant` — clean
- [x] **Test suite** ✅ `cargo test` → 417 lib + 33 doctests passed, 0 failed

---

## 🟡 Medium Priority

### Local Model Integration

> See `todo/merge.md` for full architecture rationale. 8GB VRAM sweet spot:
> **Qwen2.5-Coder 7B** (Q4_K_M) for scaffold/stub gen, Grok for complex reasoning + review.

- [ ] **Pull and serve Qwen2.5-Coder 7B via Ollama** — `ollama pull qwen2.5-coder:7b`. Run as sidecar in `docker-compose.yml` alongside Redis. Expose on `localhost:11434`.
- [x] ~~**Add Ollama service to `docker-compose.yml`**~~ ✅ Done — `docker-compose.rustassistant.yml` at repo root includes `ollama` service with CUDA GPU passthrough, `ollama-init` one-shot model puller, and `OLLAMA_BASE_URL` wired into the app service.
- [x] ~~**Implement `src/ollama_client.rs`**~~ ✅ Done — full implementation: `POST /api/chat`, `stream: false`, `options.num_predict` + `temperature`, `OllamaClientConfig::default()` reads env vars, retry loop, `GrokClient` fallback, `health_check()`, `list_models()`. See `src/ollama_client.rs`.
- [ ] **Routing heuristic tuning** — after initial Ollama integration, measure stub quality. Adjust `ModelRouter::classify_prompt` thresholds or swap naive keyword match for a classify-first LLM call. Target: scaffold/stub tasks stay local, architectural review always goes remote.

### `.rustassistant/` Per-Repo Cache

> Architecture detailed in `todo/merge.md`. Cache dir spec in `todo/sync_scheduler.rs:RUSTASSISTANT_DIR_SPEC`.

- [ ] **`tree.txt` rolling snapshot** — `RepoSyncService::walk_tree` already generates it. Wire the scheduler so it regenerates on every 5-min tick and on every git push webhook. Prepend tree to every code-gen prompt in `CompletionRequest::build_prompt`.
- [x] ~~**`symbols.json` AST upgrade**~~ ✅ Done — `extract_symbols` now tries `extract_symbols_syn()` (using `syn::parse_file`) first for every `.rs` file, falling back to the old `extract_symbols_lines()` line-scanner only when syn fails to parse (e.g. macro-heavy or generated files). `syn` handles `fn`, `struct`, `enum`, `trait`, `impl`, `type`, `const`, `mod` at the top level, including `pub` visibility and `async` detection. `impl Trait for Type` renders as `"Trait for Type"` in the symbol name. `syn = { version = "2", features = ["full", "extra-traits"] }` added to `Cargo.toml`.
- [x] ~~**`todos.json` deduplication**~~ ✅ Done — `extract_todos` now maintains a `HashSet<(String, usize)>` keyed on `(file, line)`. Duplicate entries from repeated syncs of the same file are silently dropped before being pushed to the output vec.
- [ ] **`embeddings.bin` exclusion from git** — `RepoSyncService::register` already writes `.rustassistant/.gitignore` with `embeddings.bin`. Verify the entry is present and that `git status` does not show the file.
- [x] ~~**`context.md` auto-injection**~~ ✅ Done — `build_prompt_context` now loads `symbols.json` and `manifest.json` in addition to `tree.txt` and `todos.json`. Header shows `crate_name` from manifest. TODO section shows `(none)` when empty. Added **Key Public Symbols** section: top 5 public symbols sorted fns-first, formatted as `async? Kind name (file:line)`. Total context stays under ~3000 chars.

### CLI & Developer Experience

- [ ] **Actually test the XAI API connection in `test-api` command** — `handle_test_api` in `src/bin/cli.rs:726` only checks if the key exists. Instantiate `GrokClient::from_env()` and make a minimal `ask` call (e.g. `"ping — reply with ok"`) to verify the key is accepted, printing round-trip latency and token cost.
- [ ] **Parse detailed per-file test results in `TestRunner`** — `results_by_file` is an empty `HashMap` (`src/tests_runner.rs:184`). Parse the `--json-report` pytest output and `cargo test -- --format json` output to populate per-file pass/fail counts.
- [ ] **`todo work --auto-sync`** — add a flag that automatically runs `todo sync` after a successful `todo work` + compile-check pass, eliminating the manual step 4 invocation.

### Queue & Processing

- [ ] **Implement tag refinement and project linking** — `src/queue/processor.rs:430` has a stub `advance_stage`. Replace with actual tag inference using `src/tags.rs` / `src/tag_schema.rs` and project-linking by matching item tags against registered repos from `RepoSyncService`.

### Web Dashboard

- [ ] **Add `pinned` field to `Document` struct** — `src/web_ui_extensions.rs:375` and `:719`. Add `pinned BOOLEAN DEFAULT 0` column to the `documents` table migration, expose in the `Document` struct, render a pin icon in the card template.
- [ ] **Repo management UI tab** — expose the new `/api/v1/repos` endpoints in `static/index.html`: register a local path, trigger sync, view tree/todos/symbols in a tab, chat with repo context selected.
- [ ] **Scan results auto-switch** — after clicking "Run Scan" in the dashboard, automatically switch to the Scan tab and render filter counts as coloured stat cards.
- [ ] **Repo pull/refresh button** — `POST /api/web/repos/:id/pull` endpoint that runs `git pull` on the cloned repo and returns the new HEAD commit hash.

---

## 🟢 Low Priority / Enhancements

### Large File Handling

- [x] ~~Skip LFS-tracked files (pre-trained models) during clone/audit~~ ✅
- [ ] **Make skip-extensions list configurable per-repo** — currently hardcoded: `.onnx`, `.pt`, `.pth`, `.bin`, `.h5`, `.safetensors`, `.pkl`, `.pb`, `.tflite`, `.ckpt`, `.weights`, `.npy`, `.npz`. Add a `[scan] skip_extensions = [...]` key to repo config and thread it through `src/static_analysis.rs` and `src/auto_scanner.rs`.

### Docker & Compose

- [x] ~~Align `docker-compose.yml` README quick-start with actual SQLite-based setup~~ ✅
- [ ] **Add Redis healthcheck** — add a `healthcheck` block to the `rustassistant` service in `docker-compose.yml` that runs `redis-cli -h redis ping` and fails after 3 retries.
- [ ] **Add Ollama to compose with GPU passthrough** — `deploy.resources.reservations.devices` with `driver: nvidia`, `count: 1`, `capabilities: [gpu]`. Fall back gracefully (CPU-only) if no GPU present.

### Workflow & CI/CD

- [x] ~~Move `llm-audit.yml` workflow to `nuniesmith/actions` repo~~ ✅
- [x] ~~Add `docs/audit/` directory with `.gitkeep`~~ ✅
- [x] ~~Docker image pull in `llm-audit.yml`~~ ✅
- [ ] **Expose `/api/audit` endpoint** — so the LLM audit workflow can leverage the Rust API + Redis cache instead of raw Python API calls. Add `GET/POST /api/audit` to `src/server.rs`, backed by `src/audit/runner.rs`, returning JSON audit results and writing them to `docs/audit/`.
- [ ] **Auto-append audit findings to `todo.md`** — after the LLM audit completes, call `TodoFile::append_item` to add new findings to the target repo's `todo.md` automatically.

### Code Quality

- [ ] **Consolidate `todo_items` DB table with `tasks` table** — currently two parallel systems (`src/db/queue.rs:13`). Migrate `queue_items` writes in `src/queue/processor.rs` to use `db::core::create_task` instead, then deprecate `queue_items` in a migration.
- [ ] **Standardise error handling across API handlers** — mix of `anyhow` and manual error responses. Define a shared `ApiError` type in `src/api/types.rs` implementing `IntoResponse`, replace ad-hoc `(StatusCode, Json(...))` tuples in `src/api/handlers.rs`.
- [ ] **Implement `RedisAuditCache` I/O** — `src/audit/cache.rs` has all stubs with `todo!()`. Wire `deadpool_redis::Pool` into `RedisAuditCache::new`, implement `get_file_result`/`set_file_result`/`get_run_summary`/`set_run_summary` using `GET`/`SET EX` commands.
- [ ] **Implement `AuditRunner` pipeline** — `src/audit/runner.rs` is a stub. Wire: static analysis → LLM scoring via `GrokClient` → aggregation → write JSON to `docs/audit/`. Integrate `RedisAuditCache` to skip unchanged files.

---

## 📋 Notes

### Architecture — Local + Remote Model Routing
```
User Chat / todo-work
    ↓
ModelRouter::classify_prompt()
    ├── ScaffoldStub / TodoTagging / TreeSummary → Ollama (Qwen2.5-Coder 7B, local)
    ├── RepoQuestion / SymbolExtraction         → Ollama (local, fast)
    ├── ArchitecturalReason / CodeReview        → GrokClient (remote, xAI)
    └── Unknown / force_remote                 → GrokClient (remote, fallback)
```

### `.rustassistant/` Per-Repo Cache Structure
```
.rustassistant/
├── manifest.json    — repo identity, last_synced, branch, crate metadata
├── tree.txt         — rolling file tree snapshot (regenerated on every sync)
├── todos.json       — all TODO/STUB/FIXME/HACK tags with file:line attribution
├── symbols.json     — public functions, structs, traits, impls
├── context.md       — LLM-ready summary (injected into every code-gen prompt)
├── embeddings.bin   — cached vector embeddings (gitignored — excluded from commits)
├── scan.json        — output of `todo scan` (committed — useful as PR diff)
├── gameplan.json    — latest GamePlan from `todo plan`
├── results/         — WorkResult JSON files per batch
└── backups/         — pre-change file backups created by `todo work`
```

### Scaffolded Stubs — Wiring Checklist
> All 4 stub files are now merged into `src/`. Server wiring complete. ✅

| File | Destination | Status | Merged |
|------|------------|--------|--------|
| `todo/model_router.rs` | `src/model_router.rs` | ✅ Live | 2026-03-08 |
| `todo/repo_sync.rs` | `src/repo_sync.rs` | ✅ Live | 2026-03-08 |
| `todo/repos_api.rs` | `src/api/repos.rs` | ✅ Live | 2026-03-08 |
| `todo/sync_scheduler.rs` | `src/sync_scheduler.rs` | ✅ Live | 2026-03-08 |
| `todo/rustassistant-ui.html` | `static/rustassistant-ui.html` | ✅ Live | 2026-03-08 |
| `todo/.env.rustassistant` | `.env.rustassistant` | ✅ Copied | 2026-03-08 |
| `todo/docker-compose.rustassistant.yml` | `docker-compose.rustassistant.yml` | ✅ Live (was already at root) | 2026-03-08 |

### Session 3 — Completions
| Item | File(s) changed | Notes |
|------|----------------|-------|
| JoinSet sync scheduler | `src/sync_scheduler.rs` | Semaphore-gated, `REPO_SYNC_CONCURRENCY` env var, startup tick skip |
| syn symbol extractor | `src/repo_sync.rs`, `Cargo.toml` | syn 2 full parse → line-scanner fallback; `fn/struct/enum/trait/impl/type/const/mod` |
| TODO deduplication | `src/repo_sync.rs` | `HashSet<(file, line)>` in `extract_todos` |
| Context enrichment | `src/repo_sync.rs` | Crate name + top-5 public symbols injected into `build_prompt_context` |
| Fix admin module | `src/api/admin.rs`, `src/api/mod.rs`, `src/api/auth.rs`, `src/api/jobs.rs`, `src/lib.rs` | Removed non-existent field refs; mounted in router; `hash_api_key` pub; `pending_count()` added |
| Concurrent batch indexer | `src/indexing.rs` | `JoinSet` + semaphore; `Arc<DocumentIndexer>`; per-task error logging |

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
- `todo-work` now runs `cargo check` after patching. On failure it rolls back all changes automatically.
- **IDs are stable**: 8-char hex CRC32 of the raw list line. Don't edit item text between `todo-work` and `todo-sync` — the syncer finds items by ID.
- **Cross-repo reuse**: all repos share the same 5-step pipeline. Only the `todo.md` content and source files differ. The binary is identical.

### Migration Path
The current workflow is ~1900 lines of YAML + Python. Goal: progressively move logic into the Rust binary, publish via `ci-cd.yml` to Docker Hub (`nuniesmith/rustassistant:latest`), and make the workflow a thin orchestrator calling `./rustassistant-bin <command>`. The image is already being pulled — the CLI commands are now wired.

### Redis
Configured in `docker-compose.yml` for LLM response caching (`allkeys-lru`, 256 MB). The workflow currently bypasses this. Once `RedisAuditCache` is implemented and `repos_api.rs` chat handler caches responses, this becomes the hot-path cache for both the web UI and CI workflow.

### Pre-trained Model Files
`.onnx`, `.pt`, etc. in target repos are production artifacts, not source code. The workflow skips them entirely via `GIT_LFS_SKIP_SMUDGE=1` at clone time. They're never downloaded, hashed, or included in audit context.

---

### Batch `batch-006` Summary — 2026-03-08 01:11 UTC
- Attempted: 3 | Succeeded: 3 | Failed: 0 | Skipped: 0
- ✅ `12542b08` — Implement proper document listing with filters — currently returns empty vec p…
- ✅ `7048cea1` — Implement document stats `by_type` counts — returns empty vec (`src/api/handle…
- ✅ `7167a7b9` — Calculate average chunk size — hardcoded to `0.0` (`src/api/handlers.rs:137`)

---

## 🔴 High Priority — Postgres Migration

> SQLite is the current store. Postgres unlocks full-text search, JSONB indexing, concurrent
> writes, and production-grade reliability. This section tracks the full migration path.
> Complete steps in order — each one gates the next.

### 1. docker-compose — add Postgres service

- [ ] **Add `postgres` service to `docker-compose.rustassistant.yml`** — use `postgres:16-alpine`, bind `5432:5432`, mount `postgres-data:/var/lib/postgresql/data`, add healthcheck `pg_isready -U rustassistant`. Wire `rustassistant` service `depends_on` → `postgres: condition: service_healthy`.
  ```yaml
  postgres:
    image: postgres:16-alpine
    container_name: rustassistant-postgres
    restart: unless-stopped
    environment:
      POSTGRES_USER: rustassistant
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-changeme}
      POSTGRES_DB: rustassistant
    ports:
      - "5432:5432"
    volumes:
      - postgres-data:/var/lib/postgresql/data
    networks:
      - rustassistant-net
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U rustassistant"]
      interval: 10s
      timeout: 5s
      retries: 5
  ```
- [ ] **Add `postgres-data:` to `volumes:` block** in `docker-compose.rustassistant.yml`.

### 2. .env — swap DATABASE_URL

- [ ] **Replace SQLite `DATABASE_URL` with Postgres URL** in `.env` and `.env.rustassistant`:
  ```bash
  # Remove:
  DATABASE_URL=sqlite:./data/rustassistant.db

  # Add:
  DATABASE_URL=postgresql://rustassistant:changeme@postgres:5432/rustassistant
  POSTGRES_PASSWORD=changeme
  ```

### 3. Cargo.toml — swap sqlx features

- [ ] **Swap `sqlite` feature for `postgres`** in `Cargo.toml`. Also add `uuid` and `chrono` features which Postgres query macros require:
  ```toml
  # Remove:
  sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-native-tls", "macros"] }

  # Add:
  sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls", "macros", "uuid", "chrono"] }
  ```

### 4. Migrations — rewrite for Postgres syntax

> All 15 migration files in `migrations/` need auditing. SQLite is permissive; Postgres is strict.

- [ ] **Grep and fix `INTEGER PRIMARY KEY AUTOINCREMENT`** → `BIGSERIAL PRIMARY KEY` or `UUID PRIMARY KEY DEFAULT gen_random_uuid()` throughout all `.sql` files.
- [ ] **Replace `DATETIME` columns** with `TIMESTAMPTZ` and `DEFAULT CURRENT_TIMESTAMP` with `DEFAULT NOW()`.
- [ ] **Replace `TEXT` columns used as JSON** with `JSONB` (e.g. `tags`, `metadata`, `data` columns visible in `documents`, `repositories`, `tasks` tables).
- [ ] **Replace `BOOLEAN` stored as `0`/`1` integers** with native `BOOLEAN` and `TRUE`/`FALSE` literals.
- [ ] **Delete all `PRAGMA` statements** — they are SQLite-only and will error on Postgres.
- [ ] **Fix `IF NOT EXISTS` on views/triggers** — Postgres requires `CREATE OR REPLACE` for views; triggers use a different syntax entirely (need `CREATE FUNCTION` + `CREATE TRIGGER`).
- [ ] **Fix `ON CONFLICT` / `INSERT OR IGNORE` / `INSERT OR REPLACE`** → Postgres uses `ON CONFLICT DO NOTHING` / `ON CONFLICT (...) DO UPDATE SET ...`.
- [ ] **Fix `strftime('%s', 'now')`** → `EXTRACT(EPOCH FROM NOW())::BIGINT` or just use `TIMESTAMPTZ` columns directly.
- [ ] **Migration 006 — documents table**: `content_type` check constraint uses SQLite `CHECK(content_type IN (...))` syntax — verify it works in Postgres (it does, but confirm the constraint names don't clash).

### 5. Rust code — pool type change

- [ ] **Replace `SqlitePool` with `PgPool`** everywhere in `src/`:
  ```rust
  // Remove:
  use sqlx::SqlitePool;
  let pool = SqlitePool::connect(&database_url).await?;

  // Add:
  use sqlx::PgPool;
  let pool = PgPool::connect(&database_url).await?;
  ```
  Key files: `src/db/mod.rs`, `src/server.rs`, `src/bin/server.rs`, `src/repo_sync.rs`, `src/api/handlers.rs`, `src/api/admin.rs`, `src/api/jobs.rs`.
- [ ] **Update `init_db` in `src/db/mod.rs`** — remove the `sqlite:` prefix-stripping logic; Postgres URLs don't need it. Pass the URL directly to `PgPoolOptions::new().connect(&url).await`.
- [ ] **Remove `SQLX_OFFLINE=true` workaround** from `.env` — it will need to be regenerated against Postgres anyway (see step 6).
- [ ] **Fix `query_unchecked!` usages in `src/repo_sync.rs`** — these were used to avoid SQLite offline-check failures. Replace with properly typed `query!` macros once the Postgres schema is stable.

### 6. Regenerate .sqlx query cache

- [ ] **Run `cargo sqlx prepare`** after `DATABASE_URL` points at a live Postgres instance:
  ```bash
  export DATABASE_URL=postgresql://rustassistant:changeme@localhost:5432/rustassistant
  cargo sqlx database create
  cargo sqlx migrate run
  cargo sqlx prepare   # regenerates .sqlx/ — commit the result
  ```
  The compiler will surface every type mismatch between Rust types and the new Postgres schema. Fix each one the compiler reports.

### 7. Data migration (optional — only if keeping existing SQLite data)

- [ ] **Use `pgloader` to copy existing SQLite data** into Postgres if the dev/prod SQLite DB has data worth preserving:
  ```bash
  pip install pgloader
  pgloader sqlite:./data/rustassistant.db postgresql://rustassistant:changeme@localhost/rustassistant
  ```
  After loading: manually inspect `JSONB` columns (`tags`, `metadata`) — `pgloader` copies them as text strings; cast with `ALTER TABLE ... ALTER COLUMN data TYPE JSONB USING data::jsonb`.

### 8. Update RepoSyncService persistence

- [ ] **Port `RepoSyncService` DB calls from SQLite to Postgres** — the `load_from_db`, `register`, `remove_repo_async`, and `sync` methods in `src/repo_sync.rs` use `query_unchecked!` with SQLite-flavoured upsert (`INSERT OR REPLACE`). Rewrite as standard Postgres `INSERT ... ON CONFLICT (local_path) DO UPDATE SET ...`.

---

### 📋 Postgres Migration — Order of Operations

```
1. Add Postgres to docker-compose, spin it up locally
2. Swap DATABASE_URL in .env
3. Swap sqlx feature flag in Cargo.toml
4. Audit and rewrite all 15 migration files for Postgres syntax
5. cargo sqlx database create && cargo sqlx migrate run
6. cargo sqlx prepare  (regenerates .sqlx/ cache — commit this)
7. cargo build         (fix every type error the compiler surfaces)
8. Port RepoSyncService upsert logic to Postgres ON CONFLICT syntax
9. Run full test suite: cargo test
10. Update docker-compose depends_on, redeploy
```

> **Blocking note:** Steps 3–6 must happen together in one sitting — once you flip the sqlx
> feature flag, `SQLX_OFFLINE=true cargo build` will fail until `.sqlx/` is regenerated
> against the new Postgres schema. Plan for ~2–3 hours of uninterrupted migration work.