//! API handlers for RAG system endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use std::time::Instant;

use super::types::*;
use crate::embeddings::EmbeddingGenerator;
use crate::indexing::IndexingConfig;
use crate::search::{SearchConfig, SearchFilters, SearchQuery, SemanticSearcher};
use sqlx::{Row, SqlitePool};

// ============================================================================
// Application State
// ============================================================================

#[derive(Clone)]
pub struct ApiState {
    pub db_pool: SqlitePool,
    pub embedding_generator: Arc<tokio::sync::Mutex<EmbeddingGenerator>>,
    pub searcher: Arc<SemanticSearcher>,
    pub job_queue: Arc<super::jobs::JobQueue>,
    pub start_time: std::time::SystemTime,
}

impl ApiState {
    pub async fn new(
        db_pool: SqlitePool,
        embedding_generator: Arc<tokio::sync::Mutex<EmbeddingGenerator>>,
        indexing_config: IndexingConfig,
        job_queue_config: super::jobs::JobQueueConfig,
    ) -> Self {
        let searcher = Arc::new(
            SemanticSearcher::new(SearchConfig::default())
                .await
                .expect("Failed to create semantic searcher"),
        );

        let job_queue = Arc::new(super::jobs::JobQueue::new(
            job_queue_config,
            db_pool.clone(),
            embedding_generator.clone(),
            indexing_config,
        ));

        Self {
            db_pool,
            embedding_generator,
            searcher,
            job_queue,
            start_time: std::time::SystemTime::now(),
        }
    }
}

// ============================================================================
// Health & Status
// ============================================================================

/// Health check endpoint
pub async fn health_check(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed().unwrap_or_default().as_secs();

    // Check database
    let db_healthy = sqlx::query("SELECT 1")
        .fetch_one(&state.db_pool)
        .await
        .is_ok();

    // Check embeddings (always true if we got here)
    let embeddings_healthy = true;

    let response = HealthResponse {
        status: if db_healthy && embeddings_healthy {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        services: ServiceHealth {
            database: db_healthy,
            embeddings: embeddings_healthy,
            search: db_healthy && embeddings_healthy,
        },
    };

    Json(ApiResponse::success(response))
}

/// Get system statistics
pub async fn get_stats(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    // Get document stats
    let total_docs = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM documents")
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    let indexed_docs =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM documents WHERE is_indexed = 1")
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);

    let pending_docs = total_docs - indexed_docs;

    // Get chunk stats
    let total_chunks = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM document_chunks")
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    let avg_chunks = if total_docs > 0 {
        total_chunks as f64 / total_docs as f64
    } else {
        0.0
    };

    // Get job stats
    let job_stats = state.job_queue.get_stats().await;

    let response = StatsResponse {
        documents: DocumentStats {
            total: total_docs,
            indexed: indexed_docs,
            pending: pending_docs,
            by_type: vec![], // TODO: Implement type counts
        },
        chunks: ChunkStats {
            total: total_chunks,
            avg_per_document: avg_chunks,
            avg_size: 0.0, // TODO: Calculate average size
        },
        search: SearchStats {
            total_searches: 0,
            avg_results: 0.0,
            avg_execution_time_ms: 0.0,
        },
        indexing: IndexingStats {
            jobs_queued: job_stats.queued as i64,
            jobs_processing: job_stats.processing as i64,
            jobs_completed: job_stats.completed as i64,
            jobs_failed: job_stats.failed as i64,
        },
    };

    Json(ApiResponse::success(response))
}

// ============================================================================
// Document Management
// ============================================================================

/// Upload a new document
pub async fn upload_document(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<UploadDocumentRequest>,
) -> impl IntoResponse {
    // Validate input
    if req.title.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                "Title cannot be empty".to_string(),
            )),
        )
            .into_response();
    }

    if req.content.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                "Content cannot be empty".to_string(),
            )),
        )
            .into_response();
    }

    // Insert document
    let doc_id = uuid::Uuid::new_v4().to_string();
    let tags_json = serde_json::to_string(&req.tags).unwrap_or_else(|_| "[]".to_string());

    let result: Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error> = sqlx::query!(
        r#"
        INSERT INTO documents (
            id, title, content, doc_type, tags,
            repo_id, source_type, source_url
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        doc_id,
        req.title,
        req.content,
        req.doc_type,
        tags_json,
        req.repo_id,
        req.source_type,
        req.source_url,
    )
    .execute(&state.db_pool)
    .await;

    match result {
        Ok(_result) => {
            // Queue for indexing
            let job_id = state
                .job_queue
                .submit_job(vec![doc_id.clone()], false)
                .await;

            let response = UploadDocumentResponse {
                id: doc_id,
                title: req.title,
                status: "queued_for_indexing".to_string(),
                created_at: chrono::Utc::now(),
                message: format!("Document uploaded successfully. Indexing job: {}", job_id),
            };

            (StatusCode::CREATED, Json(ApiResponse::success(response))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!(
                "Failed to upload document: {}",
                e
            ))),
        )
            .into_response(),
    }
}

