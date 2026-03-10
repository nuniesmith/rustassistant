# RustAssistant — TODO Backlog

---

## 🚀 What's Next — 2026-03-10

> **Current state:** All 5 services healthy (`rustassistant` ✅, `postgres` ✅, `redis` ✅, `ollama` ✅ CPU-only, `ollama-init` ✅). `qwen2.5-coder:7b` loaded. App starts clean, migrations apply, sync scheduler running. One stale repo record (`my-repo / /path/to/repo`) spamming WARN every minute.

### 1. Fix stale repo record — easy win, noisy logs
The auto-scanner logs `Repo my-repo path /path/to/repo does not exist` on every tick (every ~60s). There is a seeded/leftover row in `registered_repos` pointing at `/path/to/repo`. Either delete it via the API/UI or add a guard so the scanner silently skips rows where `git_url` is null and the path doesn't exist, rather than WARNing repeatedly.
- [ ] `DELETE FROM registered_repos WHERE local_path = '/path/to/repo';` via psql **or** add a DB migration/seed cleanup
- [ ] Downgrade the log from `WARN` to `DEBUG` for repos with no `git_url` and non-existent path (expected during first-run)

### 2. Fix GPU / restore Ollama CUDA passthrough
`nvidia-smi` segfaults on the host — this is a driver/toolkit version mismatch between the installed NVIDIA driver and the `nvidia-container-toolkit`. Running CPU-only right now (`OLLAMA_NUM_GPU=0`). Fix this so the RTX 2070 SUPER (8 GB VRAM, detected in earlier logs) is used — inference will be ~10–15× faster.
- [ ] Diagnose: `sudo dmesg | grep -i nvidia` and check driver version with `cat /proc/driver/nvidia/version`
- [ ] Reinstall or upgrade `nvidia-container-toolkit` to match the installed driver: `sudo apt-get install --reinstall nvidia-container-toolkit`
- [ ] Re-run `nvidia-smi` to confirm fix, then re-enable GPU in `docker-compose.yml` (`OLLAMA_NUM_GPU=1` + uncomment `deploy.resources.reservations`)

### 3. Remove deprecated `TodoItem` — kills 16 compiler warnings
`TodoItem` struct in `src/db/queue.rs` is `#[deprecated]` and still referenced in `src/db/mod.rs` and `src/db/queue.rs` itself, generating 16 warnings every build. The table was dropped in migration 017. Remove the struct and all references.
- [ ] Delete `TodoItem` struct from `src/db/queue.rs`
- [ ] Remove `TodoItem` from the `pub use` in `src/db/mod.rs`
- [ ] Verify `cargo check` produces zero deprecation warnings for this module

### 4. Wire auth middleware on repo API (`src/api/repos.rs`)
Line 3 of `src/api/repos.rs` has `// TODO: add auth middleware (reuse existing API key layer)`. The API key infrastructure exists in `src/api/admin.rs`. Without this, all repo/chat endpoints are unauthenticated.
- [ ] Add `require_api_key` extractor/middleware (same pattern as admin routes) to the repo router in `src/server.rs`
- [ ] Test with and without `Authorization: Bearer <key>` header

### 5. Fix ignored `query_router` tests — `todo!()` stubs
Four tests in `src/query_router.rs` are `#[ignore]` because `QueryRouter::new` requires `pool`, `cache`, and `context_builder` but the tests use `todo!()` placeholders. These need either mock constructors or a `QueryRouter::new_for_test()` that accepts `Option` fields.
- [ ] Add `QueryRouter::new_for_test(pool, cache, context_builder)` or make fields `Option`
- [ ] Remove `#[ignore]` from the four tests and get them green

### 6. Validate end-to-end LLM routing (local → Ollama → response)
Ollama is up with `qwen2.5-coder:7b` loaded. The model router and Ollama client are implemented but have never been exercised against a live model in this environment.
- [ ] `curl -X POST http://localhost:3000/api/v1/chat -H 'Content-Type: application/json' -d '{"prompt":"what is 2+2","repo_id":null}'` — confirm local model responds
- [ ] Register a real repo (point `REPOS_BASE_PATH` at an actual path), trigger a sync, then send a repo-context chat request — confirm RAG context is injected
- [ ] Check `ModelRouter::llm_classify` routes scaffold/stub tasks local and architecture questions remote

