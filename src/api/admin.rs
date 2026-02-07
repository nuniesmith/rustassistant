//! Admin API Module
//!
//! Provides administrative endpoints for the dashboard.
//! Includes metrics, analytics, webhook management, API key management, and system health.

use crate::api::{ApiResponse, ApiState, AuthResult};
use crate::query_analytics::QueryAnalytics;
use crate::webhooks::{WebhookConfig, WebhookEndpoint, WebhookEvent};
use anyhow::{Context, Result};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Admin Router
// ============================================================================

/// Create admin router
pub fn admin_router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/admin/stats", get(admin_stats))
        .route("/admin/health", get(health_check))
        .route("/admin/webhooks", get(list_webhooks).post(create_webhook))
        .route("/admin/webhooks/:id", delete(delete_webhook))
        .route("/admin/webhooks/:id/test", post(test_webhook))
        .route("/admin/api-keys", get(list_api_keys).post(create_api_key))
        .route("/admin/api-keys/:id", delete(revoke_api_key))
        .route("/admin/jobs", get(list_jobs))
        .route("/admin/jobs/:id/retry", post(retry_job))
        .route("/admin/analytics/popular", get(popular_queries))
        .route("/admin/analytics/trending", get(trending_queries))
        .route("/admin/analytics/stats", get(analytics_stats))
        .route("/admin/analytics/timeseries", get(timeseries_data))
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct AdminStats {
    pub total_documents: i64,
    pub total_searches: i64,
    pub active_jobs: i64,
    pub pending_jobs: i64,
    pub cache_hit_rate: f64,
    pub total_chunks: i64,
    pub indexed_documents: i64,
}

#[derive(Debug, Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub database: bool,
    pub cache: bool,
    pub vector_index: bool,
    pub webhooks: bool,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub url: String,
    pub secret: String,
    pub events: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub id: String,
    pub url: String,
    pub events: Vec<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub last_delivery: Option<DateTime<Utc>>,
    pub success_rate: f64,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub key: Option<String>, // Only returned on creation
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub request_count: i64,
}

#[derive(Debug, Serialize)]
pub struct JobResponse {
    pub id: String,
    pub document_id: String,
    pub status: String,
    pub progress: f64,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
    pub chunks_processed: i64,
}

#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    pub days: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TimeSeriesQuery {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub interval_hours: Option<i64>,
}

// ============================================================================
// Admin Endpoints
// ============================================================================

/// Get admin statistics
async fn admin_stats(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
) -> Result<impl IntoResponse, StatusCode> {
    // Get document stats
    let total_documents = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM documents")
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    let indexed_documents =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM documents WHERE indexed = 1")
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);

    let total_chunks = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM chunks")
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    // Get job stats
    let active_jobs = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM indexing_jobs WHERE status = 'processing'",
    )
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

    let pending_jobs =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM indexing_jobs WHERE status = 'pending'")
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);

    // Get cache stats
    let cache_stats = state.cache_layer.as_ref().map(|c| c.stats());
    let cache_hit_rate = if let Some(stats) = cache_stats.await {
        stats.hit_rate() * 100.0
    } else {
        0.0
    };

    // Get search count (requires analytics)
    let total_searches = if let Some(analytics) = &state.analytics {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM search_analytics")
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0)
    } else {
        0
    };

    let stats = AdminStats {
        total_documents,
        total_searches,
        active_jobs,
        pending_jobs,
        cache_hit_rate,
        total_chunks,
        indexed_documents,
    };

    Ok(Json(ApiResponse::success(stats)))
}

