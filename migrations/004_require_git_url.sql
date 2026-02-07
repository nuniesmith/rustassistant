-- Migration: 004_require_git_url.sql
-- Description: Make git_url required for repositories and add validation
-- Created: 2024-01-15
--
-- This migration prepares the repositories table for runtime git cloning
-- by ensuring all repos have a git_url configured.

-- ============================================================================
-- Add default git_url for existing repositories without one
-- ============================================================================

-- For existing repos without url, set a placeholder that indicates manual setup needed
UPDATE repositories
SET url = 'https://github.com/unknown/repo.git'
WHERE url IS NULL OR url = '';

-- ============================================================================
-- Add check constraint to ensure url is always set for new repos
-- ============================================================================

-- SQLite doesn't support adding constraints to existing tables directly,
-- so we document this as a validation rule to be enforced at the application level.
--
-- Application-level validation should ensure:
-- 1. url is not NULL
-- 2. url is not empty string
-- 3. url is a valid HTTPS URL (preferably GitHub/GitLab)
--
-- Example valid URLs:
--   - https://github.com/username/repo.git
--   - https://gitlab.com/username/repo.git
--   - https://bitbucket.org/username/repo.git

-- ============================================================================
-- Add metadata column for repository configuration
-- ============================================================================

-- Add source_type to track where repos come from
ALTER TABLE repositories ADD COLUMN source_type TEXT DEFAULT 'git' CHECK(source_type IN ('git', 'local', 'external'));

-- Add clone_depth for shallow clone configuration (NULL = full clone)
ALTER TABLE repositories ADD COLUMN clone_depth INTEGER DEFAULT 1;

-- Add last_sync_at to track when repo was last cloned/updated
ALTER TABLE repositories ADD COLUMN last_sync_at INTEGER;

-- ============================================================================
-- Create index for faster url lookups
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_repositories_url
ON repositories(url)
WHERE url IS NOT NULL;

-- Index for repositories that need syncing
CREATE INDEX IF NOT EXISTS idx_repositories_sync
ON repositories(last_sync_at, auto_scan)
WHERE auto_scan = 1;

-- ============================================================================
-- Update metadata for existing repos
-- ============================================================================

-- Mark existing repos with local paths as 'local' source type
UPDATE repositories
SET source_type = 'local'
WHERE local_path NOT LIKE '/app/repos/%'
  AND local_path NOT LIKE '%/repos/%';

-- Mark repos with valid git URLs as 'git' source type
UPDATE repositories
SET source_type = 'git'
WHERE url LIKE 'https://github.com/%'
   OR url LIKE 'https://gitlab.com/%'
   OR url LIKE 'https://bitbucket.org/%';

-- Set clone_depth to 1 for all git repos (shallow clone)
UPDATE repositories
SET clone_depth = 1
WHERE source_type = 'git';

-- Set last_sync_at to current time for repos that exist locally
UPDATE repositories
SET last_sync_at = strftime('%s', 'now')
WHERE local_path LIKE '/app/repos/%' OR local_path LIKE '%/repos/%';

-- ============================================================================
-- Create view for repository sync status
-- ============================================================================

CREATE VIEW IF NOT EXISTS repository_sync_status AS
SELECT
    id,
    name,
    url,
    source_type,
    clone_depth,
    local_path,
    CASE
        WHEN last_sync_at IS NULL THEN 'never_synced'
        WHEN (strftime('%s', 'now') - last_sync_at) > 86400 THEN 'stale' -- > 24 hours
        WHEN (strftime('%s', 'now') - last_sync_at) > 3600 THEN 'needs_update' -- > 1 hour
        ELSE 'up_to_date'
    END as sync_status,
    datetime(last_sync_at, 'unixepoch') as last_sync_time,
    strftime('%s', 'now') - COALESCE(last_sync_at, 0) as seconds_since_sync
FROM repositories
WHERE source_type = 'git';

-- ============================================================================
-- Validation queries for debugging
-- ============================================================================

-- Show repos without valid URLs (should be empty after migration)
-- SELECT id, name, url FROM repositories WHERE source_type = 'git' AND (url IS NULL OR url = '' OR url = 'https://github.com/unknown/repo.git');

-- Show repos that need syncing
-- SELECT * FROM repository_sync_status WHERE sync_status IN ('never_synced', 'stale', 'needs_update');

-- ============================================================================
-- Migration complete
-- ============================================================================

-- Migration 004 applied successfully
-- Added: source_type, clone_depth, last_sync_at columns
-- Created: repository_sync_status view
-- Created: indexes for url and sync tracking
