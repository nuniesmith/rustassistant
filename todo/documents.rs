// src/db/documents.rs
//! Documents & Ideas - Knowledge base and quick thought capture.
//!
//! Documents: Long-form research, reference material, architecture decisions.
//! Ideas: Quick thoughts, fleeting notes, tagged for organization.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

// ============================================================================
// Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    pub source_url: Option<String>,
    pub doc_type: String,
    pub tags: Option<String>,
    pub project: Option<String>,
    pub repo_id: Option<String>,
    pub format: String,
    pub word_count: i32,
    pub is_indexed: i32,
    pub embedding_id: Option<String>,
    pub pinned: i32,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Idea {
    pub id: String,
    pub content: String,
    pub tags: Option<String>,
    pub project: Option<String>,
    pub repo_id: Option<String>,
    pub priority: i32,
    pub status: String,
    pub category: Option<String>,
    pub linked_doc_id: Option<String>,
    pub linked_task_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub usage_count: i32,
    pub created_at: i64,
}

// Helper for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub doc_type: String,
    pub tags: Option<String>,
    pub word_count: i32,
    pub pinned: bool,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

// ============================================================================
// Document CRUD
// ============================================================================

pub async fn create_document(
    pool: &SqlitePool,
    title: &str,
    content: &str,
    doc_type: &str,
    tags: Option<&str>,
    project: Option<&str>,
    source_url: Option<&str>,
    format: &str,
) -> Result<Document, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let word_count = content.split_whitespace().count() as i32;

    sqlx::query(
        r#"
        INSERT INTO documents (id, title, content, doc_type, tags, project, source_url, format, word_count, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
        "#,
    )
    .bind(&id)
    .bind(title)
    .bind(content)
    .bind(doc_type)
    .bind(tags)
    .bind(project)
    .bind(source_url)
    .bind(format)
    .bind(word_count)
    .bind(now)
    .execute(pool)
    .await?;

    // Update tag usage counts
    if let Some(tags_str) = tags {
        update_tag_counts(pool, tags_str).await?;
    }

    get_document(pool, &id).await
}