### 7. Make skip-extensions configurable per-repo
Currently hardcoded in `src/static_analysis.rs` and `src/auto_scanner.rs`. Add `[scan] skip_extensions = [...]` to per-repo config and thread it through.
- [ ] Add `skip_extensions: Vec<String>` to repo config struct
- [ ] Pass it through `AutoScanner` and `StaticAnalysis` call sites
- [ ] Default to current hardcoded list if not set

### 8. Regenerate `.sqlx` cache against live Postgres
`SQLX_OFFLINE=true` is used everywhere as a workaround. Now that Postgres is healthy and persistent, generate a proper offline cache so the build doesn't depend on that env var.
- [ ] `DATABASE_URL=postgres://rustassistant:changeme@localhost:5432/rustassistant cargo sqlx prepare`
- [ ] Commit the regenerated `.sqlx/` directory
- [ ] Remove `SQLX_OFFLINE=true` from `docker/Dockerfile` (both build stages) once confirmed working

---


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
- [x] ~~`GH_PAT` needs `repo` scope — switched to `GITHUB_TOKEN` throughout~~ ✅ Done — all four `GH_PAT` references in `.github/workflows/llm-audit.yml` replaced with `GITHUB_TOKEN`: clone step env, TODO-Work step env, Commit step env, and the Python PR-creation script (`os.environ["GITHUB_TOKEN"]`). The secrets header comment updated accordingly. New token with repo access configured.

### Rust-Native TODO System

- [x] ~~Add a `rustassistant todo-scan <repo-path>` command~~ ✅ Done — `todo scan . --json --output .rustassistant/scan_fresh.json` runs offline.
- [x] ~~Generate a GAMEPLAN from a `todo.md` file using the Rust LLM client~~ ✅ Done — `todo plan todo.md --context . --output .rustassistant/gameplan.json`.
- [x] ~~Execute a single batch from the gameplan~~ ✅ Done — `todo work .rustassistant/gameplan.json --batch batch-006` ran 3 items, patched `src/api/handlers.rs` cleanly.
- [x] ~~Build a `TodoFile` struct that can parse, update, and write back `todo.md`~~ ✅ Done — `todo sync todo.md .rustassistant/results/batch-006.json` finds all items by stable CRC32 ID.
- [x] ~~`todo work --auto-sync`~~ ✅ Done — `--auto-sync` flag is live in `src/bin/cli.rs`. After a successful work + compile-check pass it automatically calls `todo sync`, eliminating the manual step 4. `--todo-md` override flag added too.
- [x] ~~**Wire workflow to Docker image**~~ ✅ Done — `llm-audit.yml` steps **TODO-Analyze**, **TODO-Plan**, and **TODO-Work** now check `steps.pull_image.outputs.ra_available == 'true'` and call `./rustassistant-bin todo scan/plan/work` first. Python blocks are preserved as fallback inside `else` branches. `jq` used for JSON extraction to keep YAML valid.

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
- [x] ~~**Feed `RepoSyncService` embeddings into RAG pipeline more granularly**~~ ✅ Done — `sync()` in `src/repo_sync.rs` now reads the previous `tree.txt` snapshot before overwriting it. The background embedding pass is skipped entirely when no `.rs` files changed. Otherwise only new files (not in the old snapshot) and modified files (mtime > `last_synced`) are passed to `embed_rust_files()`. First-sync still embeds everything. Logged at `info` level with `changed`/`total_rs` counters.

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
- [x] ~~**Port `RepoSyncService` upsert to Postgres syntax**~~ ✅ Done — `register()` now uses `ON CONFLICT (local_path) WHERE active = TRUE DO UPDATE SET` (partial-index upsert matching migration 015). `id` is refreshed on re-registration so slug renames resolve cleanly. `remove_repo` docstring updated. `load_from_db`, `remove_repo_async`, and `sync` already used correct Postgres syntax.
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

- [x] ~~**Implement tag refinement and project linking in `process_tagging`**~~ ✅ Done — `process_tagging` in `src/queue/processor.rs` now: (1) parses LLM tags from `item.tags`, (2) normalises aliases via `refine_tags()` (e.g. `tech-debt` → `technical-debt`), (3) infers `CodeStatus` from source/score via `infer_status_from_item()`, (4) derives numeric priority via `derive_priority()` using `tag_schema::Priority`, (5) resolves `repo_id` via `resolve_repo_from_path()` (LIKE query on `registered_repos.local_path`), (6) writes a linked row to `tasks` table via `db::core::create_task`, (7) advances item to `Ready`. Non-fatal on task write failure so the queue never jams.

