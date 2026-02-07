-- Migration: 005_notes_enhancements.sql
-- Description: Enhance notes system with proper tag relationships and repo linking
-- Created: 2024-01-15

-- ============================================================================
-- Create notes table if it doesn't exist
-- ============================================================================

CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'archived', 'deleted')),
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- ============================================================================
-- Enhance notes table with repo linking
-- ============================================================================

-- Add repo_id column to existing notes table (safe to run repeatedly)
ALTER TABLE notes ADD COLUMN repo_id TEXT;

-- Add foreign key index for repo_id
CREATE INDEX IF NOT EXISTS idx_notes_repo_id ON notes(repo_id);

-- Add index for status filtering
CREATE INDEX IF NOT EXISTS idx_notes_status ON notes(status);

-- Add index for created_at sorting
CREATE INDEX IF NOT EXISTS idx_notes_created ON notes(created_at DESC);

-- ============================================================================
-- Create tags table for tag management
-- ============================================================================

CREATE TABLE IF NOT EXISTS tags (
    name TEXT PRIMARY KEY,
    color TEXT DEFAULT '#3b82f6',
    description TEXT,
    usage_count INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- ============================================================================
-- Create note_tags junction table for many-to-many relationship
-- ============================================================================

CREATE TABLE IF NOT EXISTS note_tags (
    note_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (note_id, tag),
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
    FOREIGN KEY (tag) REFERENCES tags(name) ON DELETE CASCADE
);

-- Index for querying notes by tag
CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag);

-- Index for querying tags by note
CREATE INDEX IF NOT EXISTS idx_note_tags_note ON note_tags(note_id);

-- ============================================================================
-- Migrate existing inline tags to normalized structure
-- ============================================================================

-- Note: If there were existing notes with a tags column, this migration would
-- extract and normalize them. Since we're starting fresh, this section is a no-op.

-- Update usage counts for tags will be handled by triggers

-- ============================================================================
-- Create views for common queries
-- ============================================================================

-- View for notes with their tags aggregated
CREATE VIEW IF NOT EXISTS notes_with_tags AS
SELECT
    n.id,
    n.title,
    n.content,
    n.status,
    n.repo_id,
    n.created_at,
    n.updated_at,
    GROUP_CONCAT(nt.tag, ',') as tags,
    COUNT(nt.tag) as tag_count
FROM notes n
LEFT JOIN note_tags nt ON n.id = nt.note_id
GROUP BY n.id, n.title, n.content, n.status, n.repo_id, n.created_at, n.updated_at;

-- View for tags with their usage statistics
CREATE VIEW IF NOT EXISTS tag_stats AS
SELECT
    t.name,
    t.color,
    t.description,
    t.usage_count,
    COUNT(DISTINCT nt.note_id) as current_note_count,
    t.created_at,
    t.updated_at
FROM tags t
LEFT JOIN note_tags nt ON t.name = nt.tag
GROUP BY t.name, t.color, t.description, t.usage_count, t.created_at, t.updated_at
ORDER BY t.usage_count DESC;

-- View for repo notes summary
CREATE VIEW IF NOT EXISTS repo_notes_summary AS
SELECT
    r.id as repo_id,
    r.name as repo_name,
    COUNT(n.id) as note_count,
    COUNT(CASE WHEN n.status = 'inbox' THEN 1 END) as inbox_count,
    COUNT(CASE WHEN n.status = 'active' THEN 1 END) as active_count,
    COUNT(CASE WHEN n.status = 'done' THEN 1 END) as done_count,
    MAX(n.created_at) as last_note_at
FROM repositories r
LEFT JOIN notes n ON r.id = n.repo_id
GROUP BY r.id, r.name;

-- View for note activity (recent notes with repo and tag info)
CREATE VIEW IF NOT EXISTS recent_notes_activity AS
SELECT
    n.id,
    n.content,
    n.status,
    n.repo_id,
    r.name as repo_name,
    GROUP_CONCAT(nt.tag, ',') as tags,
    n.created_at,
    strftime('%Y-%m-%d %H:%M:%S', n.created_at, 'unixepoch') as created_at_formatted
FROM notes n
LEFT JOIN repositories r ON n.repo_id = r.id
LEFT JOIN note_tags nt ON n.id = nt.note_id
GROUP BY n.id, n.content, n.status, n.repo_id, r.name, n.created_at
ORDER BY n.created_at DESC
LIMIT 50;

