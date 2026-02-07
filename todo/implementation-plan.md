# RustAssistant Implementation Plan

This document breaks down the TODO items into concrete, actionable tasks with specific implementation details.

---

## Priority 1: Scan Interval Editing in Web UI ⚡

**Status:** ✅ COMPLETED  
**Estimated Effort:** 2-3 hours  
**Dependencies:** None

### Tasks

#### 1.1 Add Edit UI Component
- [x] Add inline edit form to repo card in templates
- [x] Use HTMX for seamless updates
- [x] Add validation (min 5 minutes, max 1440 minutes)
- [x] Show success/error toast notifications

#### 1.2 Create API Endpoint
- [x] Add `POST /repos/{id}/settings` handler in `web_ui.rs`
- [x] Accept form data: `scan_interval_minutes` and `auto_scan_enabled`
- [x] Update database record via `update_repo_settings()` function
- [x] Return HTMX-compatible response with toast triggers

#### 1.3 Update Auto-Scanner
- [x] Verified auto-scanner already re-reads intervals from DB each cycle
- [x] No changes needed

**Files Modified:**
- ✅ `src/web_ui.rs` - Added `update_repo_settings_handler`, `UpdateRepoSettingsRequest` struct, and `update_repo_settings()` database function
- ✅ `src/templates/pages/repos.html` - Added inline settings form with HTMX, toast notifications, and animations

**Testing:**
- [ ] Deploy and test: Change interval via UI and verify next scan uses new value
- [ ] Test validation with invalid values (< 5 or > 1440)
- [ ] Verify toast notifications display correctly
- [ ] Test auto-scan toggle checkbox

---

## Priority 2: Docker Volume Mount Elimination 🐳

**Status:** ✅ COMPLETED  
**Estimated Effort:** 6-8 hours  
**Dependencies:** None

### Tasks

#### 2.1 Bake Assets into Docker Image
- [x] Static assets already copied - DONE
- [x] Verify templates are accessible (Askama compiles them into binary)
- [x] Copy default config into image at build time
- [x] Document config override via environment variables

**Files Modified:**
- ✅ `docker/Dockerfile` - Verified correct, config copied at build time
- ✅ `docker-compose.yml` - Removed config bind mount
- ✅ `todo/DOCKER_MIGRATION_GUIDE.md` - Created comprehensive migration guide

#### 2.2 Replace Data Bind Mount with Named Volume
- [x] Update `docker-compose.yml` to use named volume for data
- [x] Add initialization logic to create DB if not exists (already in place)
- [x] Document backup/restore procedures for named volumes

**Changes Applied:**
```yaml
# Removed: - ./data:/app/data
# Removed: - ./config:/app/config:ro
# Added to volumes section:
volumes:
  rustassistant_data:
    driver: local
    name: rustassistant_data

# Updated service:
services:
  rustassistant:
    volumes:
      - rustassistant_data:/app/data
      - repos_data:/app/repos
```

#### 2.3 Implement Git Clone at Runtime
- [x] Create `RepoManager` struct in new `src/repo_manager.rs`
- [x] Add `clone_or_update()` method that:
  - Checks if repo exists in `/app/repos/{repo_name}`
  - If not exists: `git clone --depth 1 {git_url} /app/repos/{repo_name}`
  - If exists: `cd /app/repos/{repo_name} && git pull`
  - Uses GITHUB_TOKEN env var for auth
- [x] Update `auto_scanner.rs` to call clone_or_update before scanning
- [x] Add `git_url` requirement when adding repos via UI (already present)
- [x] Add migration to make `git_url` NOT NULL with default for existing repos

**Files Created:**
- ✅ `src/repo_manager.rs` - Git clone/update logic (358 lines)

**Files Modified:**
- ✅ `src/auto_scanner.rs` - Integrated RepoManager, replaced old clone logic
- ✅ `src/lib.rs` - Added repo_manager module export
- ✅ `src/web_ui.rs` - Already requires git_url in add repo form
- ✅ `migrations/004_require_git_url.sql` - New migration (134 lines)