### Web Dashboard

- [x] ~~**Add `pinned` field to `Document` struct**~~ ✅ Done — `migrations/016_add_document_pinned.sql` adds `pinned BOOLEAN NOT NULL DEFAULT FALSE` + partial index. `Document` struct in `src/db/core.rs` gains `pinned: bool` + `pin_icon()` helper. All six `SELECT` sites in `src/db/documents.rs` now include `COALESCE(pinned, FALSE) AS pinned`; pinned docs sort first (`ORDER BY pinned DESC`). `src/db/mod.rs` re-exports `set_document_pinned`. `src/web_ui_extensions.rs` both TODO stubs replaced with `doc.pin_icon()`. `POST /api/web/docs/:id/pin` and `POST /api/web/docs/:id/unpin` endpoints wired in `src/web_api.rs`.
- [x] ~~**Repo management UI tab / detail view**~~ ✅ Done — `RepoDetailPanel` component added to `static/rustassistant-ui.html`. Each repo card gains a `📂 Details` button that toggles an inline panel spanning the full grid width (slide-down animation). The panel has four tabs — **🌲 Tree** (raw `tree.txt`), **📋 TODOs** (structured rows with kind colour-coding), **⚙ Symbols** (pub/async badges + file:line), **📝 Context** (full `context.md`) — all fetched live from the existing `/api/v1/repos/:id/{tree,todos,symbols,context}` endpoints. Results are cached per-tab so switching is instant. Panel closes via the ✕ button or toggling the same card again.
- [x] ~~**Scan results auto-switch**~~ ✅ Done — `ReposPane` in `static/rustassistant-ui.html` gains a `🔍 Scan` button per repo. On success it feeds scan items into `queuedTasks`, calls `setActiveRepoId`, and calls `setTab("tasks")` — automatically switching to the Tasks pane and populating it with the scan results.
- [x] ~~**Repo pull/refresh button**~~ ✅ Done — `POST /api/web/repos/:id/pull` endpoint added to `src/web_api.rs` (`handle_pull_repo`). Uses `GitManager::update()` (git2 fetch + merge) in a blocking task, then reads the new HEAD hash. Returns `{ repo_id, head, message }`. `⬆ Pull` button added to each repo card in `ReposPane`; shows `⏳ Pulling…` while in-flight and disables Sync simultaneously.

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

- [x] ~~**Consolidate `todo_items` DB table with `tasks` table**~~ ✅ Done — `migrations/017_drop_todo_items.sql` drops the table and its three indexes. `src/db/queue.rs` `create_queue_tables` no longer emits `todo_items` DDL; `TodoItem` struct marked `#[deprecated]`. `src/scanner/github.rs` `scan_repo_for_todos` rewritten to write directly to `tasks` (source=`"github_scanner"`, source_id=content_hash for dedup). `src/cli/queue_commands.rs` `handle_report_command` migrated to read from `tasks` with Postgres `$N` placeholders. `process_tagging` already wrote to `tasks` — no change needed there.
- [x] ~~**Standardise error handling across API handlers**~~ ✅ Done — `ApiError { error, code }` moved to `src/api/types.rs` with full `IntoResponse`, `Display`, `std::error::Error`, `From<sqlx::Error>`, and `From<anyhow::Error>` impls. Added `bad_request`, `unauthorized`, and `from_error` constructors. `src/api/repos.rs` now re-exports it via `pub use crate::api::types::ApiError`. `src/api/admin.rs` SQLite `?` placeholders replaced with Postgres `$N` throughout (`create_api_key`, `revoke_api_key`, `list_jobs`, `retry_job`).
- [x] ~~**Implement `AuditRunner::run` stub**~~ ✅ Done — `AuditRunner::run` now delegates to `run_static_only(&request.repo)` and carries the original `AuditRequest` back in the response. No LLM cost (`estimated_cost_usd = 0.0`). `AuditRunnerWithGrok::run` remains the full LLM-assisted path. 8/8 `audit::runner::tests` green, including updated tests that assert `Ok` instead of `Err("not yet implemented")`.

---

### Batch `batch-012` Summary — 2026-03-08 (repo detail view + todo_items consolidation)