/// Health check
async fn health_check(State(state): State<Arc<ApiState>>) -> Result<impl IntoResponse, StatusCode> {
    // Check database
    let db_ok = sqlx::query("SELECT 1")
        .execute(&state.db_pool)
        .await
        .is_ok();

    // Check cache
    let cache_ok = state.cache_layer.as_ref().map(|c| true).unwrap_or(false);

    // Check vector index
    let index_ok = state.vector_index.is_some();

    // Check webhooks
    let webhooks_ok = state.webhook_manager.is_some();

    let status = if db_ok && cache_ok {
        "healthy"
    } else {
        "degraded"
    };

    let health = HealthStatus {
        status: status.to_string(),
        database: db_ok,
        cache: cache_ok,
        vector_index: index_ok,
        webhooks: webhooks_ok,
        timestamp: Utc::now(),
    };

    Ok(Json(ApiResponse::success(health)))
}

/// List webhooks
async fn list_webhooks(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
) -> Result<impl IntoResponse, StatusCode> {
    let manager = state
        .webhook_manager
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let webhooks = manager
        .list_endpoints()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let responses: Vec<WebhookResponse> = webhooks
        .into_iter()
        .map(|w| WebhookResponse {
            id: w.id,
            url: w.url,
            events: w.events.iter().map(|e| format!("{:?}", e)).collect(),
            enabled: w.enabled,
            created_at: w.created_at,
            last_delivery: w.last_delivery,
            success_rate: (w.delivery_count as f64 / w.delivery_count.max(1) as f64) * 100.0,
        })
        .collect();

    Ok(Json(ApiResponse::success(responses)))
}

/// Create webhook
async fn create_webhook(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
    Json(req): Json<CreateWebhookRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let manager = state
        .webhook_manager
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // Parse events
    let events: Vec<WebhookEvent> = req
        .events
        .iter()
        .filter_map(|e| match e.as_str() {
            "document.uploaded" => Some(WebhookEvent::DocumentUploaded {
                document_id: String::new(),
                title: String::new(),
            }),
            "document.indexed" => Some(WebhookEvent::DocumentIndexed {
                document_id: String::new(),
                chunks: 0,
                duration_ms: 0,
            }),
            "document.failed" => Some(WebhookEvent::DocumentIndexingFailed {
                document_id: String::new(),
                error: String::new(),
            }),
            "search.performed" => Some(WebhookEvent::SearchPerformed {
                query: String::new(),
                results: 0,
                duration_ms: 0,
            }),
            _ => None,
        })
        .collect();

    let endpoint = WebhookEndpoint {
        id: uuid::Uuid::new_v4().to_string(),
        url: req.url.clone(),
        secret: req.secret,
        events: events.clone(),
        enabled: true,
        created_at: Utc::now(),
        last_delivery: None,
        delivery_count: 0,
    };

    manager
        .register_endpoint(endpoint.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = WebhookResponse {
        id: endpoint.id,
        url: endpoint.url,
        events: req.events,
        enabled: endpoint.enabled,
        created_at: endpoint.created_at,
        last_delivery: None,
        success_rate: 0.0,
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Delete webhook
async fn delete_webhook(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let manager = state
        .webhook_manager
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    manager
        .unregister_endpoint(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::<()>::success_message("Webhook deleted")))
}

/// Test webhook
async fn test_webhook(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let manager = state
        .webhook_manager
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // Send test event
    let event = WebhookEvent::SearchPerformed {
        query: "test query".to_string(),
        results: 0,
        duration_ms: 0,
    };

    manager
        .send_event(&event)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::<()>::success_message("Test sent")))
}

/// List API keys
async fn list_api_keys(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
) -> Result<impl IntoResponse, StatusCode> {
    // Query API keys from database (you'd need to create this table)
    let keys = sqlx::query_as::<_, (String, String, String, String, Option<String>, i64)>(
        "SELECT id, name, key_prefix, created_at, last_used, request_count FROM api_keys",
    )
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    let responses: Vec<ApiKeyResponse> = keys
        .into_iter()
        .map(
            |(id, name, prefix, created, last_used, count)| ApiKeyResponse {
                id,
                name,
                prefix,
                key: None,
                created_at: created.parse().unwrap_or(Utc::now()),
                last_used: last_used.and_then(|d| d.parse().ok()),
                request_count: count,
            },
        )
        .collect();

    Ok(Json(ApiResponse::success(responses)))
}