#### 2.4 Update Documentation
- [x] Create comprehensive migration guide
- [x] Document volume backup/restore commands
- [x] Document environment variables needed
- [ ] Update README.md deployment section (pending)

**Files Created:**
- ✅ `todo/DOCKER_MIGRATION_GUIDE.md` - Complete migration guide (584 lines)

**Environment Variables Documented:**
- `GITHUB_TOKEN` - For cloning private repos
- `DATABASE_URL` - Already present
- `REPOS_DIR` - Already present (defaults to /app/repos)

---

## Priority 3: Scanning Progress Indicators 📊

**Status:** Not Started  
**Estimated Effort:** 8-10 hours  
**Dependencies:** None (but easier after Priority 2)

### Tasks

#### 3.1 Extend Repository Schema
- [x] Create migration `003_scan_progress.sql`
- [x] Add columns to repositories table:
  - `scan_status TEXT DEFAULT 'idle'` (idle/scanning/error)
  - `scan_progress TEXT` (e.g., "Processing file 12/47")
  - `scan_current_file TEXT` (current file being processed)
  - `scan_files_total INTEGER DEFAULT 0`
  - `scan_files_processed INTEGER DEFAULT 0`
  - `last_scan_duration_ms INTEGER`
  - `last_scan_files_found INTEGER`
  - `last_scan_issues_found INTEGER`
  - `last_error TEXT`

**Files Created:**
- ✅ `migrations/003_scan_progress.sql` - Complete with indexes, views, and scan_events table

**Files Modified:**
- ✅ `src/db/core.rs` - Updated Repository struct with all new fields marked with `#[sqlx(default)]`
- ✅ Added helper methods: `scan_status_display()`, `progress_percentage()`, `is_auto_scan_enabled()`

#### 3.2 Update Scanner to Report Progress
- [ ] Modify `auto_scanner.rs` scan loop to:
  - Set status='scanning' at start
  - Update scan_progress every N files (e.g., every 5)
  - Track file count and timing
  - Set status='idle' or 'error' at completion
  - Store metrics (duration, files, issues found)

**Files to Modify:**
- `src/auto_scanner.rs` - Add progress updates
- `src/db/core.rs` - Add update_scan_progress() method

#### 3.3 Create Progress UI Component
- [ ] Add progress bar component to repo card
- [ ] Show current file being processed
- [ ] Display percentage complete (files_processed / files_total)
- [ ] Use HTMX polling (`hx-trigger="every 3s"`) to update during scan
- [ ] Show last scan metrics (duration, files, issues)

**Files to Modify:**
- `src/templates/pages/repositories.html` - Add progress UI
- `src/templates/components/progress_bar.html` - New component
- `src/web_ui.rs` - Add `/api/repos/{id}/progress` endpoint

#### 3.4 Activity Log / Event Feed
- [ ] Create `scan_events` table:
  - `id INTEGER PRIMARY KEY`
  - `repo_id TEXT`
  - `event_type TEXT` (scan_started/scan_completed/error/todo_found)
  - `message TEXT`
  - `metadata TEXT` (JSON)
  - `created_at INTEGER`
- [ ] Add event logging to scanner
- [ ] Create dashboard widget showing last 20 events
- [ ] Add SSE endpoint `/events/stream` for real-time updates (optional)

**Files to Create:**
- `migrations/004_scan_events.sql`
- `src/events.rs` - Event logging module

**Files to Modify:**
- `src/auto_scanner.rs` - Log events
- `src/db/core.rs` - Add event queries
- `src/templates/pages/dashboard.html` - Add event feed widget

#### 3.5 Enhanced Health Endpoint
- [ ] Extend `/health` to return JSON with:
  - Scanner status (running/stopped)
  - Active scans count
  - Cache hit rate (last hour)
  - Queue depth
  - LLM cost today/this week/this month
  - Last scan times per repo
- [ ] Add dashboard stats panel that polls `/health` every 10s

**Files to Modify:**
- `src/server.rs` or `src/web_ui.rs` - Enhance health endpoint
- `src/templates/pages/dashboard.html` - Add stats panel

---

## Priority 4: Ideas/Thoughts Capture with Tags 💡

