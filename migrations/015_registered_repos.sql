-- Migration 015: registered_repos table
-- Backs the in-memory RepoSyncService.repos HashMap with persistent SQLite storage.
-- Repos registered via POST /api/v1/repos survive server restarts.

CREATE TABLE IF NOT EXISTS registered_repos (
    id              TEXT PRIMARY KEY,               -- slugified repo name, e.g. "rustassistant"
    name            TEXT NOT NULL,                  -- human-readable name
    local_path      TEXT NOT NULL,                  -- absolute path on the host filesystem
    remote_url      TEXT,                           -- optional GitHub / GitLab URL
    branch          TEXT NOT NULL DEFAULT 'main',
    last_synced     INTEGER,                        -- Unix timestamp (seconds), NULL = never synced
    active          BOOLEAN NOT NULL DEFAULT 1,     -- soft-delete flag
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- Fast lookup by local path (used on registration to detect duplicates)
CREATE UNIQUE INDEX IF NOT EXISTS idx_registered_repos_local_path
    ON registered_repos (local_path)
    WHERE active = 1;

-- Filter active repos quickly (the common query in list_repos)
CREATE INDEX IF NOT EXISTS idx_registered_repos_active
    ON registered_repos (active);

-- Trigger: keep updated_at current on every UPDATE
CREATE TRIGGER IF NOT EXISTS trg_registered_repos_updated_at
    AFTER UPDATE ON registered_repos
    FOR EACH ROW
BEGIN
    UPDATE registered_repos
    SET updated_at = strftime('%s', 'now')
    WHERE id = NEW.id;
END;