/// Get document by ID
pub async fn get_document(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let result = sqlx::query!(
        r#"
        SELECT
            d.id, d.title, d.content, d.doc_type, d.tags,
            d.repo_id, d.source_type, d.source_url,
            d.indexed_at, d.created_at, d.updated_at,
            COALESCE(COUNT(c.id), 0) as "chunk_count!: i64"
        FROM documents d
        LEFT JOIN document_chunks c ON d.id = c.document_id
        WHERE d.id = ?
        GROUP BY d.id
        "#,
        id
    )
    .fetch_optional(&state.db_pool)
    .await;

    match result {
        Ok(Some(row)) => {
            let tags: Vec<String> = row
                .tags
                .as_ref()
                .and_then(|t| serde_json::from_str(t).ok())
                .unwrap_or_default();

            let response = DocumentResponse {
                id: row.id.unwrap_or_default(),
                title: row.title,
                content: row.content,
                doc_type: row.doc_type.unwrap_or_default(),
                tags,
                repo_id: None, // repo_id is String in DB but i64 in response - skip for now
                source_type: row.source_type,
                source_url: row.source_url,
                indexed_at: row
                    .indexed_at
                    .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
                created_at: chrono::DateTime::from_timestamp(row.created_at, 0)
                    .unwrap_or_else(|| chrono::Utc::now()),
                updated_at: chrono::DateTime::from_timestamp(row.updated_at, 0)
                    .unwrap_or_else(|| chrono::Utc::now()),
                chunk_count: row.chunk_count,
            };

            Json(ApiResponse::success(response)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Document not found".to_string())),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!(
                "Failed to fetch document: {}",
                e
            ))),
        )
            .into_response(),
    }
}

/// List documents with pagination
pub async fn list_documents(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<ListDocumentsQuery>,
) -> impl IntoResponse {
    let limit = params.pagination.limit.min(100);
    let offset = (params.pagination.page.saturating_sub(1)) * limit;

    // Build WHERE clause
    let mut where_clauses = Vec::new();
    let mut bind_values: Vec<Box<dyn sqlx::Encode<'_, sqlx::Sqlite> + Send>> = Vec::new();

    if let Some(doc_type) = params.doc_type {
        where_clauses.push("doc_type = ?");
    }

    if let Some(repo_id) = params.repo_id {
        where_clauses.push("repo_id = ?");
    }

    if params.indexed_only.unwrap_or(false) {
        where_clauses.push("is_indexed = 1");
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Get total count
    let count_query = format!("SELECT COUNT(*) FROM documents {}", where_sql);
    let total: i64 = sqlx::query_scalar(&count_query)
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    // Get documents - simplified query without dynamic WHERE clause
    let items: Vec<serde_json::Value> = vec![]; // Placeholder - implement with proper SQL

    // TODO: Implement proper document listing with filters
    // For now, return empty list to allow compilation

    let response = PaginatedResponse::new(items, total as u32, params.pagination.page, limit);

    Json(ApiResponse::success(response)).into_response()
}

/// Update document metadata
pub async fn update_document(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateDocumentRequest>,
) -> impl IntoResponse {
    let mut updates = Vec::new();

    if let Some(title) = req.title {
        updates.push(format!("title = '{}'", title));
    }

    if let Some(tags) = req.tags {
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
        updates.push(format!("tags = '{}'", tags_json));
    }

    if updates.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error("No fields to update".to_string())),
        )
            .into_response();
    }

    let query = format!(
        "UPDATE documents SET {}, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        updates.join(", ")
    );

    let result = sqlx::query(&query).bind(id).execute(&state.db_pool).await;

    match result {
        Ok(result) if result.rows_affected() > 0 => Json(ApiResponse::message(
            "Document updated successfully".to_string(),
        ))
        .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Document not found".to_string())),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!(
                "Failed to update document: {}",
                e
            ))),
        )
            .into_response(),
    }
}

/// Delete document
pub async fn delete_document(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Delete embeddings first
    let _ = sqlx::query!("DELETE FROM document_embeddings WHERE chunk_id IN (SELECT id FROM document_chunks WHERE document_id = ?)", id)
        .execute(&state.db_pool)
        .await;

    // Delete chunks
    let _ = sqlx::query!("DELETE FROM document_chunks WHERE document_id = ?", id)
        .execute(&state.db_pool)
        .await;

    // Delete document
    let result: Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error> =
        sqlx::query!("DELETE FROM documents WHERE id = ?", id)
            .execute(&state.db_pool)
            .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => Json(ApiResponse::message(
            "Document deleted successfully".to_string(),
        ))
        .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Document not found".to_string())),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!(
                "Failed to delete document: {}",
                e
            ))),
        )
            .into_response(),
    }
}