**Status:** Not Started  
**Estimated Effort:** 10-12 hours  
**Dependencies:** None

### Tasks

#### 4.1 Create Notes Schema
- [ ] Create migration `005_notes_system.sql`:
  - `notes` table:
    - `id TEXT PRIMARY KEY`
    - `content TEXT NOT NULL`
    - `status TEXT DEFAULT 'inbox'` (inbox/active/done/archived)
    - `repo_id TEXT` (optional, FK to repositories)
    - `created_at INTEGER`
    - `updated_at INTEGER`
  - `note_tags` table:
    - `note_id TEXT`
    - `tag TEXT`
    - `PRIMARY KEY (note_id, tag)`
  - `tags` table:
    - `name TEXT PRIMARY KEY`
    - `color TEXT` (hex color)
    - `description TEXT`
    - `usage_count INTEGER DEFAULT 0`
    - `created_at INTEGER`

**Files to Create:**
- `migrations/005_notes_system.sql`
- `src/notes.rs` - Notes CRUD operations

**Files to Modify:**
- `src/db/core.rs` - Add Note struct and queries
- `src/lib.rs` - Add notes module

#### 4.2 Quick Capture Widget
- [ ] Add floating action button (FAB) to base template
- [ ] Create modal/overlay for note capture
- [ ] Support inline tag syntax (`#rust #idea`)
- [ ] HTMX form submission with optimistic UI
- [ ] Show success toast and clear form

**Files to Create:**
- `src/templates/components/note_capture.html`

**Files to Modify:**
- `src/templates/layouts/base.html` - Add FAB
- `src/web_ui.rs` - Add `POST /api/notes` endpoint

#### 4.3 Notes Page with Filtering
- [ ] Create full notes CRUD page
- [ ] Filter by tag (multi-select)
- [ ] Filter by status (dropdown)
- [ ] Filter by repo (dropdown)
- [ ] Free-text search
- [ ] Sortable by date
- [ ] Inline editing
- [ ] Bulk actions (tag, archive, delete)

**Files to Create:**
- `src/templates/pages/notes.html`

**Files to Modify:**
- `src/web_ui.rs` - Add notes page handlers
- `src/notes.rs` - Add search/filter functions

#### 4.4 Tag Management
- [ ] Create tags page showing all tags with counts
- [ ] Tag rename/merge/delete operations
- [ ] Color picker for tag colors
- [ ] Tag suggestions based on existing tags (prevent duplicates)
- [ ] Auto-complete in note input

**Files to Create:**
- `src/templates/pages/tags.html`

**Files to Modify:**
- `src/web_ui.rs` - Add tag management endpoints
- `src/notes.rs` - Add tag operations

#### 4.5 Repo-Note Linking
- [ ] Add "Notes" tab to repo detail view
- [ ] Show notes linked to this repo
- [ ] Quick-add note from repo page (pre-fills repo_id)
- [ ] Badge count on repo card showing note count

**Files to Modify:**
- `src/templates/pages/repository_detail.html` - Add notes tab
- `src/templates/pages/repositories.html` - Add note count badge

---

## Priority 5: Research Docs & RAG Integration 📚

**Status:** Not Started  
**Estimated Effort:** 15-20 hours  
**Dependencies:** None (but benefits from Priority 4 for tagging)

### Tasks

#### 5.1 Documents Schema
- [ ] Create migration `006_documents.sql`:
  - `documents` table:
    - `id TEXT PRIMARY KEY`
    - `title TEXT NOT NULL`
    - `content TEXT NOT NULL`
    - `source_url TEXT`
    - `doc_type TEXT` (research/reference/tutorial/architecture/decision)
    - `summary TEXT` (LLM-generated)
    - `created_at INTEGER`
    - `updated_at INTEGER`
  - `document_tags` table (similar to note_tags)
  - `document_chunks` table:
    - `id TEXT PRIMARY KEY`
    - `document_id TEXT`
    - `chunk_index INTEGER`
    - `content TEXT`
    - `embedding BLOB` (fastembed vector)
    - `metadata TEXT` (JSON)

**Files to Create:**
- `migrations/006_documents.sql`
- `src/documents.rs` - Document CRUD and chunking
- `src/embeddings.rs` - Embedding generation via fastembed

