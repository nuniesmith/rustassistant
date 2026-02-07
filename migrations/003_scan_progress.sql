-- Migration: 003_scan_progress.sql
-- Description: Add scan progress tracking and observability fields to repositories table
-- Created: 2024-01-15

-- ============================================================================
-- Add scan progress and status tracking columns to repositories table
-- ============================================================================

-- Scan status tracking
ALTER TABLE repositories ADD COLUMN scan_status TEXT DEFAULT 'idle' CHECK(scan_status IN ('idle', 'scanning', 'error'));

-- Current scan progress information
ALTER TABLE repositories ADD COLUMN scan_progress TEXT DEFAULT NULL;
ALTER TABLE repositories ADD COLUMN scan_current_file TEXT DEFAULT NULL;
ALTER TABLE repositories ADD COLUMN scan_files_total INTEGER DEFAULT 0;
ALTER TABLE repositories ADD COLUMN scan_files_processed INTEGER DEFAULT 0;

-- Last scan metrics
ALTER TABLE repositories ADD COLUMN last_scan_duration_ms INTEGER DEFAULT NULL;
ALTER TABLE repositories ADD COLUMN last_scan_files_found INTEGER DEFAULT 0;
ALTER TABLE repositories ADD COLUMN last_scan_issues_found INTEGER DEFAULT 0;
ALTER TABLE repositories ADD COLUMN last_error TEXT DEFAULT NULL;

-- ============================================================================
-- Create indexes for performance
-- ============================================================================

-- Index for querying active scans
CREATE INDEX IF NOT EXISTS idx_repositories_scan_status
ON repositories(scan_status)
WHERE scan_status != 'idle';

-- Index for auto-scanner queries
CREATE INDEX IF NOT EXISTS idx_repositories_auto_scan
ON repositories(auto_scan, last_scanned_at)
WHERE auto_scan = 1;

-- ============================================================================
-- Create scan_events table for activity logging
-- ============================================================================

CREATE TABLE IF NOT EXISTS scan_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id TEXT NOT NULL,
    event_type TEXT NOT NULL CHECK(event_type IN ('scan_started', 'scan_completed', 'scan_error', 'todo_found', 'issue_found', 'git_update')),
    message TEXT NOT NULL,
    metadata TEXT, -- JSON blob for additional event data
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),

    FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE CASCADE
);

-- Index for recent events queries
CREATE INDEX IF NOT EXISTS idx_scan_events_created
ON scan_events(created_at DESC);

-- Index for filtering events by repo
CREATE INDEX IF NOT EXISTS idx_scan_events_repo
ON scan_events(repo_id, created_at DESC);

-- Index for filtering events by type
CREATE INDEX IF NOT EXISTS idx_scan_events_type
ON scan_events(event_type, created_at DESC);

-- ============================================================================
-- Backfill existing repositories with default values
-- ============================================================================

-- Ensure all existing repos have default scan status
UPDATE repositories
SET scan_status = 'idle'
WHERE scan_status IS NULL;

-- Set default scan metrics for repos that have been scanned
UPDATE repositories
SET last_scan_files_found = 0,
    last_scan_issues_found = 0
WHERE last_scanned_at IS NOT NULL
  AND (last_scan_files_found IS NULL OR last_scan_issues_found IS NULL);

-- ============================================================================
-- Add helpful views for monitoring
-- ============================================================================

-- View for active scans
CREATE VIEW IF NOT EXISTS active_scans AS
SELECT
    id,
    name,
    scan_status,
    scan_progress,
    scan_current_file,
    scan_files_processed,
    scan_files_total,
    CASE
        WHEN scan_files_total > 0 THEN
            CAST((scan_files_processed * 100.0 / scan_files_total) AS INTEGER)
        ELSE 0
    END as progress_percentage,
    strftime('%Y-%m-%d %H:%M:%S', last_scanned_at, 'unixepoch') as scan_started_at
FROM repositories
WHERE scan_status = 'scanning';

-- View for recent scan activity
CREATE VIEW IF NOT EXISTS recent_scan_activity AS
SELECT
    r.id,
    r.name,
    e.event_type,
    e.message,
    e.metadata,
    strftime('%Y-%m-%d %H:%M:%S', e.created_at, 'unixepoch') as event_time
FROM scan_events e
JOIN repositories r ON e.repo_id = r.id
ORDER BY e.created_at DESC
LIMIT 50;

-- View for repository health summary
CREATE VIEW IF NOT EXISTS repository_health AS
SELECT
    id,
    name,
    scan_status,
    auto_scan,
    scan_interval_mins as scan_interval_minutes,
    last_scan_duration_ms,
    last_scan_files_found,
    last_scan_issues_found,
    CASE
        WHEN last_error IS NOT NULL THEN 'unhealthy'
        WHEN scan_status = 'error' THEN 'unhealthy'
        WHEN last_scanned_at IS NULL THEN 'never_scanned'
        WHEN (strftime('%s', 'now') - last_scanned_at) > (scan_interval_minutes * 60 * 2) THEN 'stale'
        ELSE 'healthy'
    END as health_status,
    strftime('%Y-%m-%d %H:%M:%S', last_scanned_at, 'unixepoch') as last_scan
FROM repositories;

-- ============================================================================
-- Migration complete
-- ============================================================================