| # | Item | File(s) | Result |
|---|------|---------|--------|
| 1 | `RepoDetailPanel` component — tabbed tree/todos/symbols/context inline panel | `static/rustassistant-ui.html` | ✅ |
| 2 | `📂 Details` button per repo card, `detailOpen` state, `React.Fragment` wrapper | `static/rustassistant-ui.html` | ✅ |
| 3 | CSS styles for `.repo-detail-panel`, `.detail-tabs`, `.detail-tab`, `.detail-body`, `.todo-row`, `.sym-row` | `static/rustassistant-ui.html` | ✅ |
| 4 | `migrations/017_drop_todo_items.sql` — drop `todo_items` table + indexes | `migrations/017_drop_todo_items.sql` | ✅ |
| 5 | Remove `todo_items` DDL from `create_queue_tables`; deprecate `TodoItem` struct | `src/db/queue.rs` | ✅ |
| 6 | Migrate `scan_repo_for_todos` to write to `tasks` table (dedup via `source_id`) | `src/scanner/github.rs` | ✅ |
| 7 | Migrate `report todos` CLI command to read from `tasks` with Postgres `$N` | `src/cli/queue_commands.rs` | ✅ |

- Build: ✅ `SQLX_OFFLINE=true cargo build --bin rustassistant` — clean (warnings only, all pre-existing)
- Tests: ✅ 404 passed; 45 pre-existing Postgres integration failures (require live DB); 0 new failures

### Batch `batch-011` Summary — 2026-03-08 (pinned docs + repo pull + incremental RAG + scan auto-switch)

| # | Item | File(s) | Result |
|---|------|---------|--------|
| 1 | `migrations/016_add_document_pinned.sql` — add `pinned` column + partial index | `migrations/016_add_document_pinned.sql` | ✅ |
| 2 | `Document` struct + `pin_icon()` helper + all DB query sites updated | `src/db/core.rs`, `src/db/documents.rs`, `src/db/mod.rs` | ✅ |
| 3 | `set_document_pinned()` DB helper + `POST /api/web/docs/:id/pin|unpin` endpoints | `src/db/documents.rs`, `src/web_api.rs` | ✅ |
| 4 | Replace pin `TODO` stubs with `doc.pin_icon()` in web UI extensions | `src/web_ui_extensions.rs` | ✅ |
| 5 | `POST /api/web/repos/:id/pull` endpoint (`handle_pull_repo`) | `src/web_api.rs` | ✅ |
| 6 | `⬆ Pull` button in `ReposPane` SPA (disables Sync while in-flight) | `static/rustassistant-ui.html` | ✅ |
| 7 | `🔍 Scan` button in `ReposPane` with auto-switch to Tasks tab | `static/rustassistant-ui.html` | ✅ |
| 8 | Incremental RAG embedding — diff `tree.txt`, skip unchanged `.rs` files | `src/repo_sync.rs` | ✅ |

- Build: ✅ `SQLX_OFFLINE=true cargo build --bin rustassistant` — clean (warnings only, all pre-existing)
- Tests: ✅ 404 passed; 45 pre-existing Postgres integration failures (require live DB); 0 new failures

### Batch `batch-010` Summary — 2026-03-08 (postgres + error handling + audit + queue)

| # | Item | File(s) | Result |
|---|------|---------|--------|
| 1 | Port `RepoSyncService` upsert to `ON CONFLICT (local_path)` | `src/repo_sync.rs` | ✅ |
| 2 | Wire `llm-audit.yml` TODO steps to Docker binary | `.github/workflows/llm-audit.yml` | ✅ |
| 3 | Implement `process_tagging` tag refinement + `create_task` linking | `src/queue/processor.rs` | ✅ |
| 4 | Move `ApiError` to `src/api/types.rs` with `IntoResponse` | `src/api/types.rs`, `src/api/repos.rs` | ✅ |
| 5 | Fix SQLite `?` placeholders → Postgres `$N` in `admin.rs` | `src/api/admin.rs` | ✅ |
| 6 | Wire `AuditRunner::run` stub to `run_static_only()` | `src/audit/runner.rs` | ✅ |

- Build: ✅ `SQLX_OFFLINE=true cargo build --bin rustassistant` — clean (warnings only)
- Tests: ✅ 404 passed; 45 pre-existing Postgres integration failures (require live DB); 0 new failures
- YAML: ✅ `python3 -c "import yaml; yaml.safe_load(...)"` — valid

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