#### 5.2 Embedding Pipeline
- [ ] Integrate `fastembed` crate
- [ ] Use `mxbai-embed-large-v1` model (already familiar from setup)
- [ ] Chunk documents (500-1000 tokens per chunk with overlap)
- [ ] Generate embeddings asynchronously
- [ ] Store in SQLite BLOB column (for now, LanceDB later if needed)

**Dependencies to Add:**
```toml
fastembed = "3.0"
hf-hub = "0.3"  # For model downloads
```

**Files to Create:**
- `src/embeddings.rs`

#### 5.3 Documents Web UI
- [ ] Create documents page (list/grid view)
- [ ] Upload markdown file
- [ ] Paste content directly
- [ ] Import from URL (fetch and store)
- [ ] Markdown editor with preview
- [ ] Tag documents
- [ ] Full-text and semantic search

**Files to Create:**
- `src/templates/pages/documents.html`
- `src/templates/pages/document_edit.html`

**Files to Modify:**
- `src/web_ui.rs` - Add document handlers
- `src/documents.rs` - Add search functions

#### 5.4 Semantic Search
- [ ] Add `/api/docs/search` endpoint
- [ ] Accept query string, generate embedding
- [ ] Cosine similarity search against document_chunks
- [ ] Return top N relevant chunks with source documents
- [ ] Highlight matching sections

**Files to Modify:**
- `src/embeddings.rs` - Add similarity search
- `src/documents.rs` - Add search endpoint

#### 5.5 Context Stuffing for LLM
- [ ] Create `ContextBuilder` that:
  - Takes a task/query
  - Retrieves relevant doc chunks (top 10-20)
  - Formats into context section for prompt
  - Tracks which docs were used
- [ ] Integrate into refactor_assistant and task analysis
- [ ] Add "Context Used" section to LLM responses

**Files to Create:**
- `src/context_stuffing.rs`

**Files to Modify:**
- `src/refactor_assistant.rs` - Use context stuffing
- `src/grok_client.rs` - Add context to prompts

#### 5.6 Docs Repo Scanner (Future)
- [ ] Create special repo type: "docs_repo"
- [ ] Scanner extracts markdown files
- [ ] Auto-generates document records
- [ ] Syncs on git push
- [ ] Allows git-managed knowledge base

**Files to Modify:**
- `src/auto_scanner.rs` - Add docs repo scanning mode
- `src/documents.rs` - Add sync from repo

---

## Database Migration Strategy

All migrations should follow this pattern:

```sql
-- Migration: XXX_feature_name.sql
-- Description: Brief description
-- Created: YYYY-MM-DD

-- Add new columns/tables
ALTER TABLE ... ;
CREATE TABLE ... ;

-- Add indexes for performance
CREATE INDEX ... ;

-- Backfill data if needed
UPDATE ... ;

-- Add foreign keys last
-- (SQLite requires recreation for FK additions)
```

**Migration Files to Create:**
1. `003_scan_progress.sql` - Scan status and metrics
2. `004_scan_events.sql` - Event logging
3. `005_notes_system.sql` - Notes and tags
4. `006_documents.sql` - Documents and embeddings

---

## Testing Strategy

For each priority:

### Unit Tests
- [ ] Database operations (CRUD)
- [ ] Git operations (clone, pull, status)
- [ ] Embedding generation
- [ ] Search algorithms

### Integration Tests
- [ ] Full scan cycle with progress updates
- [ ] Note creation with tags
- [ ] Document upload and chunking
- [ ] Semantic search end-to-end

### Manual Testing Checklist
- [ ] Deploy to Pi and verify no volume mount errors
- [ ] Add repo via web UI with git_url
- [ ] Watch scan progress in real-time
- [ ] Capture notes from dashboard
- [ ] Search documents semantically
- [ ] Verify cache hit rates remain high

---

## Performance Considerations

### Scan Progress Updates
- Batch DB updates (every 5 files, not every file)
- Use prepared statements
- Index on `scan_status` and `last_scan_check`

