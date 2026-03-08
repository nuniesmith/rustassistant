//! Rustassistant Server
//!
//! A clean REST API for notes, repositories, and tasks.
//! Includes integrated Web UI for repository and queue management.
//! Also mounts:
//!   /api/web/*  — pipeline dispatch, chat, SSE streaming, job history
//!   /api/v1/*   — repo CRUD + chat with repo-context injection

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
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

// Import from our crate
use rustassistant::api::repos::{repo_router, RepoAppState};
use rustassistant::auto_scanner::{AutoScanner, AutoScannerConfig};
use rustassistant::db::{
    self, get_next_task, get_stats, list_repositories, list_tasks, update_task_status, Database,
};
use rustassistant::git::GitManager;
use rustassistant::model_router::{ModelRouter, ModelRouterConfig};
use rustassistant::repo_sync::RepoSyncService;
use rustassistant::sync_scheduler::{SyncScheduler, SyncSchedulerConfig};
use rustassistant::web_api::{web_api_router, WebState};
use rustassistant::web_ui::{create_router as create_web_ui_router, WebAppState};
use rustassistant::web_ui_cache_viewer::create_cache_viewer_router;
use rustassistant::web_ui_db_explorer::create_db_explorer_router;
use rustassistant::web_ui_extensions::create_extension_router;
use rustassistant::web_ui_scan_progress::create_scan_progress_router;

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

    // Initialize database using init_db (CREATE TABLE IF NOT EXISTS — safe on existing DBs)
    info!("Initializing database at {}", database_url);
    let db = db::init_db(&database_url)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize database: {}", e))?;

    // Create app state for API
    let api_state = AppState { db: db.clone() };

    // ── Web-API pipeline state (pipeline dispatch, chat, SSE, jobs) ──────────
    let workspace = PathBuf::from(
        std::env::var("WORKSPACE_DIR").unwrap_or_else(|_| "data/workspaces".to_string()),
    );
    std::fs::create_dir_all(&workspace)?;

    let git_manager = Arc::new(
        GitManager::new(workspace.clone(), true)
            .map_err(|e| anyhow::anyhow!("GitManager init failed: {}", e))?,
    );
    let web_pipeline_state = WebState::new(db.clone(), git_manager, workspace);

    // ── RepoSyncService + ModelRouter + SyncScheduler ─────────────────────
    let sync_service = Arc::new(tokio::sync::RwLock::new(RepoSyncService::new()));
    let model_router = Arc::new(ModelRouter::new(ModelRouterConfig {
        remote_api_key: std::env::var("XAI_API_KEY").unwrap_or_default(),
        remote_model: std::env::var("REMOTE_MODEL").unwrap_or_else(|_| "grok-2-latest".to_string()),
        local_model: std::env::var("LOCAL_MODEL")
            .unwrap_or_else(|_| "qwen2.5-coder:7b".to_string()),
        local_base_url: std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string()),
        force_remote: std::env::var("FORCE_REMOTE_MODEL")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        fallback_to_remote: true,
    }));
    // Build GrokClient for the repo chat handler (None when XAI_API_KEY is unset).
    let grok_for_repo: Option<Arc<rustassistant::grok_client::GrokClient>> =
        match std::env::var("XAI_API_KEY") {
            Ok(api_key) if !api_key.is_empty() => {
                let db_path = database_url
                    .trim_start_matches("sqlite://")
                    .trim_start_matches("sqlite:");
                match db::Database::new(db_path).await {
                    Ok(grok_db) => {
                        let client = rustassistant::grok_client::GrokClient::new(api_key, grok_db);
                        info!("GrokClient ready for repo chat handler");
                        Some(Arc::new(client))
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "Failed to init DB for repo GrokClient — remote model unavailable"
                        );
                        None
                    }
                }
            }
            _ => {
                info!("XAI_API_KEY not set — repo chat will use local model only");
                None
            }
        };

    let repo_app_state = RepoAppState::from_env(
        Arc::clone(&sync_service),
        Arc::clone(&model_router),
        grok_for_repo,
    )
    .await;
    let sync_interval_secs = std::env::var("REPO_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(300);
    SyncScheduler::new(
        SyncSchedulerConfig {
            interval: Duration::from_secs(sync_interval_secs),
            ..SyncSchedulerConfig::default()
        },
        Arc::clone(&sync_service),
    )
    .start();
    info!(interval_secs = sync_interval_secs, "SyncScheduler started");

    // Create app state for Web UI
    let web_state = WebAppState::new(Database::from_pool(db.clone()), repos_dir.clone());

    // Build combined router
    let api_router = create_api_router(api_state);
    let web_router = create_web_ui_router(web_state.clone());
    let extension_router = create_extension_router(web_state.clone());
    let cache_viewer_router = create_cache_viewer_router(Arc::new(web_state.clone()));
    let db_explorer_router = create_db_explorer_router(Arc::new(web_state.clone()));
    let scan_progress_router = create_scan_progress_router(Arc::new(web_state.clone()));

    let app = Router::new()
        .merge(web_router)
        .merge(extension_router)
        .merge(cache_viewer_router)
        .merge(db_explorer_router)
        .merge(scan_progress_router)
        .merge(api_router)
        // Pipeline dispatch + chat + SSE + job history
        .merge(web_api_router(web_pipeline_state))
        // Repo CRUD + chat with repo context + /api/v1/repos/:id/sync etc.
        .nest("/api/v1", repo_router(repo_app_state));

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
        scan_cost_budget: std::env::var("AUTO_SCAN_COST_BUDGET")
            .unwrap_or_else(|_| "3.00".into())
            .parse()
            .unwrap_or(3.00),
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
