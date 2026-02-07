//! Rustassistant Server
//!
//! A clean REST API for notes, repositories, and tasks.
//! Includes integrated Web UI for repository and queue management.

use axum::response::Html;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

// Import from our crate
use rustassistant::auto_scanner::{AutoScanner, AutoScannerConfig};
use rustassistant::db::{
    self, create_note, get_next_task, get_stats, list_notes, list_repositories, list_tasks,
    search_notes, update_note_status, update_task_status, Database,
};
use rustassistant::web_ui::{create_router as create_web_ui_router, WebAppState};
use std::sync::Arc;

// ============================================================================
// Application State
// ============================================================================

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct CreateNoteRequest {
    content: String,
    tags: Option<String>,
    project: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListNotesQuery {
    limit: Option<i64>,
    status: Option<String>,
    project: Option<String>,
    tag: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UpdateStatusRequest {
    status: String,
}

#[derive(Debug, Deserialize)]
struct AddRepoRequest {
    path: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListTasksQuery {
    limit: Option<i64>,
    status: Option<String>,
    priority: Option<i32>,
    repo_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Json<Self> {
        Json(Self {
            success: true,
            data: Some(data),
            error: None,
        })
    }
}

impl ApiResponse<()> {
    fn error(msg: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::BAD_REQUEST,
            Json(Self {
                success: false,
                data: None,
                error: Some(msg.into()),
            }),
        )
    }

    fn not_found(msg: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::NOT_FOUND,
            Json(Self {
                success: false,
                data: None,
                error: Some(msg.into()),
            }),
        )
    }
}

// ============================================================================
// Handlers
// ============================================================================

// Health check
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "rustassistant",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

// Root page - Simple status page
#[allow(dead_code)]
async fn root_handler() -> impl IntoResponse {
    Html(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>RustAssistant</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 800px;
            margin: 50px auto;
            padding: 20px;
            line-height: 1.6;
        }
        h1 { color: #2563eb; }
        .status {
            background: #10b981;
            color: white;
            padding: 8px 16px;
            border-radius: 4px;
            display: inline-block;
        }
        .endpoints {
            background: #f3f4f6;
            padding: 20px;
            border-radius: 8px;
            margin-top: 20px;
        }
        code {
            background: #e5e7eb;
            padding: 2px 6px;
            border-radius: 3px;
            font-family: 'Courier New', monospace;
        }
        .endpoint {
            margin: 10px 0;
        }
    </style>
</head>
<body>
    <h1>🦀 RustAssistant API</h1>
    <div class="status">✓ Online</div>

    <p>Welcome to the RustAssistant REST API server.</p>

    <div class="endpoints">
        <h2>Available Endpoints</h2>

        <div class="endpoint">
            <strong>GET</strong> <code>/health</code> - Health check
        </div>
        <div class="endpoint">
            <strong>GET</strong> <code>/api/stats</code> - Get statistics
        </div>

        <h3>Notes</h3>
        <div class="endpoint">
            <strong>POST</strong> <code>/api/notes</code> - Create note
        </div>
        <div class="endpoint">
            <strong>GET</strong> <code>/api/notes</code> - List notes
        </div>
        <div class="endpoint">
            <strong>GET</strong> <code>/api/notes/search?q=query</code> - Search notes
        </div>
        <div class="endpoint">
            <strong>GET</strong> <code>/api/notes/:id</code> - Get note
        </div>
        <div class="endpoint">
            <strong>PUT</strong> <code>/api/notes/:id</code> - Update note
        </div>
        <div class="endpoint">
            <strong>DELETE</strong> <code>/api/notes/:id</code> - Delete note
        </div>

        <h3>Repositories</h3>
        <div class="endpoint">
            <strong>POST</strong> <code>/api/repos</code> - Add repository
        </div>
        <div class="endpoint">
            <strong>GET</strong> <code>/api/repos</code> - List repositories
        </div>
        <div class="endpoint">
            <strong>GET</strong> <code>/api/repos/:id</code> - Get repository
        </div>
        <div class="endpoint">
            <strong>DELETE</strong> <code>/api/repos/:id</code> - Delete repository
        </div>

        <h3>Tasks</h3>
        <div class="endpoint">
            <strong>GET</strong> <code>/api/tasks</code> - List tasks
        </div>
        <div class="endpoint">
            <strong>GET</strong> <code>/api/tasks/next</code> - Get next task
        </div>
        <div class="endpoint">
            <strong>PUT</strong> <code>/api/tasks/:id</code> - Update task
        </div>
    </div>

    <p style="margin-top: 30px; color: #6b7280;">
        <strong>Note:</strong> The web UI is currently being updated for the new schema.
        Use the REST API endpoints above or the CLI tool for now.
    </p>
</body>
</html>"#,
    )
}

// Stats
async fn get_statistics(State(state): State<AppState>) -> impl IntoResponse {
    match get_stats(&state.db).await {
        Ok(stats) => ApiResponse::ok(stats).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

// --- Notes ---

async fn create_note_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateNoteRequest>,
) -> impl IntoResponse {
    match create_note(
        &state.db,
        &req.content,
        req.tags.as_deref(),
        req.project.as_deref(),
    )
    .await
    {
        Ok(note) => (StatusCode::CREATED, ApiResponse::ok(note)).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

async fn list_notes_handler(
    State(state): State<AppState>,
    Query(query): Query<ListNotesQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50);
    match list_notes(
        &state.db,
        limit,
        query.status.as_deref(),
        query.project.as_deref(),
        query.tag.as_deref(),
    )
    .await
    {
        Ok(notes) => ApiResponse::ok(notes).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

async fn search_notes_handler(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(20);
    match search_notes(&state.db, &query.q, limit).await {
        Ok(notes) => ApiResponse::ok(notes).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

async fn get_note_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match db::get_note(&state.db, &id).await {
        Ok(note) => (StatusCode::OK, ApiResponse::ok(note)).into_response(),
        Err(db::DbError::NotFound(msg)) => ApiResponse::not_found(msg).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

async fn update_note_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateStatusRequest>,
) -> impl IntoResponse {
    match update_note_status(&state.db, &id, &req.status).await {
        Ok(()) => ApiResponse::ok(serde_json::json!({"updated": true})).into_response(),
        Err(db::DbError::NotFound(msg)) => ApiResponse::not_found(msg).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

async fn delete_note_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match db::delete_note(&state.db, &id).await {
        Ok(()) => (
            StatusCode::OK,
            ApiResponse::ok(serde_json::json!({"deleted": true})),
        )
            .into_response(),
        Err(db::DbError::NotFound(msg)) => ApiResponse::not_found(msg).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

// --- Repositories ---

async fn add_repo_handler(
    State(state): State<AppState>,
    Json(req): Json<AddRepoRequest>,
) -> impl IntoResponse {
    // Derive name from path if not provided
    let name = req.name.unwrap_or_else(|| {
        std::path::Path::new(&req.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string()
    });

    match db::add_repository(&state.db, &req.path, &name, None).await {
        Ok(repo) => (StatusCode::CREATED, ApiResponse::ok(repo)).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

async fn list_repos_handler(State(state): State<AppState>) -> impl IntoResponse {
    match list_repositories(&state.db).await {
        Ok(repos) => ApiResponse::ok(repos).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

async fn get_repo_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match db::get_repository(&state.db, &id).await {
        Ok(repo) => (StatusCode::OK, ApiResponse::ok(repo)).into_response(),
        Err(db::DbError::NotFound(msg)) => ApiResponse::not_found(msg).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

async fn delete_repo_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match db::remove_repository(&state.db, &id).await {
        Ok(()) => (
            StatusCode::OK,
            ApiResponse::ok(serde_json::json!({"deleted": true})),
        )
            .into_response(),
        Err(db::DbError::NotFound(msg)) => ApiResponse::not_found(msg).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

// --- Tasks ---

async fn list_tasks_handler(
    State(state): State<AppState>,
    Query(query): Query<ListTasksQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50);
    match list_tasks(
        &state.db,
        limit,
        query.status.as_deref(),
        query.priority,
        query.repo_id.as_deref(),
    )
    .await
    {
        Ok(tasks) => ApiResponse::ok(tasks).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

async fn get_next_task_handler(State(state): State<AppState>) -> impl IntoResponse {
    match get_next_task(&state.db).await {
        Ok(Some(task)) => ApiResponse::ok(task).into_response(),
        Ok(None) => {
            ApiResponse::ok(serde_json::json!({"message": "No pending tasks"})).into_response()
        }
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

async fn update_task_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateStatusRequest>,
) -> impl IntoResponse {
    match update_task_status(&state.db, &id, &req.status).await {
        Ok(()) => ApiResponse::ok(serde_json::json!({"updated": true})).into_response(),
        Err(db::DbError::NotFound(msg)) => ApiResponse::not_found(msg).into_response(),
        Err(e) => ApiResponse::error(e.to_string()).into_response(),
    }
}

// ============================================================================
// Router
// ============================================================================

fn create_api_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check (root kept minimal for API)
        .route("/health", get(health_check))
        .route("/api/stats", get(get_statistics))
        // Notes
        // Notes routes moved to web_ui module to avoid conflicts
        // Repositories
        .route("/api/repos", post(add_repo_handler))
        .route("/api/repos", get(list_repos_handler))
        .route("/api/repos/:id", get(get_repo_handler))
        .route("/api/repos/:id", delete(delete_repo_handler))
        // Tasks
        .route("/api/tasks", get(list_tasks_handler))
        .route("/api/tasks/next", get(get_next_task_handler))
        .route("/api/tasks/:id", put(update_task_handler))
        .layer(cors)
        .with_state(state)
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,rustassistant=debug".into()),
        )
        .init();

    // Get configuration
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/rustassistant.db".into());
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let repos_dir = std::env::var("REPOS_DIR").unwrap_or_else(|_| "/app/repos".into());
    let addr = format!("{}:{}", host, port);

    // Ensure repos directory exists
    std::fs::create_dir_all(&repos_dir).expect("Failed to create repos directory");

    // Initialize database with migrations
    info!("Initializing database at {}", database_url);
    let mut config = db::DatabaseConfig::from_env();
    // Override path from DATABASE_URL if it's a sqlite URL
    if database_url.starts_with("sqlite:") {
        config.path = std::path::PathBuf::from(database_url.trim_start_matches("sqlite:"));
    }
    let db = db::init_pool(&config).await?;

    // Create app state for API
    let api_state = AppState { db: db.clone() };

    // Create app state for Web UI
    let web_state = WebAppState::new(Database::from_pool(db.clone()), repos_dir.clone());

    // Build combined router with Web UI at root and API at /api
    let api_router = create_api_router(api_state);
    let web_router = create_web_ui_router(web_state);

    // Merge routers: Web UI gets root paths, API gets /api/* and /health
    let app = Router::new().merge(web_router).merge(api_router);

    // Start auto-scanner in background if enabled
    let scanner_config = AutoScannerConfig {
        enabled: std::env::var("AUTO_SCAN_ENABLED")
            .unwrap_or_else(|_| "true".into())
            .parse()
            .unwrap_or(true),
        default_interval_minutes: std::env::var("AUTO_SCAN_INTERVAL")
            .unwrap_or_else(|_| "60".into())
            .parse()
            .unwrap_or(60),
        max_concurrent_scans: std::env::var("AUTO_SCAN_MAX_CONCURRENT")
            .unwrap_or_else(|_| "2".into())
            .parse()
            .unwrap_or(2),
    };

    if scanner_config.enabled {
        info!(
            "🔍 Starting auto-scanner (interval: {} minutes)",
            scanner_config.default_interval_minutes
        );
        let scanner = Arc::new(AutoScanner::new(
            scanner_config,
            db.clone(),
            std::path::PathBuf::from(&repos_dir),
        ));
        let scanner_clone = scanner.clone();
        tokio::spawn(async move {
            if let Err(e) = scanner_clone.start().await {
                tracing::error!("Auto-scanner error: {}", e);
            }
        });
    } else {
        info!("Auto-scanner is disabled");
    }

    // Start server
    info!("🚀 Rustassistant server starting on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