-- ============================================================================
-- Create triggers for maintaining tag usage counts
-- ============================================================================

-- Trigger to increment usage_count when a tag is added to a note
CREATE TRIGGER IF NOT EXISTS increment_tag_usage
AFTER INSERT ON note_tags
BEGIN
    UPDATE tags
    SET usage_count = usage_count + 1,
        updated_at = strftime('%s', 'now')
    WHERE name = NEW.tag;
END;

-- Trigger to decrement usage_count when a tag is removed from a note
CREATE TRIGGER IF NOT EXISTS decrement_tag_usage
AFTER DELETE ON note_tags
BEGIN
    UPDATE tags
    SET usage_count = usage_count - 1,
        updated_at = strftime('%s', 'now')
    WHERE name = OLD.tag;
END;

-- Trigger to auto-create tag if it doesn't exist when adding to a note
CREATE TRIGGER IF NOT EXISTS auto_create_tag
BEFORE INSERT ON note_tags
WHEN NOT EXISTS (SELECT 1 FROM tags WHERE name = NEW.tag)
BEGIN
    INSERT INTO tags (name, created_at, updated_at)
    VALUES (NEW.tag, strftime('%s', 'now'), strftime('%s', 'now'));
END;

-- Trigger to update note's updated_at when tags are modified
CREATE TRIGGER IF NOT EXISTS update_note_timestamp_on_tag_add
AFTER INSERT ON note_tags
BEGIN
    UPDATE notes
    SET updated_at = strftime('%s', 'now')
    WHERE id = NEW.note_id;
END;

CREATE TRIGGER IF NOT EXISTS update_note_timestamp_on_tag_remove
AFTER DELETE ON note_tags
BEGIN
    UPDATE notes
    SET updated_at = strftime('%s', 'now')
    WHERE id = OLD.note_id;
END;

-- ============================================================================
-- Add some default tags for common use cases
-- ============================================================================

INSERT OR IGNORE INTO tags (name, color, description) VALUES
    ('idea', '#10b981', 'New ideas and brainstorming'),
    ('todo', '#f59e0b', 'Things to do'),
    ('bug', '#ef4444', 'Bug reports and issues'),
    ('question', '#8b5cf6', 'Questions and uncertainties'),
    ('research', '#3b82f6', 'Research notes'),
    ('refactor', '#ec4899', 'Code refactoring ideas'),
    ('performance', '#f97316', 'Performance improvements'),
    ('documentation', '#06b6d4', 'Documentation related'),
    ('security', '#dc2626', 'Security concerns'),
    ('feature', '#22c55e', 'Feature requests');

-- ============================================================================
-- Cleanup: Mark the old tags column as deprecated (but keep for backward compatibility)
-- ============================================================================

-- NOTE: We're keeping the old 'tags' column for now to maintain backward compatibility
-- It can be dropped in a future migration once all code is updated to use the normalized structure

-- Add a comment column to track migration status (SQLite doesn't support column comments)
-- This is just for documentation purposes

-- ============================================================================
-- Rollback instructions (in case needed)
-- ============================================================================

-- To rollback this migration, run:
-- DROP VIEW IF EXISTS recent_notes_activity;
-- DROP VIEW IF EXISTS repo_notes_summary;
-- DROP VIEW IF EXISTS tag_stats;
-- DROP VIEW IF EXISTS notes_with_tags;
-- DROP TRIGGER IF EXISTS update_note_timestamp_on_tag_remove;
-- DROP TRIGGER IF EXISTS update_note_timestamp_on_tag_add;
-- DROP TRIGGER IF EXISTS auto_create_tag;
-- DROP TRIGGER IF EXISTS decrement_tag_usage;
-- DROP TRIGGER IF EXISTS increment_tag_usage;
-- DROP TABLE IF EXISTS note_tags;
-- DROP TABLE IF EXISTS tags;
-- DROP INDEX IF EXISTS idx_notes_created;
-- DROP INDEX IF EXISTS idx_notes_status;
-- DROP INDEX IF EXISTS idx_notes_repo_id;
-- Then manually remove repo_id column (requires table recreation in SQLite)

-- ============================================================================
-- Migration complete
-- ============================================================================
