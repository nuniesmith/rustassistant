Alright Jordan, I've done a deep dive through all the project knowledge. Here's a comprehensive review covering where things stand and concrete plans for each area you mentioned.

---

## Overall Assessment

Rustassistant is in solid shape. You've got a working 2-container deployment (rustassistant + redis), functional repo scanning with auto-scan, a web UI with dashboard/repos/queue pages, and Grok integration with 70%+ cache hit rates. The architecture decisions — unified server binary, SQLite + Redis caching, Askama templates with HTMX — are all sound for a solo-dev Pi-hosted tool.

That said, the project has accumulated a lot of documentation from iterative sessions (79+ markdown files, many now archived) and the codebase is at a natural inflection point where it needs to evolve from "scan repos and show a queue" into a broader personal knowledge/workflow system. Let me address each of your goals.

---

## 1. Moving to External Repo-Based Structure (No Volume Mounts)

Right now your docker-compose mounts host paths directly for repos, templates, config, and data:

```yaml
volumes:
  - ./data:/app/data
  - ./templates:/app/templates:ro
  - ./config:/app/config:ro
  - /home/jordan/github/fks:/repos:ro
```

This works but is fragile — you need host paths to exist, it couples container layout to host filesystem, and it won't work cleanly in CI/CD or on a fresh deploy.

**What to change:**

- **Templates and static assets** should be `COPY`'d into the Docker image at build time. They're part of your application, not runtime configuration. Your Dockerfile should include:
  ```dockerfile
  COPY templates/ /app/templates/
  COPY static/ /app/static/
  COPY config/ /app/config/
  ```

- **Data (SQLite)** should use a Docker named volume, not a bind mount. This is more portable and Docker manages the lifecycle:
  ```yaml
  volumes:
    rustassistant-data:
  services:
    rustassistant:
      volumes:
        - rustassistant-data:/app/data
  ```

- **Repos to scan** — this is the interesting one. Instead of mounting host directories, the container should **clone repos via git at runtime**. You already track repos in the database with their git remotes. The scanner should clone (or shallow-clone) repos into a container-internal path like `/app/repos/`, update them with `git pull` on each scan interval, and work from there. This eliminates the volume mount entirely. You'd store a GitHub token as an environment variable / Docker secret for private repos.

- **Redis data** already uses a named volume, which is correct.

The net result: your `docker-compose.yml` has zero bind mounts. Everything is either baked into the image (templates, static, config) or managed as named volumes (data) or cloned at runtime (repos). This makes deployment to any host trivial — just `docker compose up -d` with the right env vars.

---

## 2. Web UI — Scanning Progress & Observability

The current UI shows repo status and queue items, but there's no real-time feedback on what the scanner is doing. You're right that you shouldn't need to tail logs to know what's happening.

**What to add:**

- **Scan status per repo** — Add columns to the `repositories` table: `scan_status` (idle/scanning/error), `scan_progress` (text like "Analyzing file 12/47"), `last_scan_duration_ms`, `last_scan_files_found`, `last_scan_issues_found`, `last_error`. The scanner updates these as it works. The web UI polls every few seconds (HTMX `hx-trigger="every 5s"`) and shows a progress indicator on each repo card.

- **Progress bar component** — For active scans, show a simple progress bar. Since you know the total file count from the directory tree, you can calculate percentage as files are processed. A simple CSS bar with HTMX partial updates works well here without needing WebSockets.

- **Activity log / event feed** — Add a lightweight `scan_events` table that logs timestamped entries like "Started scanning fks", "Found 3 TODOs in main.rs", "Scan complete: 47 files, 12 issues, 2.3s". Show the last N events on the dashboard as a live feed. This replaces reading docker logs.

- **Health/stats endpoint enhancement** — Expand `/health` to return scanner status, cache hit rates, last scan times, queue depths, and LLM cost so far today. Display these on the dashboard with auto-refresh.

- **Toast notifications** — You already have toast support for clipboard copy. Extend it to show scan-start and scan-complete events via SSE (Server-Sent Events) or polling. SSE is trivial with Axum and avoids the complexity of WebSockets.

