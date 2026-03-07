# RustAssistant — TODO Backlog

> Maintained manually + updated by the LLM Audit workflow (`todo-plan`, `todo-work`, `todo-review`).
> Items marked with ✅ have been completed. Items with ⚠️ are partial. Items with ❌ are blocked.

---

## 🔴 High Priority

### API & Data Layer
- [ ] Fix admin module — `pub mod admin` is commented out due to accessing non-existent `ApiState` fields (`src/api/mod.rs`)
- [ ] Implement proper document listing with filters — currently returns empty vec placeholder (`src/api/handlers.rs:345`)
- [ ] Implement document stats `by_type` counts — returns empty vec (`src/api/handlers.rs:132`)
- [ ] Calculate average chunk size — hardcoded to `0.0` (`src/api/handlers.rs:137`)

### Search & RAG
- [ ] Integrate RAG context search with LanceDB vector search — currently returns empty results (`src/research/worker.rs:275`)

### Indexing
- [ ] Implement concurrent batch indexing with semaphore — currently sequential only (`src/indexing.rs:395`)

---

## 🟡 Medium Priority

### CLI & Developer Experience
- [ ] Actually test the XAI API connection in `test-api` command — currently only checks if the key exists (`src/bin/cli.rs:726`)
- [ ] Parse detailed per-file test results in `TestRunner` — `results_by_file` is an empty `HashMap` (`src/tests_runner.rs:184`)

### Queue & Processing
- [ ] Implement tag refinement and project linking in queue processor tagging stage (`src/queue/processor.rs:430`)

### Web UI
- [ ] Add `pinned` field to `Document` struct for docs list and detail views (`src/web_ui_extensions.rs:375`, `src/web_ui_extensions.rs:719`)

---

## 🟢 Low Priority / Enhancements

### Docker & Compose
- [x] ~~Align `docker-compose.yml` README quick-start with actual SQLite-based setup~~ ✅ Fixed — README now references SQLite, port 3000, `docker compose`
- [ ] Add `docker-compose.yml` healthcheck for Redis connectivity from rustassistant container

### Workflow & CI/CD
- [x] ~~Move `llm-audit.yml` workflow to `nuniesmith/actions` repo where it belongs~~ ✅ Removed from rustassistant; see actions repo outline below
- [x] ~~Add a `docs/audit/` directory with `.gitkeep` so workflow report commits don't need to `mkdir`~~ ✅ Created
- [ ] Expose an `/api/audit` endpoint so the LLM audit workflow can leverage the Rust API + Redis cache instead of raw Python API calls

### Code Quality
- [ ] Consolidate `todo_items` DB table usage with the `tasks` table — currently two parallel systems (`src/db/queue.rs:13`)
- [ ] Standardise error handling across API handlers (mix of `anyhow` and manual error responses)

---

## 📋 Notes

- The `auto_scanner` module already integrates `StaticAnalyzer` + `TodoScanner` + `PromptRouter` for smart file triage before LLM calls. The audit workflow should eventually call into this rather than reimplementing analysis in Python.
- Redis is configured in `docker-compose.yml` for LLM response caching (`allkeys-lru`, 256 MB). The workflow currently bypasses this entirely.
- The `.rustassistant/` directory is **tracked in git** (removed from `.gitignore`). It stores both the CLI analysis cache (`cache/`) and the LLM audit workflow's cross-run state (`cache.json`, `batches/`). See `.rustassistant/README.md` for details.