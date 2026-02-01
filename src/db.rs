//! # Database Module
//!
//! SQLite-based storage for notes, tags, and repositories.
//! This module provides the core data persistence layer for Rustassistant.
//!
//! ## Schema Overview
//!
//! - **notes**: Core note storage with content, status, and timestamps
//! - **tags**: Reusable tags for categorization
//! - **note_tags**: Many-to-many relationship between notes and tags
//! - **repositories**: Tracked git repositories with metadata
//!
//! ## Usage
//!
//! ```rust,no_run
//! use devflow::db::{Database, Note, NoteStatus};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let db = Database::new("data/rustassistant.db").await?;
//!
//!     let note_id = db.create_note("My first note", NoteStatus::Inbox).await?;
//!     db.add_tag_to_note(note_id, "idea").await?;
//!
//!     let notes = db.list_notes(None, None, None).await?;
//!     println!("Found {} notes", notes.len());
//!
//!     Ok(())
//! }
//! ```

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

/// Note status for workflow tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteStatus {
    /// Newly captured, not yet processed
    Inbox,
    /// Being actively worked on
    Active,
    /// Converted to task or implemented
    Processed,
    /// Parked for later consideration
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

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "inbox" => Some(NoteStatus::Inbox),
            "active" => Some(NoteStatus::Active),
            "processed" => Some(NoteStatus::Processed),
            "archived" => Some(NoteStatus::Archived),
            _ => None,
        }
    }
}

impl std::fmt::Display for NoteStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A note with content, tags, and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub content: String,
    pub status: NoteStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
}

impl Note {
    /// Get status as string for web UI
    pub fn status_str(&self) -> String {
        self.status.to_string()
    }

    /// Get tags as comma-separated string for web UI
    pub fn tags_str(&self) -> String {
        self.tags.join(",")
    }

    /// Format created_at for display
    pub fn created_at_formatted(&self) -> String {
        self.created_at.format("%Y-%m-%d %H:%M").to_string()
    }

    /// Format updated_at for display
    pub fn updated_at_formatted(&self) -> String {
        self.updated_at.format("%Y-%m-%d %H:%M").to_string()
    }
}

/// A repository being tracked
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub remote_url: Option<String>,
    pub default_branch: String,
    pub last_analyzed: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Repository {
    /// Format created_at for display
    pub fn created_at_formatted(&self) -> String {
        self.created_at.format("%Y-%m-%d %H:%M").to_string()
    }
}

/// LLM API cost record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCost {
    pub id: i64,
    pub model: String,
    pub operation: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub estimated_cost_usd: f64,
    pub repository_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

impl LlmCost {
    /// Format created_at for display
    pub fn created_at_formatted(&self) -> String {
        self.created_at.format("%Y-%m-%d %H:%M").to_string()
    }
}

/// Main database connection and operations
#[derive(Clone)]
pub struct Database {
    pool: sqlx::SqlitePool,
}