---

## 3. Web UI — Ideas/Thoughts Capture with Tags

You already have a notes system via CLI (`rustassistant note add "..." --tags x,y`), but it's not exposed in the web UI beyond a basic list. This needs to become a first-class feature.

**What to build:**

- **Quick capture widget** — A persistent input box on the dashboard (or accessible from any page via a floating button) where you type a thought and hit enter. Support inline tag syntax like `#rust #idea` or a tag selector dropdown. This should feel as frictionless as a text message to yourself.

- **Notes page with filtering** — Full CRUD for notes in the web UI. Filter by tag, status (inbox/active/processed/archived), date range, and free-text search. HTMX makes this trivial with `hx-get="/notes?tag=rust&status=inbox"`.

- **Tag management** — A dedicated tags page showing all tags with counts, the ability to rename/merge/delete tags, and color coding. Tags are your primary organizational primitive, so they need to be easy to manage.

- **Tag suggestions** — When adding a note, suggest tags based on content (simple keyword matching initially, LLM-powered later). Also suggest existing tags as you type to prevent tag proliferation (e.g., "rust" vs "Rust" vs "rust-lang").

- **Linking notes to repos** — Allow tagging a note with a repo name so thoughts about a specific project are associated with it. When viewing a repo, you'd see related notes alongside scan results.

---

## 4. Research Docs & Knowledge Base / RAG Integration

This is the most ambitious and highest-value addition. You want to save research docs, build up a searchable knowledge base, and use it for context stuffing.

**Architecture approach:**

- **Docs table in SQLite** — `documents(id, title, content, source_url, doc_type, tags, created_at, updated_at)`. Doc types could be: research, reference, tutorial, architecture-decision, etc.

- **Web UI for docs** — Upload/paste markdown documents, tag them, search them. A simple editor (even just a textarea with markdown preview) for writing and editing. Import from URL (fetch and store).

- **Vector embeddings via fastembed-rs** — You're already using mxbai-embed-large-v1 locally. When a doc is saved, chunk it and store embeddings in LanceDB. This powers semantic search ("find docs about async trait patterns") and context stuffing ("give me everything relevant to this task").

- **Context stuffing pipeline** — When the LLM is about to analyze a task or answer a question, query LanceDB for relevant doc chunks and stuff them into the prompt. With Grok 4.1's 2M token window, you can be aggressive here.

- **Separate docs repo (future)** — For now, store everything in rustassistant's database. When the collection grows, you can create a `nuniesmith/docs` repo that contains markdown files organized by topic. Rustassistant would scan this repo like any other, but instead of looking for TODOs, it indexes the content into the RAG system. The docs repo becomes a git-managed knowledge base that's portable and version-controlled.

- **Cross-repo utility** — The docs repo concept is powerful because the same Rust deployment patterns, Axum middleware patterns, Docker best practices, etc. apply across all your projects. Index once, use everywhere.

---

## 5. Editable Scan Interval in Web UI

Currently scan interval is set via environment variable or direct SQLite update. The web UI should expose this directly.

**Implementation:**

- The `repositories` table already has `scan_interval_minutes`. Add an edit form/modal on the repo card — either inline editing (click the interval, type a new number, hit save) or a "Configure" button that opens a settings panel.

- API endpoint: `POST /api/repos/{id}/settings` accepting `{ "scan_interval_minutes": 30 }`.

- Web handler: `POST /repos/{id}/settings` that updates the DB and redirects back to the repos page with a success toast.

- The auto-scanner loop should re-read intervals from the DB on each cycle rather than caching them, so changes take effect immediately.

---

## Priority Ordering

If I were to suggest an implementation order:

1. **Scan interval editing** — Quick win, 30 minutes of work, immediately useful
2. **Docker volume mount elimination** — Architectural, do this before adding more features so you're building on the right foundation
3. **Scanning progress indicators** — Adds essential observability
4. **Ideas/thoughts capture in web UI** — Unlocks daily use of the web UI beyond just viewing scan results
5. **Research docs & RAG integration** — The big value-add, but build on top of the above