pub async fn get_document(pool: &SqlitePool, id: &str) -> Result<Document, sqlx::Error> {
    sqlx::query_as::<_, Document>("SELECT * FROM documents WHERE id = ?1")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn update_document(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    content: &str,
    doc_type: &str,
    tags: Option<&str>,
    project: Option<&str>,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().timestamp();
    let word_count = content.split_whitespace().count() as i32;

    sqlx::query(
        r#"
        UPDATE documents
        SET title = ?1, content = ?2, doc_type = ?3, tags = ?4, project = ?5,
            word_count = ?6, is_indexed = 0, updated_at = ?7
        WHERE id = ?8
        "#,
    )
    .bind(title)
    .bind(content)
    .bind(doc_type)
    .bind(tags)
    .bind(project)
    .bind(word_count)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_document(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM documents WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_documents(
    pool: &SqlitePool,
    limit: i64,
    doc_type: Option<&str>,
    tag: Option<&str>,
    project: Option<&str>,
    status: Option<&str>,
) -> Result<Vec<Document>, sqlx::Error> {
    let mut query = String::from(
        "SELECT * FROM documents WHERE 1=1"
    );
    let mut binds: Vec<String> = Vec::new();

    if let Some(dt) = doc_type {
        binds.push(dt.to_string());
        query.push_str(&format!(" AND doc_type = ?{}", binds.len()));
    }
    if let Some(t) = tag {
        binds.push(format!("%{}%", t));
        query.push_str(&format!(" AND tags LIKE ?{}", binds.len()));
    }
    if let Some(p) = project {
        binds.push(p.to_string());
        query.push_str(&format!(" AND project = ?{}", binds.len()));
    }
    if let Some(s) = status {
        binds.push(s.to_string());
        query.push_str(&format!(" AND status = ?{}", binds.len()));
    } else {
        binds.push("archived".to_string());
        query.push_str(&format!(" AND status != ?{}", binds.len()));
    }

    query.push_str(" ORDER BY pinned DESC, updated_at DESC");
    binds.push(limit.to_string());
    query.push_str(&format!(" LIMIT ?{}", binds.len()));

    // Build query dynamically - SQLx doesn't support dynamic bind counts easily,
    // so we use raw query with manual binding
    let mut q = sqlx::query_as::<_, Document>(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q.fetch_all(pool).await
}

pub async fn search_documents(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
) -> Result<Vec<Document>, sqlx::Error> {
    // Use FTS5 for full-text search
    sqlx::query_as::<_, Document>(
        r#"
        SELECT d.* FROM documents d
        JOIN documents_fts fts ON d.rowid = fts.rowid
        WHERE documents_fts MATCH ?1
        ORDER BY rank
        LIMIT ?2
        "#,
    )
    .bind(query)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn toggle_document_pin(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE documents SET pinned = CASE WHEN pinned = 1 THEN 0 ELSE 1 END WHERE id = ?1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_documents(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM documents WHERE status != 'archived'")
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

// ============================================================================
// Ideas CRUD
// ============================================================================

pub async fn create_idea(
    pool: &SqlitePool,
    content: &str,
    tags: Option<&str>,
    project: Option<&str>,
    category: Option<&str>,
    priority: i32,
) -> Result<Idea, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        r#"
        INSERT INTO ideas (id, content, tags, project, category, priority, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
        "#,
    )
    .bind(&id)
    .bind(content)
    .bind(tags)
    .bind(project)
    .bind(category)
    .bind(priority)
    .bind(now)
    .execute(pool)
    .await?;

    // Update tag usage
    if let Some(tags_str) = tags {
        update_tag_counts(pool, tags_str).await?;
    }

    get_idea(pool, &id).await
}

pub async fn get_idea(pool: &SqlitePool, id: &str) -> Result<Idea, sqlx::Error> {
    sqlx::query_as::<_, Idea>("SELECT * FROM ideas WHERE id = ?1")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn update_idea(
    pool: &SqlitePool,
    id: &str,
    content: &str,
    tags: Option<&str>,
    category: Option<&str>,
    priority: i32,
    status: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        r#"
        UPDATE ideas
        SET content = ?1, tags = ?2, category = ?3, priority = ?4, status = ?5, updated_at = ?6
        WHERE id = ?7
        "#,
    )
    .bind(content)
    .bind(tags)
    .bind(category)
    .bind(priority)
    .bind(status)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_idea_status(
    pool: &SqlitePool,
    id: &str,
    status: &str,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().timestamp();
    sqlx::query("UPDATE ideas SET status = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(status)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_idea(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM ideas WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_ideas(
    pool: &SqlitePool,
    limit: i64,
    status: Option<&str>,
    category: Option<&str>,
    tag: Option<&str>,
    project: Option<&str>,
) -> Result<Vec<Idea>, sqlx::Error> {
    let mut query = String::from("SELECT * FROM ideas WHERE 1=1");
    let mut binds: Vec<String> = Vec::new();

    if let Some(s) = status {
        binds.push(s.to_string());
        query.push_str(&format!(" AND status = ?{}", binds.len()));
    } else {
        binds.push("archived".to_string());
        query.push_str(&format!(" AND status != ?{}", binds.len()));
    }
    if let Some(c) = category {
        binds.push(c.to_string());
        query.push_str(&format!(" AND category = ?{}", binds.len()));
    }
    if let Some(t) = tag {
        binds.push(format!("%{}%", t));
        query.push_str(&format!(" AND tags LIKE ?{}", binds.len()));
    }
    if let Some(p) = project {
        binds.push(p.to_string());
        query.push_str(&format!(" AND project = ?{}", binds.len()));
    }

    query.push_str(" ORDER BY priority ASC, updated_at DESC");
    binds.push(limit.to_string());
    query.push_str(&format!(" LIMIT ?{}", binds.len()));

    let mut q = sqlx::query_as::<_, Idea>(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q.fetch_all(pool).await
}

pub async fn count_ideas(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ideas WHERE status != 'archived'")
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn count_ideas_by_status(pool: &SqlitePool) -> Result<Vec<(String, i64)>, sqlx::Error> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, COUNT(*) FROM ideas GROUP BY status ORDER BY COUNT(*) DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ============================================================================
// Tags Management
// ============================================================================

/// Ensure tags exist in the registry and update usage counts
pub async fn update_tag_counts(pool: &SqlitePool, tags_csv: &str) -> Result<(), sqlx::Error> {
    for tag in tags_csv.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let tag_lower = tag.to_lowercase();
        sqlx::query(
            r#"
            INSERT INTO tags (name, usage_count, created_at)
            VALUES (?1, 1, strftime('%s', 'now'))
            ON CONFLICT(name) DO UPDATE SET usage_count = usage_count + 1
            "#,
        )
        .bind(&tag_lower)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Get all tags ordered by usage
pub async fn list_tags(pool: &SqlitePool, limit: i64) -> Result<Vec<Tag>, sqlx::Error> {
    sqlx::query_as::<_, Tag>(
        "SELECT * FROM tags ORDER BY usage_count DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Search tags by prefix (for autocomplete)
pub async fn search_tags(pool: &SqlitePool, prefix: &str) -> Result<Vec<Tag>, sqlx::Error> {
    sqlx::query_as::<_, Tag>(
        "SELECT * FROM tags WHERE name LIKE ?1 ORDER BY usage_count DESC LIMIT 20",
    )
    .bind(format!("{}%", prefix.to_lowercase()))
    .fetch_all(pool)
    .await
}

/// Update a tag's color
pub async fn set_tag_color(
    pool: &SqlitePool,
    name: &str,
    color: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE tags SET color = ?1 WHERE name = ?2")
        .bind(color)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}
