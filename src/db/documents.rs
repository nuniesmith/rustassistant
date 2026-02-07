//! Document database operations for RAG system
//!
//! Provides CRUD operations for documents, chunks, and embeddings.

use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{DbError, DbResult, Document, DocumentChunk, DocumentEmbedding};

// ============================================================================
// Document CRUD Operations
// ============================================================================

/// Create a new document
pub async fn create_document(
    pool: &SqlitePool,
    title: String,
    content: String,
    content_type: String,
    source_type: String,
    doc_type: String,
    repo_id: Option<String>,
    tags: Option<Vec<String>>,
) -> DbResult<Document> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    let word_count = content.split_whitespace().count() as i64;
    let char_count = content.chars().count() as i64;

    let tags_str = tags.as_ref().map(|t| t.join(","));

    // Insert document
    sqlx::query(
        "INSERT INTO documents
        (id, title, content, content_type, source_type, doc_type, tags, repo_id, word_count, char_count, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&title)
    .bind(&content)
    .bind(&content_type)
    .bind(&source_type)
    .bind(&doc_type)
    .bind(&tags_str)
    .bind(&repo_id)
    .bind(word_count)
    .bind(char_count)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(DbError::Sqlx)?;

    // If tags provided, insert into document_tags
    if let Some(tag_list) = tags {
        for tag in tag_list {
            // Ensure tag exists in tags table
            let _ = super::upsert_tag(pool, &tag, None, None).await;

            // Link to document
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO document_tags (document_id, tag, created_at) VALUES (?, ?, ?)"
            )
            .bind(&id)
            .bind(&tag)
            .bind(now)
            .execute(pool)
            .await;
        }
    }

    get_document(pool, &id).await
}

/// Get a document by ID
pub async fn get_document(pool: &SqlitePool, id: &str) -> DbResult<Document> {
    let row = sqlx::query!(
        "SELECT id, title, content, content_type, source_type, source_url, doc_type,
                tags, repo_id, file_path, word_count, char_count,
                created_at, updated_at, indexed_at
         FROM documents WHERE id = ?",
        id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => DbError::NotFound(format!("Document {} not found", id)),
        e => DbError::Sqlx(e),
    })?;

    Ok(Document {
        id: row.id.unwrap_or_default(),
        title: row.title,
        content: row.content,
        content_type: row.content_type.unwrap_or_else(|| "markdown".to_string()),
        source_type: row.source_type.unwrap_or_else(|| "manual".to_string()),
        source_url: row.source_url,
        doc_type: row.doc_type.unwrap_or_else(|| "reference".to_string()),
        tags: row.tags,
        repo_id: row.repo_id,
        file_path: row.file_path,
        word_count: row.word_count.unwrap_or(0),
        char_count: row.char_count.unwrap_or(0),
        created_at: row.created_at,
        updated_at: row.updated_at,
        indexed_at: row.indexed_at,
    })
}