/// Create API key
async fn create_api_key(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let api_key = crate::api::generate_api_key();
    let key_hash = crate::api::hash_api_key(&api_key);
    let prefix = &api_key[..8];

    let id = uuid::Uuid::new_v4().to_string();
    let created_at = Utc::now();

    // Store in database
    sqlx::query(
        "INSERT INTO api_keys (id, name, key_hash, key_prefix, created_at, request_count) VALUES (?, ?, ?, ?, ?, 0)",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&key_hash)
    .bind(prefix)
    .bind(created_at.to_rfc3339())
    .execute(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = ApiKeyResponse {
        id,
        name: req.name,
        prefix: prefix.to_string(),
        key: Some(api_key),
        created_at,
        last_used: None,
        request_count: 0,
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Revoke API key
async fn revoke_api_key(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    sqlx::query("DELETE FROM api_keys WHERE id = ?")
        .bind(&id)
        .execute(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::<()>::success_message("API key revoked")))
}

/// List indexing jobs
async fn list_jobs(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
) -> Result<impl IntoResponse, StatusCode> {
    let jobs = sqlx::query_as::<_, (String, String, String, f64, Option<String>, Option<String>, Option<String>, i64)>(
        "SELECT id, document_id, status, progress, started_at, completed_at, error, chunks_processed
         FROM indexing_jobs
         ORDER BY created_at DESC
         LIMIT 50",
    )
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    let responses: Vec<JobResponse> = jobs
        .into_iter()
        .map(
            |(id, doc_id, status, progress, started, completed, error, chunks)| {
                let started_dt: Option<DateTime<Utc>> = started.and_then(|s| s.parse().ok());
                let completed_dt: Option<DateTime<Utc>> = completed.and_then(|c| c.parse().ok());

                let duration_ms = if let (Some(s), Some(c)) = (started_dt, completed_dt) {
                    Some((c - s).num_milliseconds())
                } else {
                    None
                };

                JobResponse {
                    id,
                    document_id: doc_id,
                    status,
                    progress,
                    started_at: started_dt,
                    completed_at: completed_dt,
                    duration_ms,
                    error,
                    chunks_processed: chunks,
                }
            },
        )
        .collect();

    Ok(Json(ApiResponse::success(responses)))
}

/// Retry failed job
async fn retry_job(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    // Reset job status to pending
    sqlx::query("UPDATE indexing_jobs SET status = 'pending', error = NULL WHERE id = ?")
        .bind(&id)
        .execute(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::<()>::success_message("Job queued")))
}

/// Get popular queries
async fn popular_queries(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
    Query(params): Query<AnalyticsQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let analytics = state
        .analytics
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let limit = params.limit.unwrap_or(10);
    let queries = analytics
        .get_popular_queries(limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::success(queries)))
}

/// Get trending queries
async fn trending_queries(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
    Query(params): Query<AnalyticsQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let analytics = state
        .analytics
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let limit = params.limit.unwrap_or(10);
    let queries = analytics
        .get_trending_queries(limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::success(queries)))
}

/// Get analytics statistics
async fn analytics_stats(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
    Query(params): Query<AnalyticsQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let analytics = state
        .analytics
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let days = params.days.unwrap_or(30);
    let stats = analytics
        .get_stats(days)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::success(stats)))
}

/// Get time series data
async fn timeseries_data(
    State(state): State<Arc<ApiState>>,
    _auth: AuthResult,
    Query(params): Query<TimeSeriesQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let analytics = state
        .analytics
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let end = params.end.unwrap_or_else(Utc::now);
    let start = params.start.unwrap_or_else(|| end - Duration::days(7));
    let interval = params.interval_hours.unwrap_or(24);

    let series = analytics
        .get_time_series(start, end, interval)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::success(series)))
}
