//! Database module for Rustassistant
//!
//! Provides SQLite-based storage for notes, repositories, and tasks.
//! Uses sqlx for async database operations.

use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, FromRow, SqlitePool};
use thiserror::Error;

use super::queue::create_queue_tables;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

pub type DbResult<T> = Result<T, DbError>;

// ============================================================================
// Models
// ============================================================================

/// A note/thought captured by the user
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Note {
    pub id: String,
    pub content: String,
    pub tags: Option<String>,
    pub project: Option<String>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Note {
    /// Get status as a string (legacy API)
    pub fn status_str(&self) -> &str {
        &self.status
    }

    /// Get formatted created_at timestamp (legacy API)
    pub fn created_at_formatted(&self) -> String {
        chrono::DateTime::from_timestamp(self.created_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Get formatted updated_at timestamp (legacy API)
    pub fn updated_at_formatted(&self) -> String {
        chrono::DateTime::from_timestamp(self.updated_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// A tracked repository
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Repository {
    pub id: String,
    pub path: String,
    pub name: String,
    pub status: String,
    pub last_analyzed: Option<i64>,
    pub metadata: Option<String>, // JSON blob
    pub created_at: i64,
    pub updated_at: i64,
}

impl Repository {
    /// Get formatted created_at timestamp (legacy API)
    pub fn created_at_formatted(&self) -> String {
        chrono::DateTime::from_timestamp(self.created_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// A generated or manual task
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: i32, // 1=critical, 2=high, 3=medium, 4=low
    pub status: String,
    pub source: String,            // "note", "analysis", "manual"
    pub source_id: Option<String>, // ID of note or file that generated this
    pub repo_id: Option<String>,
    pub file_path: Option<String>,
    pub line_number: Option<i32>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ============================================================================
// Database Initialization
// ============================================================================

/// Initialize the database connection pool and create tables
pub async fn init_db(database_url: &str) -> DbResult<SqlitePool> {
    // Create the database file directory if needed
    if database_url.starts_with("sqlite:") {
        let path = database_url.trim_start_matches("sqlite:");
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // Run migrations (create tables)
    create_tables(&pool).await?;

    // Create queue system tables
    create_queue_tables(&pool).await.map_err(DbError::Sqlx)?;

    Ok(pool)
}

/// Create all required tables
async fn create_tables(pool: &SqlitePool) -> DbResult<()> {
    // Notes table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            tags TEXT,
            project TEXT,
            status TEXT NOT NULL DEFAULT 'inbox',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Repositories table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS repositories (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            last_analyzed INTEGER,
            metadata TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Tasks table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            priority INTEGER NOT NULL DEFAULT 3,
            status TEXT NOT NULL DEFAULT 'pending',
            source TEXT NOT NULL DEFAULT 'manual',
            source_id TEXT,
            repo_id TEXT,
            file_path TEXT,
            line_number INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (repo_id) REFERENCES repositories(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create indexes for common queries
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_notes_status ON notes(status)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_notes_project ON notes(project)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_notes_created ON notes(created_at DESC)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority, status)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_repo ON tasks(repo_id)")
        .execute(pool)
        .await?;

    Ok(())
}

// ============================================================================
// Note Operations
// ============================================================================

/// Create a new note
pub async fn create_note(
    pool: &SqlitePool,
    content: &str,
    tags: Option<&str>,
    project: Option<&str>,
) -> DbResult<Note> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        r#"
        INSERT INTO notes (id, content, tags, project, status, created_at, updated_at)
        VALUES (?, ?, ?, ?, 'inbox', ?, ?)
        "#,
    )
    .bind(&id)
    .bind(content)
    .bind(tags)
    .bind(project)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(Note {
        id,
        content: content.to_string(),
        tags: tags.map(|s| s.to_string()),
        project: project.map(|s| s.to_string()),
        status: "inbox".to_string(),
        created_at: now,
        updated_at: now,
    })
}

/// Get a note by ID
pub async fn get_note(pool: &SqlitePool, id: &str) -> DbResult<Note> {
    sqlx::query_as::<_, Note>("SELECT * FROM notes WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Note not found: {}", id)))
}

/// List notes with optional filtering
pub async fn list_notes(
    pool: &SqlitePool,
    limit: i64,
    status: Option<&str>,
    project: Option<&str>,
    tag: Option<&str>,
) -> DbResult<Vec<Note>> {
    let mut query = String::from("SELECT * FROM notes WHERE 1=1");

    if status.is_some() {
        query.push_str(" AND status = ?");
    }
    if project.is_some() {
        query.push_str(" AND project = ?");
    }
    if tag.is_some() {
        query.push_str(" AND tags LIKE ?");
    }

    query.push_str(" ORDER BY created_at DESC LIMIT ?");

    let mut q = sqlx::query_as::<_, Note>(&query);

    if let Some(s) = status {
        q = q.bind(s);
    }
    if let Some(p) = project {
        q = q.bind(p);
    }
    if let Some(t) = tag {
        q = q.bind(format!("%{}%", t));
    }
    q = q.bind(limit);

    Ok(q.fetch_all(pool).await?)
}

/// Search notes by content
pub async fn search_notes(pool: &SqlitePool, query: &str, limit: i64) -> DbResult<Vec<Note>> {
    let search_pattern = format!("%{}%", query);

    Ok(sqlx::query_as::<_, Note>(
        r#"
        SELECT * FROM notes
        WHERE content LIKE ? OR tags LIKE ?
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(&search_pattern)
    .bind(&search_pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

/// Update note status
pub async fn update_note_status(pool: &SqlitePool, id: &str, status: &str) -> DbResult<()> {
    let now = chrono::Utc::now().timestamp();

    let result = sqlx::query("UPDATE notes SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(DbError::NotFound(format!("Note not found: {}", id)));
    }

    Ok(())
}

/// Delete a note
pub async fn delete_note(pool: &SqlitePool, id: &str) -> DbResult<()> {
    let result = sqlx::query("DELETE FROM notes WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(DbError::NotFound(format!("Note not found: {}", id)));
    }

    Ok(())
}

// ============================================================================
// Repository Operations
// ============================================================================

/// Add a repository to track
pub async fn add_repository(pool: &SqlitePool, path: &str, name: &str) -> DbResult<Repository> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        r#"
        INSERT INTO repositories (id, path, name, status, created_at, updated_at)
        VALUES (?, ?, ?, 'active', ?, ?)
        "#,
    )
    .bind(&id)
    .bind(path)
    .bind(name)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(Repository {
        id,
        path: path.to_string(),
        name: name.to_string(),
        status: "active".to_string(),
        last_analyzed: None,
        metadata: None,
        created_at: now,
        updated_at: now,
    })
}

/// Get a repository by ID
pub async fn get_repository(pool: &SqlitePool, id: &str) -> DbResult<Repository> {
    sqlx::query_as::<_, Repository>("SELECT * FROM repositories WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Repository not found: {}", id)))
}

/// Get a repository by path
pub async fn get_repository_by_path(pool: &SqlitePool, path: &str) -> DbResult<Option<Repository>> {
    Ok(
        sqlx::query_as::<_, Repository>("SELECT * FROM repositories WHERE path = ?")
            .bind(path)
            .fetch_optional(pool)
            .await?,
    )
}

/// List all repositories
pub async fn list_repositories(pool: &SqlitePool) -> DbResult<Vec<Repository>> {
    Ok(
        sqlx::query_as::<_, Repository>("SELECT * FROM repositories ORDER BY name ASC")
            .fetch_all(pool)
            .await?,
    )
}

/// Update repository analysis timestamp and metadata
pub async fn update_repository_analysis(
    pool: &SqlitePool,
    id: &str,
    metadata: Option<&str>,
) -> DbResult<()> {
    let now = chrono::Utc::now().timestamp();

    let result = sqlx::query(
        "UPDATE repositories SET last_analyzed = ?, metadata = ?, updated_at = ? WHERE id = ?",
    )
    .bind(now)
    .bind(metadata)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(DbError::NotFound(format!("Repository not found: {}", id)));
    }

    Ok(())
}

/// Remove a repository
pub async fn remove_repository(pool: &SqlitePool, id: &str) -> DbResult<()> {
    let result = sqlx::query("DELETE FROM repositories WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(DbError::NotFound(format!("Repository not found: {}", id)));
    }

    Ok(())
}

// ============================================================================
// Task Operations
// ============================================================================

/// Create a new task
#[allow(clippy::too_many_arguments)]
pub async fn create_task(
    pool: &SqlitePool,
    title: &str,
    description: Option<&str>,
    priority: i32,
    source: &str,
    source_id: Option<&str>,
    repo_id: Option<&str>,
    file_path: Option<&str>,
    line_number: Option<i32>,
) -> DbResult<Task> {
    let id = format!(
        "TASK-{}",
        &uuid::Uuid::new_v4().to_string()[..8].to_uppercase()
    );
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        r#"
        INSERT INTO tasks (id, title, description, priority, status, source, source_id,
                          repo_id, file_path, line_number, created_at, updated_at)
        VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(title)
    .bind(description)
    .bind(priority)
    .bind(source)
    .bind(source_id)
    .bind(repo_id)
    .bind(file_path)
    .bind(line_number)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(Task {
        id,
        title: title.to_string(),
        description: description.map(|s| s.to_string()),
        priority,
        status: "pending".to_string(),
        source: source.to_string(),
        source_id: source_id.map(|s| s.to_string()),
        repo_id: repo_id.map(|s| s.to_string()),
        file_path: file_path.map(|s| s.to_string()),
        line_number,
        created_at: now,
        updated_at: now,
    })
}

/// List tasks with optional filtering
pub async fn list_tasks(
    pool: &SqlitePool,
    limit: i64,
    status: Option<&str>,
    priority: Option<i32>,
    repo_id: Option<&str>,
) -> DbResult<Vec<Task>> {
    let mut query = String::from("SELECT * FROM tasks WHERE 1=1");

    if status.is_some() {
        query.push_str(" AND status = ?");
    }
    if priority.is_some() {
        query.push_str(" AND priority <= ?");
    }
    if repo_id.is_some() {
        query.push_str(" AND repo_id = ?");
    }

    query.push_str(" ORDER BY priority ASC, created_at DESC LIMIT ?");

    let mut q = sqlx::query_as::<_, Task>(&query);

    if let Some(s) = status {
        q = q.bind(s);
    }
    if let Some(p) = priority {
        q = q.bind(p);
    }
    if let Some(r) = repo_id {
        q = q.bind(r);
    }
    q = q.bind(limit);

    Ok(q.fetch_all(pool).await?)
}

/// Update task status
pub async fn update_task_status(pool: &SqlitePool, id: &str, status: &str) -> DbResult<()> {
    let now = chrono::Utc::now().timestamp();

    let result = sqlx::query("UPDATE tasks SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(DbError::NotFound(format!("Task not found: {}", id)));
    }

    Ok(())
}

/// Get the next recommended task (highest priority pending task)
pub async fn get_next_task(pool: &SqlitePool) -> DbResult<Option<Task>> {
    Ok(sqlx::query_as::<_, Task>(
        r#"
        SELECT * FROM tasks
        WHERE status = 'pending'
        ORDER BY priority ASC, created_at ASC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?)
}

// ============================================================================
// Statistics
// ============================================================================

/// Get database statistics
#[derive(Debug, Serialize)]
pub struct DbStats {
    pub total_notes: i64,
    pub inbox_notes: i64,
    pub total_repos: i64,
    pub total_tasks: i64,
    pub pending_tasks: i64,
}

pub async fn get_stats(pool: &SqlitePool) -> DbResult<DbStats> {
    let total_notes: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes")
        .fetch_one(pool)
        .await?;

    let inbox_notes: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes WHERE status = 'inbox'")
        .fetch_one(pool)
        .await?;

    let total_repos: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM repositories")
        .fetch_one(pool)
        .await?;

    let total_tasks: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks")
        .fetch_one(pool)
        .await?;

    let pending_tasks: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status = 'pending'")
            .fetch_one(pool)
            .await?;

    Ok(DbStats {
        total_notes: total_notes.0,
        inbox_notes: inbox_notes.0,
        total_repos: total_repos.0,
        total_tasks: total_tasks.0,
        pending_tasks: pending_tasks.0,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> SqlitePool {
        init_db("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_create_and_get_note() {
        let pool = setup_test_db().await;

        let note = create_note(
            &pool,
            "Test note content",
            Some("tag1,tag2"),
            Some("testproject"),
        )
        .await
        .unwrap();

        assert_eq!(note.content, "Test note content");
        assert_eq!(note.tags, Some("tag1,tag2".to_string()));
        assert_eq!(note.project, Some("testproject".to_string()));
        assert_eq!(note.status, "inbox");

        let fetched = get_note(&pool, &note.id).await.unwrap();
        assert_eq!(fetched.id, note.id);
        assert_eq!(fetched.content, note.content);
    }

    #[tokio::test]
    async fn test_list_notes() {
        let pool = setup_test_db().await;

        create_note(&pool, "Note 1", None, None).await.unwrap();
        create_note(&pool, "Note 2", Some("important"), None)
            .await
            .unwrap();
        create_note(&pool, "Note 3", None, Some("project1"))
            .await
            .unwrap();

        let all_notes = list_notes(&pool, 10, None, None, None).await.unwrap();
        assert_eq!(all_notes.len(), 3);

        let project_notes = list_notes(&pool, 10, None, Some("project1"), None)
            .await
            .unwrap();
        assert_eq!(project_notes.len(), 1);
    }

    #[tokio::test]
    async fn test_search_notes() {
        let pool = setup_test_db().await;

        create_note(&pool, "Rust programming tips", Some("rust,dev"), None)
            .await
            .unwrap();
        create_note(&pool, "Python basics", Some("python"), None)
            .await
            .unwrap();

        let results = search_notes(&pool, "Rust", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("Rust"));
    }

    #[tokio::test]
    async fn test_repository_crud() {
        let pool = setup_test_db().await;

        let repo = add_repository(&pool, "/path/to/repo", "my-repo")
            .await
            .unwrap();

        assert_eq!(repo.name, "my-repo");
        assert_eq!(repo.path, "/path/to/repo");

        let repos = list_repositories(&pool).await.unwrap();
        assert_eq!(repos.len(), 1);

        remove_repository(&pool, &repo.id).await.unwrap();
        let repos = list_repositories(&pool).await.unwrap();
        assert_eq!(repos.len(), 0);
    }

    #[tokio::test]
    async fn test_task_creation_and_next() {
        let pool = setup_test_db().await;

        // Create tasks with different priorities
        create_task(
            &pool,
            "Low priority task",
            None,
            4,
            "manual",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        create_task(
            &pool,
            "High priority task",
            None,
            2,
            "manual",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        create_task(
            &pool,
            "Critical task",
            None,
            1,
            "manual",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // get_next_task should return the critical task
        let next = get_next_task(&pool).await.unwrap().unwrap();
        assert_eq!(next.title, "Critical task");
        assert_eq!(next.priority, 1);
    }

    #[tokio::test]
    async fn test_stats() {
        let pool = setup_test_db().await;

        create_note(&pool, "Note 1", None, None).await.unwrap();
        create_note(&pool, "Note 2", None, None).await.unwrap();
        add_repository(&pool, "/path", "repo").await.unwrap();
        create_task(&pool, "Task 1", None, 2, "manual", None, None, None, None)
            .await
            .unwrap();

        let stats = get_stats(&pool).await.unwrap();
        assert_eq!(stats.total_notes, 2);
        assert_eq!(stats.inbox_notes, 2);
        assert_eq!(stats.total_repos, 1);
        assert_eq!(stats.total_tasks, 1);
        assert_eq!(stats.pending_tasks, 1);
    }
}

// ============================================================================
// Backward Compatibility Layer
// ============================================================================
// This Database struct provides compatibility with existing code that uses
// the old struct-based API. New code should use the function-based API above.

/// Backward-compatible Database wrapper
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Create a new database connection (legacy API)
    pub async fn new(database_url: &str) -> DbResult<Self> {
        let pool = init_db(database_url).await?;
        Ok(Self { pool })
    }

    /// Create a Database from an existing SqlitePool
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get a reference to the pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Create a note (legacy API)
    pub async fn create_note(&self, content: &str, status: NoteStatus) -> DbResult<String> {
        let note = create_note(&self.pool, content, None, None).await?;
        if status.as_str() != "inbox" {
            update_note_status(&self.pool, &note.id, status.as_str()).await?;
        }
        Ok(note.id)
    }

    /// Get a note by ID (legacy API)
    pub async fn get_note(&self, id: &str) -> DbResult<Note> {
        get_note(&self.pool, id).await
    }

    /// List notes (legacy API)
    pub async fn list_notes(
        &self,
        status: Option<NoteStatus>,
        limit: Option<i64>,
        _offset: Option<i64>,
    ) -> DbResult<Vec<Note>> {
        let limit = limit.unwrap_or(50);
        let status_str = status.map(|s| s.as_str());
        list_notes(&self.pool, limit, status_str, None, None).await
    }

    /// Add a repository (legacy API)
    pub async fn add_repository(
        &self,
        name: &str,
        path: &str,
        _remote_url: Option<String>,
        _default_branch: Option<String>,
    ) -> DbResult<String> {
        let repo = add_repository(&self.pool, path, name).await?;
        Ok(repo.id)
    }

    /// Get a repository by ID (legacy API)
    pub async fn get_repository(&self, id: &str) -> DbResult<Repository> {
        get_repository(&self.pool, id).await
    }

    /// List repositories (legacy API)
    pub async fn list_repositories(&self) -> DbResult<Vec<Repository>> {
        list_repositories(&self.pool).await
    }

    /// Record LLM cost (legacy API - now a no-op, consider removing calls)
    pub async fn record_llm_cost(
        &self,
        _model: &str,
        _operation: &str,
        _prompt_tokens: i64,
        _completion_tokens: i64,
        _estimated_cost_usd: f64,
        _repository_id: Option<i64>,
    ) -> DbResult<()> {
        // Legacy API - no longer storing LLM costs in new schema
        // Keep as no-op for compatibility
        Ok(())
    }

    /// Get total LLM cost (legacy API - returns 0.0)
    pub async fn get_total_llm_cost(&self) -> DbResult<f64> {
        Ok(0.0)
    }

    /// Get LLM cost by period (legacy API - returns 0.0)
    pub async fn get_llm_cost_by_period(&self, _hours: i64) -> DbResult<f64> {
        Ok(0.0)
    }

    /// Get cache hit rate from llm_costs table (last 30 days)
    ///
    /// Returns the percentage of queries that were cache hits (0-100).
    /// Returns 0 if no data or if the llm_costs table doesn't exist.
    pub async fn get_cache_hit_rate(&self) -> DbResult<i64> {
        // Query cache hit stats from llm_costs table (created by CostTracker)
        let result = sqlx::query_as::<_, (i64, i64)>(
            r#"
            SELECT
                COUNT(*) as total,
                COALESCE(SUM(CASE WHEN cache_hit = TRUE THEN 1 ELSE 0 END), 0) as hits
            FROM llm_costs
            WHERE timestamp >= datetime('now', '-30 days')
            "#,
        )
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(Some((total, hits))) if total > 0 => {
                Ok(((hits as f64 / total as f64) * 100.0) as i64)
            }
            _ => Ok(0), // No data or table doesn't exist
        }
    }

    /// Get cost by model (legacy API - returns empty map)
    pub async fn get_cost_by_model(&self) -> DbResult<std::collections::HashMap<String, f64>> {
        Ok(std::collections::HashMap::new())
    }

    /// Count notes (legacy API)
    pub async fn count_notes(&self) -> DbResult<i64> {
        let stats = get_stats(&self.pool).await?;
        Ok(stats.total_notes)
    }

    /// Count repositories (legacy API)
    pub async fn count_repositories(&self) -> DbResult<i64> {
        let stats = get_stats(&self.pool).await?;
        Ok(stats.total_repos)
    }

    /// Get recent LLM operations (legacy API - returns empty vec)
    pub async fn get_recent_llm_operations(&self, _limit: i64) -> DbResult<Vec<LlmCost>> {
        Ok(Vec::new())
    }

    /// Get stats (legacy API)
    pub async fn get_stats(&self) -> DbResult<DatabaseStats> {
        let stats = get_stats(&self.pool).await?;
        Ok(DatabaseStats {
            total_notes: stats.total_notes,
            inbox_notes: stats.inbox_notes,
            total_tags: 0, // Not tracked in new schema
            total_repositories: stats.total_repos,
        })
    }
}

/// Legacy note status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteStatus {
    Inbox,
    Active,
    Processed,
    Archived,
}

impl NoteStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            NoteStatus::Inbox => "inbox",
            NoteStatus::Active => "active",
            NoteStatus::Processed => "processed",
            NoteStatus::Archived => "archived",
        }
    }
}

/// Legacy stats struct
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_notes: i64,
    pub inbox_notes: i64,
    pub total_tags: i64,
    pub total_repositories: i64,
}

/// Legacy LlmCost struct (kept for compatibility)
#[derive(Debug, Clone)]
pub struct LlmCost {
    pub id: String,
    pub model: String,
    pub operation: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub estimated_cost_usd: f64,
    pub repository_id: Option<String>,
    pub created_at: i64,
}

impl LlmCost {
    /// Get formatted created_at timestamp (legacy API)
    pub fn created_at_formatted(&self) -> String {
        chrono::DateTime::from_timestamp(self.created_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// Legacy LlmCostStats struct (kept for compatibility)
#[derive(Debug, Clone)]
pub struct LlmCostStats {
    pub total_cost: f64,
    pub cost_last_24h: f64,
    pub cost_last_7d: f64,
    pub cost_last_30d: f64,
    pub by_model: std::collections::HashMap<String, f64>,
}