/// List all documents with optional filters
pub async fn list_documents(
    pool: &SqlitePool,
    doc_type: Option<String>,
    repo_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> DbResult<Vec<Document>> {
    let mut query = String::from(
        "SELECT id, title, content, content_type, source_type, source_url, doc_type,
                tags, repo_id, file_path, word_count, char_count,
                created_at, updated_at, indexed_at
         FROM documents WHERE 1=1",
    );

    if doc_type.is_some() {
        query.push_str(" AND doc_type = ?");
    }
    if repo_id.is_some() {
        query.push_str(" AND repo_id = ?");
    }

    query.push_str(" ORDER BY updated_at DESC");

    if limit.is_some() {
        query.push_str(" LIMIT ?");
    }
    if offset.is_some() {
        query.push_str(" OFFSET ?");
    }

    let mut q = sqlx::query(&query);

    if let Some(dt) = doc_type {
        q = q.bind(dt);
    }
    if let Some(rid) = repo_id {
        q = q.bind(rid);
    }
    if let Some(lim) = limit {
        q = q.bind(lim);
    }
    if let Some(off) = offset {
        q = q.bind(off);
    }

    let rows = q.fetch_all(pool).await.map_err(DbError::Sqlx)?;

    Ok(rows
        .into_iter()
        .map(|row| Document {
            id: row.get("id"),
            title: row.get("title"),
            content: row.get("content"),
            content_type: row.get("content_type"),
            source_type: row.get("source_type"),
            source_url: row.get("source_url"),
            doc_type: row.get("doc_type"),
            tags: row.get("tags"),
            repo_id: row.get("repo_id"),
            file_path: row.get("file_path"),
            word_count: row.get("word_count"),
            char_count: row.get("char_count"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            indexed_at: row.get("indexed_at"),
        })
        .collect())
}

/// Update document content
pub async fn update_document(
    pool: &SqlitePool,
    id: &str,
    title: Option<String>,
    content: Option<String>,
    doc_type: Option<String>,
    tags: Option<Vec<String>>,
) -> DbResult<Document> {
    let now = chrono::Utc::now().timestamp();

    // Get current document
    let mut doc = get_document(pool, id).await?;

    // Update fields if provided
    if let Some(t) = title {
        doc.title = t;
    }
    if let Some(c) = content {
        doc.content = c.clone();
        doc.word_count = c.split_whitespace().count() as i64;
        doc.char_count = c.chars().count() as i64;
    }
    if let Some(dt) = doc_type {
        doc.doc_type = dt;
    }

    // Update in database
    sqlx::query(
        "UPDATE documents
         SET title = ?, content = ?, doc_type = ?, word_count = ?, char_count = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&doc.title)
    .bind(&doc.content)
    .bind(&doc.doc_type)
    .bind(doc.word_count)
    .bind(doc.char_count)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(DbError::Sqlx)?;

    // Update tags if provided
    if let Some(tag_list) = tags {
        // Remove old tags
        sqlx::query("DELETE FROM document_tags WHERE document_id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(DbError::Sqlx)?;

        // Add new tags
        for tag in tag_list {
            let _ = super::upsert_tag(pool, &tag, None, None).await;

            let _ = sqlx::query(
                "INSERT OR IGNORE INTO document_tags (document_id, tag, created_at) VALUES (?, ?, ?)"
            )
            .bind(id)
            .bind(&tag)
            .bind(now)
            .execute(pool)
            .await;
        }
    }

    get_document(pool, id).await
}

/// Delete a document and its chunks/embeddings
pub async fn delete_document(pool: &SqlitePool, id: &str) -> DbResult<()> {
    let rows = sqlx::query("DELETE FROM documents WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(DbError::Sqlx)?
        .rows_affected();

    if rows == 0 {
        Err(DbError::NotFound(format!("Document {} not found", id)))
    } else {
        Ok(())
    }
}

/// Search documents by title
pub async fn search_documents_by_title(
    pool: &SqlitePool,
    query: &str,
    limit: Option<i64>,
) -> DbResult<Vec<Document>> {
    let search_pattern = format!("%{}%", query);
    let limit = limit.unwrap_or(50);

    let rows = sqlx::query!(
        "SELECT id, title, content, content_type, source_type, source_url, doc_type,
                tags, repo_id, file_path, word_count, char_count,
                created_at, updated_at, indexed_at
         FROM documents
         WHERE title LIKE ? OR content LIKE ?
         ORDER BY updated_at DESC
         LIMIT ?",
        search_pattern,
        search_pattern,
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(DbError::Sqlx)?;

    Ok(rows
        .into_iter()
        .map(|row| Document {
            id: row.id.unwrap_or_default(),
            title: row.title,
            content: row.content,
            content_type: row.content_type.unwrap_or_else(|| "markdown".to_string()),
            source_type: row.source_type.unwrap_or_else(|| "manual".to_string()),
            source_url: row.source_url,
            doc_type: row.doc_type.unwrap_or_else(|| "reference".to_string()),
            tags: row.tags,
            repo_id: row.repo_id,
            file_path: row.file_path,
            word_count: row.word_count.unwrap_or(0),
            char_count: row.char_count.unwrap_or(0),
            created_at: row.created_at,
            updated_at: row.updated_at,
            indexed_at: row.indexed_at,
        })
        .collect())
}

/// Search documents by tags
pub async fn search_documents_by_tags(
    pool: &SqlitePool,
    tags: Vec<String>,
    limit: Option<i64>,
) -> DbResult<Vec<Document>> {
    let limit = limit.unwrap_or(50);

    // Build query to find documents with ANY of the tags
    let placeholders: Vec<String> = tags.iter().map(|_| "?".to_string()).collect();
    let query = format!(
        "SELECT DISTINCT d.id, d.title, d.content, d.content_type, d.source_type, d.source_url,
                d.doc_type, d.tags, d.repo_id, d.file_path, d.word_count, d.char_count,
                d.created_at, d.updated_at, d.indexed_at
         FROM documents d
         JOIN document_tags dt ON d.id = dt.document_id
         WHERE dt.tag IN ({})
         ORDER BY d.updated_at DESC
         LIMIT ?",
        placeholders.join(",")
    );

    let mut q = sqlx::query(&query);
    for tag in tags {
        q = q.bind(tag);
    }
    q = q.bind(limit);

    let rows = q.fetch_all(pool).await.map_err(DbError::Sqlx)?;

    Ok(rows
        .into_iter()
        .map(|row| Document {
            id: row.get("id"),
            title: row.get("title"),
            content: row.get("content"),
            content_type: row.get("content_type"),
            source_type: row.get("source_type"),
            source_url: row.get("source_url"),
            doc_type: row.get("doc_type"),
            tags: row.get("tags"),
            repo_id: row.get("repo_id"),
            file_path: row.get("file_path"),
            word_count: row.get("word_count"),
            char_count: row.get("char_count"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            indexed_at: row.get("indexed_at"),
        })
        .collect())
}

/// Get tags for a document
pub async fn get_document_tags(pool: &SqlitePool, document_id: &str) -> DbResult<Vec<String>> {
    let tags = sqlx::query_scalar::<_, String>(
        "SELECT tag FROM document_tags WHERE document_id = ? ORDER BY tag",
    )
    .bind(document_id)
    .fetch_all(pool)
    .await
    .map_err(DbError::Sqlx)?;

    Ok(tags)
}

/// Count documents
pub async fn count_documents(pool: &SqlitePool) -> DbResult<i64> {
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM documents")
        .fetch_one(pool)
        .await
        .map_err(DbError::Sqlx)?;

    Ok(count)
}

/// Count documents by type
pub async fn count_documents_by_type(pool: &SqlitePool, doc_type: &str) -> DbResult<i64> {
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM documents WHERE doc_type = ?")
        .bind(doc_type)
        .fetch_one(pool)
        .await
        .map_err(DbError::Sqlx)?;

    Ok(count)
}

// ============================================================================
// Document Chunk Operations
// ============================================================================

/// Create document chunks
pub async fn create_chunks(
    pool: &SqlitePool,
    document_id: String,
    chunks: Vec<(String, i64, i64, Option<String>)>, // (content, char_start, char_end, heading)
) -> DbResult<Vec<DocumentChunk>> {
    let now = chrono::Utc::now().timestamp();
    let mut created_chunks = Vec::new();

    for (index, (content, char_start, char_end, heading)) in chunks.into_iter().enumerate() {
        let id = Uuid::new_v4().to_string();
        let word_count = content.split_whitespace().count() as i64;

        sqlx::query(
            "INSERT INTO document_chunks
            (id, document_id, chunk_index, content, char_start, char_end, word_count, heading, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&document_id)
        .bind(index as i64)
        .bind(&content)
        .bind(char_start)
        .bind(char_end)
        .bind(word_count)
        .bind(&heading)
        .bind(now)
        .execute(pool)
        .await
        .map_err(DbError::Sqlx)?;

        created_chunks.push(DocumentChunk {
            id,
            document_id: document_id.clone(),
            chunk_index: index as i64,
            content,
            char_start,
            char_end,
            word_count,
            heading,
            created_at: now,
        });
    }

    Ok(created_chunks)
}

/// Get chunks for a document
pub async fn get_document_chunks(
    pool: &SqlitePool,
    document_id: &str,
) -> DbResult<Vec<DocumentChunk>> {
    let rows = sqlx::query!(
        "SELECT id, document_id, chunk_index, content, char_start, char_end,
                word_count, heading, created_at
         FROM document_chunks
         WHERE document_id = ?
         ORDER BY chunk_index",
        document_id
    )
    .fetch_all(pool)
    .await
    .map_err(DbError::Sqlx)?;

    Ok(rows
        .into_iter()
        .map(|row| DocumentChunk {
            id: row.id.unwrap_or_default(),
            document_id: row.document_id,
            chunk_index: row.chunk_index,
            content: row.content,
            char_start: row.char_start,
            char_end: row.char_end,
            word_count: row.word_count.unwrap_or(0),
            heading: row.heading,
            created_at: row.created_at,
        })
        .collect())
}

/// Delete chunks for a document
pub async fn delete_document_chunks(pool: &SqlitePool, document_id: &str) -> DbResult<()> {
    sqlx::query("DELETE FROM document_chunks WHERE document_id = ?")
        .bind(document_id)
        .execute(pool)
        .await
        .map_err(DbError::Sqlx)?;

    Ok(())
}

// ============================================================================
// Embedding Operations
// ============================================================================

/// Store embedding for a chunk
pub async fn store_embedding(
    pool: &SqlitePool,
    chunk_id: String,
    embedding: Vec<f32>,
    model: String,
) -> DbResult<DocumentEmbedding> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let embedding_json =
        serde_json::to_string(&embedding).map_err(|e| DbError::InvalidInput(e.to_string()))?;
    let dimension = embedding.len() as i64;

    sqlx::query(
        "INSERT OR REPLACE INTO document_embeddings
        (id, chunk_id, embedding, model, dimension, created_at)
        VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&chunk_id)
    .bind(&embedding_json)
    .bind(&model)
    .bind(dimension)
    .bind(now)
    .execute(pool)
    .await
    .map_err(DbError::Sqlx)?;

    Ok(DocumentEmbedding {
        id,
        chunk_id,
        embedding: embedding_json,
        model,
        dimension,
        created_at: now,
    })
}

/// Get embeddings for a document
pub async fn get_document_embeddings(
    pool: &SqlitePool,
    document_id: &str,
) -> DbResult<Vec<(DocumentChunk, DocumentEmbedding)>> {
    let rows = sqlx::query!(
        "SELECT
            c.id as chunk_id, c.document_id, c.chunk_index, c.content,
            c.char_start, c.char_end, c.word_count, c.heading, c.created_at as chunk_created_at,
            e.id as embedding_id, e.embedding, e.model, e.dimension, e.created_at as embedding_created_at
         FROM document_chunks c
         JOIN document_embeddings e ON c.id = e.chunk_id
         WHERE c.document_id = ?
         ORDER BY c.chunk_index",
        document_id
    )
    .fetch_all(pool)
    .await
    .map_err(DbError::Sqlx)?;

    let mut results = Vec::new();
    for row in rows {
        let chunk = DocumentChunk {
            id: row.chunk_id.clone().unwrap_or_default(),
            document_id: row.document_id,
            chunk_index: row.chunk_index,
            content: row.content,
            char_start: row.char_start,
            char_end: row.char_end,
            word_count: row.word_count.unwrap_or(0),
            heading: row.heading,
            created_at: row.chunk_created_at,
        };

        let embedding = DocumentEmbedding {
            id: row.embedding_id.unwrap_or_default(),
            chunk_id: row.chunk_id.unwrap_or_default(),
            embedding: row.embedding,
            model: row.model,
            dimension: row.dimension,
            created_at: row.embedding_created_at,
        };

        results.push((chunk, embedding));
    }

    Ok(results)
}

/// Get all embeddings (for search)
pub async fn get_all_embeddings(pool: &SqlitePool) -> DbResult<Vec<(String, DocumentEmbedding)>> {
    let rows = sqlx::query!(
        "SELECT e.id, e.chunk_id, e.embedding, e.model, e.dimension, e.created_at,
                c.document_id
         FROM document_embeddings e
         JOIN document_chunks c ON e.chunk_id = c.id"
    )
    .fetch_all(pool)
    .await
    .map_err(DbError::Sqlx)?;

    let mut results = Vec::new();
    for row in rows {
        let embedding = DocumentEmbedding {
            id: row.id.unwrap_or_default(),
            chunk_id: row.chunk_id,
            embedding: row.embedding,
            model: row.model,
            dimension: row.dimension,
            created_at: row.created_at,
        };
        results.push((row.document_id, embedding));
    }

    Ok(results)
}

/// Delete embeddings for a document
pub async fn delete_document_embeddings(pool: &SqlitePool, document_id: &str) -> DbResult<()> {
    sqlx::query(
        "DELETE FROM document_embeddings
         WHERE chunk_id IN (SELECT id FROM document_chunks WHERE document_id = ?)",
    )
    .bind(document_id)
    .execute(pool)
    .await
    .map_err(DbError::Sqlx)?;

    Ok(())
}

/// Mark document as indexed
pub async fn mark_document_indexed(pool: &SqlitePool, document_id: &str) -> DbResult<()> {
    let now = chrono::Utc::now().timestamp();

    sqlx::query("UPDATE documents SET indexed_at = ? WHERE id = ?")
        .bind(now)
        .bind(document_id)
        .execute(pool)
        .await
        .map_err(DbError::Sqlx)?;

    Ok(())
}

/// Get documents needing indexing
pub async fn get_unindexed_documents(pool: &SqlitePool, limit: i64) -> DbResult<Vec<Document>> {
    let rows = sqlx::query!(
        "SELECT id, title, content, content_type, source_type, source_url, doc_type,
                tags, repo_id, file_path, word_count, char_count,
                created_at, updated_at, indexed_at
         FROM documents
         WHERE indexed_at IS NULL OR updated_at > indexed_at
         ORDER BY updated_at DESC
         LIMIT ?",
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(DbError::Sqlx)?;

    Ok(rows
        .into_iter()
        .map(|row| Document {
            id: row.id.unwrap_or_default(),
            title: row.title,
            content: row.content,
            content_type: row.content_type.unwrap_or_else(|| "markdown".to_string()),
            source_type: row.source_type.unwrap_or_else(|| "manual".to_string()),
            source_url: row.source_url,
            doc_type: row.doc_type.unwrap_or_else(|| "reference".to_string()),
            tags: row.tags,
            repo_id: row.repo_id,
            file_path: row.file_path,
            word_count: row.word_count.unwrap_or(0),
            char_count: row.char_count.unwrap_or(0),
            created_at: row.created_at,
            updated_at: row.updated_at,
            indexed_at: row.indexed_at,
        })
        .collect())
}