### Embeddings
- Generate async in background task
- Cache embeddings (don't regenerate unchanged chunks)
- Consider batch processing

### Web UI Polling
- Use 3-5 second intervals (not 1 second)
- Only poll when scans are active
- Use ETags/Last-Modified to avoid redundant transfers

---

## Deployment Checklist

Before deploying each priority:

- [ ] Run all migrations
- [ ] Update docker-compose.yml
- [ ] Set environment variables
- [ ] Test on development machine
- [ ] Backup database
- [ ] Deploy to Pi
- [ ] Verify functionality
- [ ] Monitor logs for errors
- [ ] Update documentation

---

## Timeline Estimate

| Priority | Effort | Target Completion |
|----------|--------|-------------------|
| 1. Scan Interval UI | 2-3h | Week 1 |
| 2. Docker Volumes | 6-8h | Week 1-2 |
| 3. Progress Indicators | 8-10h | Week 2-3 |
| 4. Notes System | 10-12h | Week 3-4 |
| 5. RAG/Docs | 15-20h | Week 4-6 |

**Total Estimated Effort:** 41-53 hours across 6 weeks

---

## Future Enhancements (Post-Priority 5)

- [ ] Mobile-responsive UI improvements
- [ ] Dark/light theme toggle
- [ ] Export notes/docs to markdown
- [ ] Keyboard shortcuts
- [ ] Batch repo operations
- [ ] LLM model selection in UI
- [ ] Cost budgets and alerts
- [ ] Scheduled reports (daily digest)
- [ ] WebSocket for real-time updates (replace polling)
- [ ] Multi-user support with auth
- [ ] API authentication with tokens
- [ ] Webhook integrations (Slack, Discord)

---

## Questions / Decisions Needed

1. **LanceDB vs SQLite for embeddings?**
   - Start with SQLite BLOB, migrate to LanceDB if performance issues
   - LanceDB better for >10K documents

2. **SSE vs Polling for real-time updates?**
   - Start with polling (simpler)
   - Move to SSE if user experience suffers

3. **Notes vs Tasks - how do they relate?**
   - Notes are informal thoughts/ideas
   - Tasks are actionable items with status tracking
   - A note can "promote" to a task

4. **Should docs repo be a separate Git repo or part of rustassistant repo?**
   - Separate repo allows use across multiple projects
   - Same approach as FKS (external repo, scanned by rustassistant)

---

## Success Metrics

After completing all priorities, we should have:

- ✅ Zero bind mounts in docker-compose (portable deployment)
- ✅ Real-time scan progress visible in web UI
- ✅ <3 second latency for note capture
- ✅ >90% cache hit rate maintained
- ✅ Semantic search returning relevant docs <500ms
- ✅ 70%+ reduction in "what's happening?" log tailing
- ✅ Active use of notes for daily thoughts/ideas
- ✅ Knowledge base with 10+ useful docs

---

## Getting Started

**Recommended next steps:**

1. ✅ Review this plan
2. ✅ Start with Priority 1 (quick win) - COMPLETED
3. ⏭️ Move to Priority 2 (architectural improvement)
4. Implement Priority 3-5 in order
5. Iterate based on real usage

**Progress:**
- ✅ Implementation plan created
- ✅ Migration 003 (scan progress schema) created with full event tracking
- ✅ Updated Repository struct in `db/core.rs` with new fields
- ✅ Priority 1 completed: Scan interval editing in web UI
  - Added API endpoint for settings updates
  - Created inline edit form with HTMX
  - Added toast notifications
  - Input validation (5-1440 minutes)
- ✅ Priority 2 completed: Docker volume mount elimination
  - Created `src/repo_manager.rs` (358 lines)
  - Integrated RepoManager into auto-scanner
  - Updated docker-compose.yml to use named volumes
  - Created migration 004 for git_url requirement
  - Created comprehensive Docker migration guide (584 lines)

**Next Steps:**
1. Run migrations 003 and 004 on database
2. Test Priority 1 and 2 implementations
3. Migrate from bind mounts to named volumes (follow DOCKER_MIGRATION_GUIDE.md)
4. Begin Priority 3: Scan progress indicators

Let's build! 🚀