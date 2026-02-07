-- ============================================================================
-- Migration: Add scan progress, activity feed, documents, and ideas
-- ============================================================================
-- Run with: sqlite3 data/rustassistant.db < migrations/003_scan_docs_ideas.sql
-- ============================================================================

-- Add scan progress tracking columns to repositories
ALTER TABLE repositories ADD COLUMN scan_status TEXT NOT NULL DEFAULT 'idle';
-- scan_status: 'idle', 'cloning', 'scanning', 'analyzing', 'error'

ALTER TABLE repositories ADD COLUMN scan_progress TEXT;
-- JSON: {"current_file": "src/main.rs", "files_scanned": 12, "total_files": 47, "phase": "scanning"}

ALTER TABLE repositories ADD COLUMN scan_files_total INTEGER NOT NULL DEFAULT 0;
ALTER TABLE repositories ADD COLUMN scan_files_done INTEGER NOT NULL DEFAULT 0;
ALTER TABLE repositories ADD COLUMN scan_issues_found INTEGER NOT NULL DEFAULT 0;
ALTER TABLE repositories ADD COLUMN scan_duration_ms INTEGER;
ALTER TABLE repositories ADD COLUMN last_scan_error TEXT;

-- ============================================================================
-- Scan Events - Activity feed for scanner operations
-- ============================================================================
CREATE TABLE IF NOT EXISTS scan_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id TEXT,
    event_type TEXT NOT NULL,
    -- event_type: 'scan_start', 'scan_complete', 'scan_error', 'file_analyzed',
    --             'todo_found', 'issue_found', 'repo_cloned', 'repo_updated',
    --             'cache_hit', 'llm_call', 'system'
    message TEXT NOT NULL,
    details TEXT,          -- JSON for extra context
    level TEXT NOT NULL DEFAULT 'info',
    -- level: 'debug', 'info', 'warn', 'error'
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_scan_events_created ON scan_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_scan_events_repo ON scan_events(repo_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_scan_events_type ON scan_events(event_type);
CREATE INDEX IF NOT EXISTS idx_scan_events_level ON scan_events(level);

-- ============================================================================
-- Documents - Knowledge base for research, docs, reference material
-- ============================================================================
CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    summary TEXT,               -- LLM-generated summary
    source_url TEXT,            -- Where it came from (if imported)
    doc_type TEXT NOT NULL DEFAULT 'reference',
    -- doc_type: 'research', 'reference', 'tutorial', 'architecture',
    --           'decision', 'snippet', 'template', 'runbook'
    tags TEXT,                  -- Comma-separated tags
    project TEXT,               -- Optional project association
    repo_id TEXT,               -- Optional repo association
    format TEXT NOT NULL DEFAULT 'markdown',
    -- format: 'markdown', 'plaintext', 'html', 'code'
    word_count INTEGER NOT NULL DEFAULT 0,
    is_indexed INTEGER NOT NULL DEFAULT 0,  -- Whether embeddings exist in LanceDB
    embedding_id TEXT,          -- Reference to LanceDB vector
    pinned INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active',
    -- status: 'active', 'archived', 'draft'
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_documents_type ON documents(doc_type);
CREATE INDEX IF NOT EXISTS idx_documents_status ON documents(status);
CREATE INDEX IF NOT EXISTS idx_documents_project ON documents(project);
CREATE INDEX IF NOT EXISTS idx_documents_tags ON documents(tags);
CREATE INDEX IF NOT EXISTS idx_documents_created ON documents(created_at DESC);

-- Full-text search for documents
CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
    title,
    content,
    tags,
    content='documents',
    content_rowid='rowid'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS documents_ai AFTER INSERT ON documents BEGIN
    INSERT INTO documents_fts(rowid, title, content, tags)
    VALUES (new.rowid, new.title, new.content, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS documents_ad AFTER DELETE ON documents BEGIN
    INSERT INTO documents_fts(documents_fts, rowid, title, content, tags)
    VALUES ('delete', old.rowid, old.title, old.content, old.tags);
END;

CREATE TRIGGER IF NOT EXISTS documents_au AFTER UPDATE ON documents BEGIN
    INSERT INTO documents_fts(documents_fts, rowid, title, content, tags)
    VALUES ('delete', old.rowid, old.title, old.content, old.tags);
    INSERT INTO documents_fts(rowid, title, content, tags)
    VALUES (new.rowid, new.title, new.content, new.tags);
END;

-- ============================================================================
-- Ideas - Quick thought capture with tagging
-- ============================================================================
CREATE TABLE IF NOT EXISTS ideas (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    tags TEXT,                  -- Comma-separated tags
    project TEXT,               -- Optional project link
    repo_id TEXT,               -- Optional repo link
    priority INTEGER NOT NULL DEFAULT 3,
    -- 1=urgent, 2=high, 3=normal, 4=low, 5=someday
    status TEXT NOT NULL DEFAULT 'inbox',
    -- status: 'inbox', 'active', 'in_progress', 'done', 'archived'
    category TEXT,
    -- category: 'feature', 'bug', 'improvement', 'research', 'question', 'random'
    linked_doc_id TEXT,         -- Optional link to a document
    linked_task_id TEXT,        -- Optional link to a task
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE SET NULL,
    FOREIGN KEY (linked_doc_id) REFERENCES documents(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_ideas_status ON ideas(status);
CREATE INDEX IF NOT EXISTS idx_ideas_priority ON ideas(priority);
CREATE INDEX IF NOT EXISTS idx_ideas_category ON ideas(category);
CREATE INDEX IF NOT EXISTS idx_ideas_tags ON ideas(tags);
CREATE INDEX IF NOT EXISTS idx_ideas_project ON ideas(project);
CREATE INDEX IF NOT EXISTS idx_ideas_created ON ideas(created_at DESC);

-- ============================================================================
-- Tags registry - centralized tag management
-- ============================================================================
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    color TEXT,                 -- Hex color for UI display
    usage_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);
CREATE INDEX IF NOT EXISTS idx_tags_usage ON tags(usage_count DESC);