impl Database {
    /// Create a new database connection
    ///
    /// Creates the database file and schema if it doesn't exist.
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create database directory")?;
        }

        let database_url = format!("sqlite:{}?mode=rwc", path.display());

        let pool = sqlx::SqlitePool::connect(&database_url)
            .await
            .context("Failed to connect to database")?;

        let db = Self { pool };
        db.initialize_schema().await?;

        Ok(db)
    }

    /// Initialize the database schema
    async fn initialize_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'inbox',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create notes table")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create tags table")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS note_tags (
                note_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (note_id, tag_id),
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create note_tags table")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS repositories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                path TEXT NOT NULL UNIQUE,
                remote_url TEXT,
                default_branch TEXT NOT NULL DEFAULT 'main',
                last_analyzed TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create repositories table")?;

        // Create indexes for performance
        // Create LLM cost tracking table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS llm_costs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                model TEXT NOT NULL,
                operation TEXT NOT NULL,
                prompt_tokens INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                estimated_cost_usd REAL NOT NULL DEFAULT 0.0,
                repository_id INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (repository_id) REFERENCES repositories(id) ON DELETE SET NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create llm_costs table")?;

        // Create indexes for performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_notes_status ON notes(status)")
            .execute(&self.pool)
            .await
            .context("Failed to create notes status index")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_notes_created_at ON notes(created_at DESC)")
            .execute(&self.pool)
            .await
            .context("Failed to create notes created_at index")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name)")
            .execute(&self.pool)
            .await
            .context("Failed to create tags name index")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_llm_costs_created_at ON llm_costs(created_at DESC)",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create llm_costs created_at index")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_llm_costs_model ON llm_costs(model)")
            .execute(&self.pool)
            .await
            .context("Failed to create llm_costs model index")?;

        Ok(())
    }

    // ============================================================================
    // Note Operations
    // ============================================================================

    /// Create a new note
    pub async fn create_note(&self, content: &str, status: NoteStatus) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO notes (content, status)
            VALUES (?, ?)
            "#,
        )
        .bind(content)
        .bind(status.as_str())
        .execute(&self.pool)
        .await
        .context("Failed to create note")?;

        Ok(result.last_insert_rowid())
    }

    /// Get a note by ID with its tags
    pub async fn get_note(&self, id: i64) -> Result<Option<Note>> {
        let note_row = sqlx::query_as::<_, (i64, String, String, String, String)>(
            r#"
            SELECT id, content, status, created_at, updated_at
            FROM notes
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch note")?;

        match note_row {
            Some((id, content, status_str, created_at, updated_at)) => {
                let tags = self.get_note_tags(id).await?;

                Ok(Some(Note {
                    id,
                    content,
                    status: NoteStatus::from_str(&status_str).unwrap_or(NoteStatus::Inbox),
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .unwrap_or_else(|_| Utc::now().into())
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at)
                        .unwrap_or_else(|_| Utc::now().into())
                        .with_timezone(&Utc),
                    tags,
                }))
            }
            None => Ok(None),
        }
    }

    /// List notes with optional filtering
    pub async fn list_notes(
        &self,
        status: Option<NoteStatus>,
        tag: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<Note>> {
        let mut query = String::from(
            r#"
            SELECT DISTINCT n.id, n.content, n.status, n.created_at, n.updated_at
            FROM notes n
            "#,
        );

        let mut conditions = Vec::new();

        if tag.is_some() {
            query.push_str(
                r#"
                JOIN note_tags nt ON n.id = nt.note_id
                JOIN tags t ON nt.tag_id = t.id
                "#,
            );
            conditions.push("t.name = ?");
        }

        if status.is_some() {
            conditions.push("n.status = ?");
        }

        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(" ORDER BY n.created_at DESC");

        if let Some(limit_val) = limit {
            query.push_str(&format!(" LIMIT {}", limit_val));
        }

        let mut query_builder = sqlx::query_as::<_, (i64, String, String, String, String)>(&query);

        if let Some(tag_name) = tag {
            query_builder = query_builder.bind(tag_name);
        }

        if let Some(status_val) = status {
            query_builder = query_builder.bind(status_val.as_str());
        }

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .context("Failed to list notes")?;

        let mut notes = Vec::new();
        for (id, content, status_str, created_at, updated_at) in rows {
            let tags = self.get_note_tags(id).await?;

            notes.push(Note {
                id,
                content,
                status: NoteStatus::from_str(&status_str).unwrap_or(NoteStatus::Inbox),
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                tags,
            });
        }

        Ok(notes)
    }

    /// Update note content
    pub async fn update_note_content(&self, id: i64, content: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE notes
            SET content = ?, updated_at = datetime('now')
            WHERE id = ?
            "#,
        )
        .bind(content)
        .bind(id)
        .execute(&self.pool)
        .await
        .context("Failed to update note content")?;

        Ok(())
    }

    /// Update note status
    pub async fn update_note_status(&self, id: i64, status: NoteStatus) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE notes
            SET status = ?, updated_at = datetime('now')
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(id)
        .execute(&self.pool)
        .await
        .context("Failed to update note status")?;

        Ok(())
    }

    /// Delete a note
    pub async fn delete_note(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM notes WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Failed to delete note")?;

        Ok(())
    }

    /// Search notes by content
    pub async fn search_notes(&self, query: &str) -> Result<Vec<Note>> {
        let search_pattern = format!("%{}%", query);

        let rows = sqlx::query_as::<_, (i64, String, String, String, String)>(
            r#"
            SELECT id, content, status, created_at, updated_at
            FROM notes
            WHERE content LIKE ?
            ORDER BY created_at DESC
            "#,
        )
        .bind(&search_pattern)
        .fetch_all(&self.pool)
        .await
        .context("Failed to search notes")?;

        let mut notes = Vec::new();
        for (id, content, status_str, created_at, updated_at) in rows {
            let tags = self.get_note_tags(id).await?;

            notes.push(Note {
                id,
                content,
                status: NoteStatus::from_str(&status_str).unwrap_or(NoteStatus::Inbox),
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                tags,
            });
        }

        Ok(notes)
    }

    // ============================================================================
    // Tag Operations
    // ============================================================================

    /// Get or create a tag by name
    async fn get_or_create_tag(&self, name: &str) -> Result<i64> {
        // Try to get existing tag
        let existing = sqlx::query_as::<_, (i64,)>("SELECT id FROM tags WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to query tag")?;

        if let Some((id,)) = existing {
            return Ok(id);
        }

        // Create new tag
        let result = sqlx::query("INSERT INTO tags (name) VALUES (?)")
            .bind(name)
            .execute(&self.pool)
            .await
            .context("Failed to create tag")?;

        Ok(result.last_insert_rowid())
    }

    /// Add a tag to a note
    pub async fn add_tag_to_note(&self, note_id: i64, tag_name: &str) -> Result<()> {
        let tag_id = self.get_or_create_tag(tag_name).await?;

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO note_tags (note_id, tag_id)
            VALUES (?, ?)
            "#,
        )
        .bind(note_id)
        .bind(tag_id)
        .execute(&self.pool)
        .await
        .context("Failed to add tag to note")?;

        Ok(())
    }

    /// Remove a tag from a note
    pub async fn remove_tag_from_note(&self, note_id: i64, tag_name: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM note_tags
            WHERE note_id = ? AND tag_id = (SELECT id FROM tags WHERE name = ?)
            "#,
        )
        .bind(note_id)
        .bind(tag_name)
        .execute(&self.pool)
        .await
        .context("Failed to remove tag from note")?;

        Ok(())
    }

    /// Get all tags for a note
    async fn get_note_tags(&self, note_id: i64) -> Result<Vec<String>> {
        let tags = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT t.name
            FROM tags t
            JOIN note_tags nt ON t.id = nt.tag_id
            WHERE nt.note_id = ?
            ORDER BY t.name
            "#,
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch note tags")?;

        Ok(tags.into_iter().map(|(name,)| name).collect())
    }

    /// List all tags with usage counts
    pub async fn list_tags(&self) -> Result<Vec<(String, i64)>> {
        let tags = sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT t.name, COUNT(nt.note_id) as count
            FROM tags t
            LEFT JOIN note_tags nt ON t.id = nt.tag_id
            GROUP BY t.id, t.name
            ORDER BY count DESC, t.name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list tags")?;

        Ok(tags)
    }

    // ============================================================================
    // Repository Operations
    // ============================================================================

    /// Add a repository to track
    pub async fn add_repository(
        &self,
        name: &str,
        path: &str,
        remote_url: Option<&str>,
        default_branch: &str,
    ) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO repositories (name, path, remote_url, default_branch)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(name)
        .bind(path)
        .bind(remote_url)
        .bind(default_branch)
        .execute(&self.pool)
        .await
        .context("Failed to add repository")?;

        Ok(result.last_insert_rowid())
    }

    /// Get a repository by name
    pub async fn get_repository(&self, name: &str) -> Result<Option<Repository>> {
        let repo = sqlx::query_as::<
            _,
            (
                i64,
                String,
                String,
                Option<String>,
                String,
                Option<String>,
                String,
                String,
            ),
        >(
            r#"
            SELECT id, name, path, remote_url, default_branch, last_analyzed, created_at, updated_at
            FROM repositories
            WHERE name = ?
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch repository")?;

        Ok(repo.map(
            |(
                id,
                name,
                path,
                remote_url,
                default_branch,
                last_analyzed,
                created_at,
                updated_at,
            )| {
                Repository {
                    id,
                    name,
                    path,
                    remote_url,
                    default_branch,
                    last_analyzed: last_analyzed.and_then(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    }),
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .unwrap_or_else(|_| Utc::now().into())
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at)
                        .unwrap_or_else(|_| Utc::now().into())
                        .with_timezone(&Utc),
                }
            },
        ))
    }

    /// List all repositories
    pub async fn list_repositories(&self) -> Result<Vec<Repository>> {
        let repos = sqlx::query_as::<
            _,
            (
                i64,
                String,
                String,
                Option<String>,
                String,
                Option<String>,
                String,
                String,
            ),
        >(
            r#"
            SELECT id, name, path, remote_url, default_branch, last_analyzed, created_at, updated_at
            FROM repositories
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list repositories")?;

        Ok(repos
            .into_iter()
            .map(
                |(
                    id,
                    name,
                    path,
                    remote_url,
                    default_branch,
                    last_analyzed,
                    created_at,
                    updated_at,
                )| {
                    Repository {
                        id,
                        name,
                        path,
                        remote_url,
                        default_branch,
                        last_analyzed: last_analyzed.and_then(|s| {
                            DateTime::parse_from_rfc3339(&s)
                                .ok()
                                .map(|dt| dt.with_timezone(&Utc))
                        }),
                        created_at: DateTime::parse_from_rfc3339(&created_at)
                            .unwrap_or_else(|_| Utc::now().into())
                            .with_timezone(&Utc),
                        updated_at: DateTime::parse_from_rfc3339(&updated_at)
                            .unwrap_or_else(|_| Utc::now().into())
                            .with_timezone(&Utc),
                    }
                },
            )
            .collect())
    }

    /// Update repository's last analyzed timestamp
    pub async fn update_repository_analyzed(&self, id: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE repositories
            SET last_analyzed = datetime('now'), updated_at = datetime('now')
            WHERE id = ?
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .context("Failed to update repository analyzed timestamp")?;

        Ok(())
    }

    /// Delete a repository
    pub async fn delete_repository(&self, name: &str) -> Result<()> {
        sqlx::query("DELETE FROM repositories WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await
            .context("Failed to delete repository")?;

        Ok(())
    }

    // ============================================================================
    // LLM Cost Tracking
    // ============================================================================

    /// Record an LLM API call cost
    pub async fn record_llm_cost(
        &self,
        model: &str,
        operation: &str,
        prompt_tokens: i64,
        completion_tokens: i64,
        estimated_cost_usd: f64,
        repository_id: Option<i64>,
    ) -> Result<i64> {
        let total_tokens = prompt_tokens + completion_tokens;

        let result = sqlx::query(
            r#"
            INSERT INTO llm_costs (model, operation, prompt_tokens, completion_tokens, total_tokens, estimated_cost_usd, repository_id)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(model)
        .bind(operation)
        .bind(prompt_tokens)
        .bind(completion_tokens)
        .bind(total_tokens)
        .bind(estimated_cost_usd)
        .bind(repository_id)
        .execute(&self.pool)
        .await
        .context("Failed to record LLM cost")?;

        Ok(result.last_insert_rowid())
    }

    /// Get total LLM costs
    pub async fn get_total_llm_cost(&self) -> Result<f64> {
        let (total,) = sqlx::query_as::<_, (f64,)>(
            "SELECT COALESCE(SUM(estimated_cost_usd), 0.0) FROM llm_costs",
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get total LLM cost")?;

        Ok(total)
    }

    /// Get LLM costs for a specific time period (days)
    pub async fn get_llm_cost_by_period(&self, days: i64) -> Result<f64> {
        let (total,) = sqlx::query_as::<_, (f64,)>(
            r#"
            SELECT COALESCE(SUM(estimated_cost_usd), 0.0)
            FROM llm_costs
            WHERE created_at >= datetime('now', '-' || ? || ' days')
            "#,
        )
        .bind(days)
        .fetch_one(&self.pool)
        .await
        .context("Failed to get LLM cost by period")?;

        Ok(total)
    }

    /// Get cost breakdown by model
    pub async fn get_cost_by_model(&self) -> Result<Vec<(String, f64, i64)>> {
        let costs = sqlx::query_as::<_, (String, f64, i64)>(
            r#"
            SELECT model, SUM(estimated_cost_usd), SUM(total_tokens)
            FROM llm_costs
            GROUP BY model
            ORDER BY SUM(estimated_cost_usd) DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get cost by model")?;

        Ok(costs)
    }

    /// Get recent LLM operations
    pub async fn get_recent_llm_operations(&self, limit: i64) -> Result<Vec<LlmCost>> {
        let operations = sqlx::query_as::<_, (i64, String, String, i64, i64, i64, f64, Option<i64>, String)>(
            r#"
            SELECT id, model, operation, prompt_tokens, completion_tokens, total_tokens, estimated_cost_usd, repository_id, created_at
            FROM llm_costs
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get recent LLM operations")?;

        Ok(operations
            .into_iter()
            .map(
                |(
                    id,
                    model,
                    operation,
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                    estimated_cost_usd,
                    repository_id,
                    created_at,
                )| {
                    LlmCost {
                        id,
                        model,
                        operation,
                        prompt_tokens,
                        completion_tokens,
                        total_tokens,
                        estimated_cost_usd,
                        repository_id,
                        created_at: DateTime::parse_from_rfc3339(&created_at)
                            .unwrap_or_else(|_| Utc::now().into())
                            .with_timezone(&Utc),
                    }
                },
            )
            .collect())
    }

    // ============================================================================
    // Statistics
    // ============================================================================

    /// Get database statistics
    pub async fn get_stats(&self) -> Result<DatabaseStats> {
        let (total_notes,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM notes")
            .fetch_one(&self.pool)
            .await
            .context("Failed to count notes")?;

        let (inbox_notes,) =
            sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM notes WHERE status = 'inbox'")
                .fetch_one(&self.pool)
                .await
                .context("Failed to count inbox notes")?;

        let (total_tags,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM tags")
            .fetch_one(&self.pool)
            .await
            .context("Failed to count tags")?;

        let (total_repos,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM repositories")
            .fetch_one(&self.pool)
            .await
            .context("Failed to count repositories")?;

        Ok(DatabaseStats {
            total_notes,
            inbox_notes,
            total_tags,
            total_repositories: total_repos,
        })
    }

    /// Count total notes
    pub async fn count_notes(&self) -> Result<i64> {
        let (count,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM notes")
            .fetch_one(&self.pool)
            .await
            .context("Failed to count notes")?;
        Ok(count)
    }

    /// Count total repositories
    pub async fn count_repositories(&self) -> Result<i64> {
        let (count,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM repositories")
            .fetch_one(&self.pool)
            .await
            .context("Failed to count repositories")?;
        Ok(count)
    }
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub total_notes: i64,
    pub inbox_notes: i64,
    pub total_tags: i64,
    pub total_repositories: i64,
}

/// LLM cost statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCostStats {
    pub total_cost: f64,
    pub cost_last_24h: f64,
    pub cost_last_7d: f64,
    pub cost_last_30d: f64,
    pub by_model: Vec<(String, f64, i64)>, // (model, cost, tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_note() -> Result<()> {
        let db = Database::new(":memory:").await?;

        let note_id = db.create_note("Test note", NoteStatus::Inbox).await?;
        let note = db.get_note(note_id).await?;

        assert!(note.is_some());
        let note = note.unwrap();
        assert_eq!(note.content, "Test note");
        assert_eq!(note.status, NoteStatus::Inbox);

        Ok(())
    }

    #[tokio::test]
    async fn test_add_tags_to_note() -> Result<()> {
        let db = Database::new(":memory:").await?;

        let note_id = db.create_note("Test note", NoteStatus::Inbox).await?;
        db.add_tag_to_note(note_id, "idea").await?;
        db.add_tag_to_note(note_id, "rust").await?;

        let note = db.get_note(note_id).await?.unwrap();
        assert_eq!(note.tags.len(), 2);
        assert!(note.tags.contains(&"idea".to_string()));
        assert!(note.tags.contains(&"rust".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_list_notes_by_tag() -> Result<()> {
        let db = Database::new(":memory:").await?;

        let note1 = db.create_note("Note 1", NoteStatus::Inbox).await?;
        db.add_tag_to_note(note1, "idea").await?;

        let note2 = db.create_note("Note 2", NoteStatus::Inbox).await?;
        db.add_tag_to_note(note2, "bug").await?;

        let idea_notes = db.list_notes(None, Some("idea"), None).await?;
        assert_eq!(idea_notes.len(), 1);
        assert_eq!(idea_notes[0].content, "Note 1");

        Ok(())
    }

    #[tokio::test]
    async fn test_search_notes() -> Result<()> {
        let db = Database::new(":memory:").await?;

        db.create_note("Implement search feature", NoteStatus::Inbox)
            .await?;
        db.create_note("Fix bug in parser", NoteStatus::Inbox)
            .await?;

        let results = db.search_notes("search").await?;
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("search"));

        Ok(())
    }
}