// ============================================================================
// Search
// ============================================================================

/// Search documents
pub async fn search_documents(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<SearchRequest>,
) -> impl IntoResponse {
    let start_time = Instant::now();

    // Build search filters
    let filters = SearchFilters {
        doc_type: req.filters.doc_type.clone(),
        tags: req.filters.tags.clone(),
        repo_id: req.filters.repo_id,
        source_type: req.filters.source_type.clone(),
        indexed_only: req.filters.indexed_only.unwrap_or(false),
        created_after: req.filters.date_from.map(|dt| dt.timestamp()),
        created_before: req.filters.date_to.map(|dt| dt.timestamp()),
    };

    // Build search query
    let query = SearchQuery {
        text: req.query.clone(),
        top_k: req.limit,
        filters,
    };

    // Perform search
    let results = state.searcher.search(&state.db_pool, &query).await;

    match results {
        Ok(search_results) => {
            let execution_time = start_time.elapsed().as_millis() as u64;

            let items: Vec<SearchResultItem> = search_results
                .iter()
                .map(|r| {
                    let tags: Vec<String> = vec![];

                    SearchResultItem {
                        document_id: r.document_id.parse().unwrap_or(0),
                        chunk_id: r.chunk_id.parse().unwrap_or(0),
                        title: format!("Document {}", r.document_id),
                        content: r.content.clone(),
                        doc_type: "document".to_string(),
                        score: r.score,
                        tags,
                        source_url: None,
                        created_at: chrono::Utc::now(),
                    }
                })
                .collect();

            let response = SearchResponse {
                results: items,
                total_results: search_results.len(),
                search_type: req.search_type,
                query: req.query,
                execution_time_ms: execution_time,
            };

            Json(ApiResponse::success(response)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(format!("Search failed: {}", e))),
        )
            .into_response(),
    }
}

// ============================================================================
// Indexing
// ============================================================================

/// Index a single document
pub async fn index_document(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<IndexDocumentRequest>,
) -> impl IntoResponse {
    let doc_id = req.document_id.clone();
    let job_id = state
        .job_queue
        .submit_job(vec![req.document_id], req.force_reindex)
        .await;

    let response = IndexJobResponse {
        job_id,
        document_ids: vec![doc_id],
        status: "queued".to_string(),
        queued_at: chrono::Utc::now(),
    };

    Json(ApiResponse::success(response)).into_response()
}

/// Batch index documents
pub async fn batch_index_documents(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<BatchIndexRequest>,
) -> impl IntoResponse {
    if req.document_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                "No document IDs provided".to_string(),
            )),
        )
            .into_response();
    }

    let job_id = state
        .job_queue
        .submit_job(req.document_ids.clone(), req.force_reindex)
        .await;

    let response = IndexJobResponse {
        job_id,
        document_ids: req.document_ids,
        status: "queued".to_string(),
        queued_at: chrono::Utc::now(),
    };

    (StatusCode::ACCEPTED, Json(ApiResponse::success(response))).into_response()
}

/// Get indexing job status
pub async fn get_index_job_status(
    State(state): State<Arc<ApiState>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    match state.job_queue.get_job(&job_id).await {
        Some(job) => {
            let response = IndexStatusResponse {
                job_id: job.id,
                status: match job.status {
                    super::jobs::JobStatus::Queued => IndexJobStatus::Queued,
                    super::jobs::JobStatus::Processing => IndexJobStatus::Processing,
                    super::jobs::JobStatus::Completed => IndexJobStatus::Completed,
                    super::jobs::JobStatus::Failed => IndexJobStatus::Failed,
                    super::jobs::JobStatus::Cancelled => IndexJobStatus::Failed,
                },
                documents_total: job.progress.total,
                documents_completed: job.progress.completed,
                documents_failed: job.progress.failed,
                started_at: job.started_at,
                completed_at: job.completed_at,
                error: job.error,
            };

            Json(ApiResponse::success(response)).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error("Job not found".to_string())),
        )
            .into_response(),
    }
}

/// List all indexing jobs
pub async fn list_index_jobs(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let jobs = state.job_queue.list_jobs().await;

    let job_summaries: Vec<_> = jobs
        .iter()
        .map(|job| {
            serde_json::json!({
                "job_id": job.id,
                "status": job.status,
                "documents_total": job.progress.total,
                "documents_completed": job.progress.completed,
                "created_at": job.created_at,
                "completed_at": job.completed_at,
            })
        })
        .collect();

    Json(ApiResponse::success(job_summaries)).into_response()
}

/// Cancel an indexing job
pub async fn cancel_index_job(
    State(state): State<Arc<ApiState>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    match state.job_queue.cancel_job(&job_id).await {
        Ok(_) => Json(ApiResponse::message(
            "Job cancelled successfully".to_string(),
        ))
        .into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ApiResponse::<()>::error(e))).into_response(),
    }
}
