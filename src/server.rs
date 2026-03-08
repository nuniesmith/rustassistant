//! Axum API server for the audit service + RustAssistant dashboard

use crate::api::repos::{repo_router, RepoAppState};
use crate::config::Config;
use crate::db::Database;
use crate::db::{self, init_db, Repository};
use crate::error::{AuditError, Result};
use crate::git::GitManager;
use crate::llm::LlmClient;
use crate::model_router::{ModelRouter, ModelRouterConfig};
use crate::queue::{get_queue_stats, QueueStats};
use crate::repo_sync::RepoSyncService;
use crate::scanner::github::sync_repos_to_db;
use crate::sync_scheduler::{SyncScheduler, SyncSchedulerConfig};
use crate::web_api::{web_api_router, WebState};
// Neuromorphic mapper removed - feature not currently implemented

use crate::scanner::Scanner;
use crate::tags::TagScanner;
use crate::tasks::TaskGenerator;
use crate::types::{AuditReport, AuditRequest, AuditTag, Task};
use axum::{
    extract::{Json, Path, Query, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    config: Arc<Config>,
    pub(crate) git_manager: Arc<GitManager>,
    llm_client: Option<Arc<LlmClient>>,
    pub(crate) db_pool: SqlitePool,
}

impl AppState {
    /// Create new application state
    pub async fn new(config: Config) -> Result<Self> {
        let git_manager = Arc::new(GitManager::new(
            config.git.workspace_dir.clone(),
            config.git.shallow_clone,
        )?);

        let llm_client = if config.llm.enabled {
            if let Some(api_key) = &config.llm.api_key {
                let client = LlmClient::new_with_provider(
                    api_key.clone(),
                    config.llm.provider.clone(),
                    config.llm.model.clone(),
                    config.llm.max_tokens,
                    config.llm.temperature,
                )?;
                Some(Arc::new(client))
            } else {
                return Err(AuditError::config("LLM enabled but no API key provided"));
            }
        } else {
            None
        };

        // Initialize database
        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/rustassistant.db".into());
        let db_pool = db::init_db(&database_url)
            .await
            .map_err(|e| AuditError::other(format!("Failed to initialize database: {}", e)))?;

        Ok(Self {
            config: Arc::new(config),
            git_manager,
            llm_client,
            db_pool,
        })
    }
}

/// Run the audit server
pub async fn run_server(config: Config) -> Result<()> {
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let socket_addr: SocketAddr = addr
        .parse()
        .map_err(|e| AuditError::config(format!("Invalid server address: {}", e)))?;

    info!("Starting RustAssistant server on {}", socket_addr);

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create application state
    let state = AppState::new(config.clone()).await?;

    // ------------------------------------------------------------------
    // Build the RustAssistant dashboard web state
    // ------------------------------------------------------------------
    let workspace = PathBuf::from(
        std::env::var("WORKSPACE_DIR").unwrap_or_else(|_| "data/workspaces".to_string()),
    );
    std::fs::create_dir_all(&workspace)
        .map_err(|e| AuditError::other(format!("Cannot create workspace dir: {}", e)))?;

    let web_state = WebState::new(state.db_pool.clone(), state.git_manager.clone(), workspace);

    // ------------------------------------------------------------------
    // Build RepoSyncService + ModelRouter + SyncScheduler
    // ------------------------------------------------------------------

    // Re-use the same SQLite pool that the rest of the app already has.
    // This ensures registered_repos rows land in the same database file.
    let sync_repo_service = {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:data/rustassistant.db".to_string());
        match init_db(&db_url).await {
            Ok(pool) => {
                let mut svc = RepoSyncService::with_db(pool);
                match svc.load_from_db().await {
                    Ok(n) => info!(count = n, "Loaded persisted repos from SQLite"),
                    Err(e) => {
                        warn!(error = %e, "Failed to load repos from SQLite — starting empty")
                    }
                }
                svc
            }
            Err(e) => {
                warn!(error = %e, "Could not open DB for RepoSyncService — using in-memory only");
                RepoSyncService::new()
            }
        }
    };

    let sync_service = Arc::new(tokio::sync::RwLock::new(sync_repo_service));

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
    let grok_for_repo: Option<Arc<crate::grok_client::GrokClient>> = match std::env::var(
        "XAI_API_KEY",
    ) {
        Ok(api_key) if !api_key.is_empty() => {
            let db_url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "data/rustassistant.db".to_string());
            let db_path = db_url
                .trim_start_matches("sqlite://")
                .trim_start_matches("sqlite:");
            match Database::new(db_path).await {
                Ok(db) => {
                    let client = crate::grok_client::GrokClient::new(api_key, db);
                    info!("GrokClient ready for repo chat handler");
                    Some(Arc::new(client))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to init DB for repo GrokClient — remote model unavailable");
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

    // Start background sync scheduler
    let sync_interval_secs = std::env::var("REPO_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(300);
    SyncScheduler::new(
        SyncSchedulerConfig {
            interval: std::time::Duration::from_secs(sync_interval_secs),
            ..SyncSchedulerConfig::default()
        },
        Arc::clone(&sync_service),
    )
    .start();
    info!(interval_secs = sync_interval_secs, "SyncScheduler started");

    // SECURITY: Configure restrictive CORS policy
    let cors = build_cors_layer();

    // ------------------------------------------------------------------
    // Static file serving for the dashboard SPA
    // ------------------------------------------------------------------
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string());
    let serve_static = ServeDir::new(&static_dir).append_index_html_on_directories(true);

    // ------------------------------------------------------------------
    // Compose routers
    // ------------------------------------------------------------------
    let app = Router::new()
        // Legacy audit API
        .route("/health", get(health_check))
        .route("/api/audit", post(create_audit))
        .route("/api/audit/{id}", get(get_audit))
        .route("/api/audit/{id}/tasks", get(get_audit_tasks))
        .route("/api/clone", post(clone_repository))
        .route("/api/scan/tags", post(scan_tags))
        .route("/api/scan/static", post(scan_static))
        .route("/api/repos", get(list_repos))
        .route("/api/repos/scan", post(scan_repos))
        .route("/api/queue/status", get(queue_status))
        .route("/api/github/stats", get(github_stats))
        .route("/api/github/repos", get(github_repos))
        .route("/api/github/issues", get(github_issues))
        .route("/api/github/prs", get(github_prs))
        .route("/api/github/search", get(github_search))
        .route("/api/github/sync", post(github_sync))
        .with_state(state)
        // New dashboard API (separate state)
        .merge(web_api_router(web_state))
        // Repo management + chat API at /api/v1
        .nest("/api/v1", repo_router(repo_app_state))
        // Serve static files (dashboard HTML/CSS/JS) at /static and /
        .nest_service("/static", serve_static.clone())
        .fallback_service(serve_static)
        // Middleware (applied last, wraps everything)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    info!("Dashboard available at  http://{}/", socket_addr);
    info!(
        "API docs at             http://{}/api/web/health",
        socket_addr
    );
    info!("Security: Restrictive CORS enabled, Git URL whitelist active");

    // Start server
    let listener = tokio::net::TcpListener::bind(&socket_addr)
        .await
        .map_err(|e| AuditError::other(format!("Failed to bind to {}: {}", socket_addr, e)))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| AuditError::other(format!("Server error: {}", e)))?;

    Ok(())
}

/// Build a restrictive CORS layer
///
/// SECURITY: This replaces the previous `CorsLayer::permissive()` which allowed
/// any origin to make requests, exposing the API to CSRF/XSS attacks.
fn build_cors_layer() -> CorsLayer {
    // Get allowed origins from environment or use defaults
    let allowed_origins: Vec<String> = std::env::var("CORS_ALLOWED_ORIGINS")
        .map(|s| s.split(',').map(|o| o.trim().to_string()).collect())
        .unwrap_or_else(|_| {
            vec![
                "http://localhost:3000".to_string(),
                "http://localhost:8080".to_string(),
                "http://127.0.0.1:3000".to_string(),
                "http://127.0.0.1:8080".to_string(),
            ]
        });

    info!("CORS allowed origins: {:?}", allowed_origins);

    // Build the CORS layer with restrictive settings
    CorsLayer::new()
        // Only allow specific origins (not wildcard)
        .allow_origin(
            allowed_origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect::<Vec<header::HeaderValue>>(),
        )
        // Only allow specific HTTP methods
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        // Only allow specific headers
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
        // Don't allow credentials by default (enable explicitly if needed)
        .allow_credentials(false)
        // Cache preflight requests for 1 hour
        .max_age(Duration::from_secs(3600))
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Create a new audit
async fn create_audit(
    State(state): State<AppState>,
    Json(request): Json<AuditRequest>,
) -> Result<Json<AuditResponse>> {
    info!("Creating audit for repository: {}", request.repository);

    // SECURITY: Validate repository URL/path before cloning to prevent SSRF attacks
    let repo_path =
        if request.repository.starts_with("http") || request.repository.starts_with("git@") {
            // Validate Git URL against whitelist (SSRF prevention)
            state
                .config
                .security
                .validate_git_url(&request.repository)?;

            // Clone from validated URL
            state.git_manager.clone_repo(&request.repository, None)?
        } else {
            // Validate local path (path traversal prevention)
            state
                .config
                .security
                .validate_local_path(&request.repository)?;

            // Use local path
            std::path::PathBuf::from(&request.repository)
        };

    // Checkout branch if specified
    if let Some(branch) = &request.branch {
        state.git_manager.checkout(&repo_path, branch)?;
    }

    // Create scanner
    let scanner = Scanner::new(
        repo_path.clone(),
        state.config.scanner.max_file_size,
        request.include_tests,
    )?;

    // Perform scan
    let mut report = scanner.scan(&request)?;

    // If LLM is enabled, perform LLM analysis
    if request.enable_llm {
        if let Some(llm_client) = &state.llm_client {
            info!("Performing LLM analysis");

            // Analyze files (simplified - in production, batch this)
            for file_analysis in &mut report.files {
                if file_analysis.priority as u8 >= 3 {
                    // High and Critical priority
                    if let Ok(content) = tokio::fs::read_to_string(&file_analysis.path).await {
                        if let Ok(llm_result) = llm_client
                            .analyze_file(&file_analysis.path, &content, file_analysis.category)
                            .await
                        {
                            file_analysis.llm_analysis = Some(llm_result.summary.clone());
                            file_analysis.security_rating =
                                Some(crate::types::SecurityRating::from_importance(
                                    llm_result.importance,
                                ));
                        }
                    }
                }
            }
        }
    }

    // Generate tasks
    let mut task_gen = TaskGenerator::new();

    // Collect all tags
    let all_tags: Vec<_> = report.files.iter().flat_map(|f| &f.tags).cloned().collect();
    task_gen.generate_from_tags(&all_tags)?;
    task_gen.generate_from_analyses(&report.files)?;

    report.tasks = task_gen.tasks().to_vec();
    report.summary.total_tasks = report.tasks.len();

    // Save report
    let report_id = report.id.clone();
    save_report(&state.config.storage.reports_dir, &report).await?;

    // Save tasks
    save_tasks(&state.config.storage.tasks_dir, &report.tasks).await?;

    info!(
        "Audit completed: {} files, {} issues, {} tasks",
        report.summary.total_files, report.summary.total_issues, report.summary.total_tasks
    );

    Ok(Json(AuditResponse {
        id: report_id,
        status: "completed".to_string(),
        report,
    }))
}

/// Get an audit report by ID
async fn get_audit(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AuditReport>> {
    let report_path = state
        .config
        .storage
        .reports_dir
        .join(format!("{}.json", id));

    let content = tokio::fs::read_to_string(&report_path)
        .await
        .map_err(|_| AuditError::FileNotFound(report_path.clone()))?;

    let report: AuditReport = serde_json::from_str(&content)?;

    Ok(Json(report))
}

/// Get tasks for an audit
async fn get_audit_tasks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TasksResponse>> {
    let tasks_path = state.config.storage.tasks_dir.join(format!("{}.json", id));

    let content = tokio::fs::read_to_string(&tasks_path)
        .await
        .map_err(|_| AuditError::FileNotFound(tasks_path.clone()))?;

    let tasks: Vec<Task> = serde_json::from_str(&content)?;

    Ok(Json(TasksResponse { tasks }))
}

/// Clone a repository endpoint
async fn clone_repository(
    State(state): State<AppState>,
    Json(request): Json<CloneRequest>,
) -> Result<Json<CloneResponse>> {
    info!("Cloning repository: {}", request.url);

    // SECURITY: Validate Git URL against whitelist to prevent SSRF attacks
    // This prevents attackers from using the clone endpoint to:
    // 1. Access internal services (e.g., http://localhost, http://169.254.169.254)
    // 2. Clone from untrusted/malicious repositories
    // 3. Exfiltrate data to attacker-controlled servers
    state.config.security.validate_git_url(&request.url)?;

    let repo_path = state.git_manager.clone_repo(&request.url, None)?;

    if let Some(branch) = &request.branch {
        state.git_manager.checkout(&repo_path, branch)?;
    }

    let stats = state.git_manager.stats(&repo_path)?;

    Ok(Json(CloneResponse {
        path: repo_path.to_string_lossy().to_string(),
        branch: state
            .git_manager
            .current_branch(&repo_path)
            .unwrap_or_default(),
        commit_count: stats.commit_count,
    }))
}

/// Scan for tags only
async fn scan_tags(
    State(_state): State<AppState>,
    Json(request): Json<ScanRequest>,
) -> Result<Json<TagsResponse>> {
    info!("Scanning for tags in: {}", request.path);

    let tag_scanner = TagScanner::new()?;
    let tags = tag_scanner.scan_directory(&std::path::PathBuf::from(&request.path))?;

    let grouped = tag_scanner.group_by_type(&tags);

    let by_type: HashMap<String, usize> = grouped
        .into_iter()
        .map(|(k, v)| (format!("{:?}", k), v.len()))
        .collect();

    Ok(Json(TagsResponse {
        total: tags.len(),
        by_type,
        tags,
    }))
}

/// Perform static analysis only
async fn scan_static(
    State(state): State<AppState>,
    Json(request): Json<ScanRequest>,
) -> Result<Json<StaticAnalysisResponse>> {
    info!("Running static analysis on: {}", request.path);

    let scanner = Scanner::new(
        std::path::PathBuf::from(&request.path),
        state.config.scanner.max_file_size,
        false,
    )?;

    let audit_request = AuditRequest {
        repository: request.path.clone(),
        branch: None,
        enable_llm: false,
        focus: vec![],
        include_tests: false,
    };

    let report = scanner.scan(&audit_request)?;

    Ok(Json(StaticAnalysisResponse {
        total_files: report.summary.total_files,
        total_issues: report.summary.total_issues,
        critical_files: report.summary.critical_files,
        issues_by_severity: report.issues_by_severity,
    }))
}

/// Save report to disk
async fn save_report(dir: &std::path::Path, report: &AuditReport) -> Result<()> {
    fs::create_dir_all(dir).await?;
    let path = dir.join(format!("{}.json", report.id));
    let content = serde_json::to_string_pretty(report)?;
    fs::write(path, content).await?;
    Ok(())
}

/// Save tasks to disk
async fn save_tasks(dir: &std::path::Path, tasks: &[crate::types::Task]) -> Result<()> {
    fs::create_dir_all(dir).await?;

    // Use the first task's associated report ID or generate a new one
    let id = if !tasks.is_empty() {
        tasks[0].id.split('-').next().unwrap_or("tasks")
    } else {
        "tasks"
    };

    let path = dir.join(format!("{}.json", id));
    let content = serde_json::to_string_pretty(tasks)?;
    fs::write(path, content).await?;
    Ok(())
}

// ===== Response Types =====

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct AuditResponse {
    id: String,
    status: String,
    report: AuditReport,
}

#[derive(Debug, Deserialize)]
struct CloneRequest {
    url: String,
    branch: Option<String>,
}

#[derive(Debug, Serialize)]
struct CloneResponse {
    path: String,
    branch: String,
    commit_count: usize,
}

#[derive(Debug, Deserialize)]
struct ScanRequest {
    path: String,
}

#[derive(Debug, Serialize)]
struct TagsResponse {
    total: usize,
    by_type: HashMap<String, usize>,
    tags: Vec<AuditTag>,
}

#[derive(Debug, Serialize)]
struct TasksResponse {
    tasks: Vec<Task>,
}

#[derive(Debug, Serialize)]
struct StaticAnalysisResponse {
    total_files: usize,
    total_issues: usize,
    critical_files: usize,
    issues_by_severity: HashMap<crate::types::IssueSeverity, usize>,
}

// ===== Visualization Endpoints =====

// Neuromorphic visualization endpoints removed - feature specific to another project

// ===== Error Response =====

impl IntoResponse for AuditError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuditError::FileNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AuditError::Config(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AuditError::InvalidApiKey { .. } => (StatusCode::UNAUTHORIZED, self.to_string()),
            AuditError::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(ErrorResponse {
            error: message,
            status: status.as_u16(),
        });

        (status, body).into_response()
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    status: u16,
}

// ============================================================================
// Repository Management Endpoints
// ============================================================================

/// List all tracked repositories
async fn list_repos(State(state): State<AppState>) -> Result<Json<Vec<Repository>>> {
    let repos = db::list_repositories(&state.db_pool)
        .await
        .map_err(|e| AuditError::other(format!("Failed to list repositories: {}", e)))?;

    Ok(Json(repos))
}

#[derive(Debug, Deserialize)]
struct ScanReposRequest {
    token: Option<String>,
}

#[derive(Debug, Serialize)]
struct ScanReposResponse {
    synced_count: usize,
    repositories: Vec<Repository>,
}

/// Scan and sync repositories from GitHub
async fn scan_repos(
    State(state): State<AppState>,
    Json(req): Json<ScanReposRequest>,
) -> Result<Json<ScanReposResponse>> {
    // Sync repositories from GitHub
    let repo_ids = sync_repos_to_db(&state.db_pool, req.token.as_deref())
        .await
        .map_err(|e| AuditError::other(format!("Failed to sync repositories: {}", e)))?;

    // Fetch the synced repositories
    let repositories = db::list_repositories(&state.db_pool)
        .await
        .map_err(|e| AuditError::other(format!("Failed to list repositories: {}", e)))?;

    Ok(Json(ScanReposResponse {
        synced_count: repo_ids.len(),
        repositories,
    }))
}

// ============================================================================
// Queue Management Endpoints
// ============================================================================

/// Get queue status
async fn queue_status(State(state): State<AppState>) -> Result<Json<QueueStats>> {
    let stats = get_queue_stats(&state.db_pool)
        .await
        .map_err(|e| AuditError::other(format!("Failed to get queue stats: {}", e)))?;

    Ok(Json(stats))
}

// ============================================================================
// GitHub Integration Endpoints
// ============================================================================

#[derive(Debug, Serialize)]
struct GitHubStatsResponse {
    repositories: i64,
    issues: i64,
    pull_requests: i64,
    commits: i64,
    events: i64,
    last_sync: Option<String>,
    top_repos: Vec<TopRepo>,
}

#[derive(Debug, Serialize)]
struct TopRepo {
    name: String,
    stars: i64,
}

#[derive(Debug, Deserialize)]
struct GitHubSearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: i32,
}

fn default_limit() -> i32 {
    20
}

#[derive(Debug, Deserialize)]
struct GitHubIssuesQuery {
    repo: Option<String>,
    state: Option<String>,
    #[serde(default = "default_limit")]
    limit: i32,
}

#[derive(Debug, Deserialize)]
struct GitHubPrsQuery {
    repo: Option<String>,
    state: Option<String>,
    #[serde(default = "default_limit")]
    limit: i32,
}

#[derive(Debug, Deserialize)]
struct GitHubReposQuery {
    language: Option<String>,
    #[serde(default = "default_limit")]
    limit: i32,
}

#[derive(Debug, Deserialize)]
struct GitHubSyncRequest {
    full: Option<bool>,
    repo: Option<String>,
}

/// Get GitHub integration statistics
async fn github_stats(State(state): State<AppState>) -> Result<Json<GitHubStatsResponse>> {
    let stats: (i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            (SELECT COUNT(*) FROM github_repositories) as repos,
            (SELECT COUNT(*) FROM github_issues) as issues,
            (SELECT COUNT(*) FROM github_pull_requests) as prs,
            (SELECT COUNT(*) FROM github_commits) as commits,
            (SELECT COUNT(*) FROM github_events) as events
        "#,
    )
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| AuditError::other(format!("Failed to get GitHub stats: {}", e)))?;

    let last_sync: Option<String> = sqlx::query_scalar(
        "SELECT MAX(last_synced_at) FROM github_repositories WHERE last_synced_at IS NOT NULL",
    )
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| AuditError::other(format!("Failed to get last sync time: {}", e)))?;

    let top_repos: Vec<(String, i64)> = sqlx::query_as(
        "SELECT full_name, stargazers_count FROM github_repositories
         ORDER BY stargazers_count DESC LIMIT 5",
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| AuditError::other(format!("Failed to get top repos: {}", e)))?;

    Ok(Json(GitHubStatsResponse {
        repositories: stats.0,
        issues: stats.1,
        pull_requests: stats.2,
        commits: stats.3,
        events: stats.4,
        last_sync,
        top_repos: top_repos
            .into_iter()
            .map(|(name, stars)| TopRepo { name, stars })
            .collect(),
    }))
}

/// Search GitHub repositories
async fn github_repos(
    State(state): State<AppState>,
    Query(params): Query<GitHubReposQuery>,
) -> Result<Json<Vec<crate::github::search::SearchResult>>> {
    use crate::github::search::{GitHubSearcher, SearchQuery, SearchType};

    let searcher = GitHubSearcher::new(state.db_pool.clone());
    let mut query = SearchQuery::new("")
        .with_type(SearchType::Repositories)
        .limit(params.limit);

    if let Some(lang) = params.language {
        query = query.with_language(lang);
    }

    let results = searcher
        .search(query)
        .await
        .map_err(|e| AuditError::other(format!("Failed to search repositories: {}", e)))?;

    Ok(Json(results))
}

/// Get GitHub issues
async fn github_issues(
    State(state): State<AppState>,
    Query(params): Query<GitHubIssuesQuery>,
) -> Result<Json<Vec<crate::github::search::SearchResult>>> {
    use crate::github::search::{GitHubSearcher, SearchQuery, SearchType};

    let searcher = GitHubSearcher::new(state.db_pool.clone());
    let mut query = SearchQuery::new("")
        .with_type(SearchType::Issues)
        .limit(params.limit);

    let state_param = params.state.as_deref().unwrap_or("open");
    if state_param == "open" {
        query = query.only_open();
    } else if state_param == "closed" {
        query = query.only_closed();
    }

    if let Some(repo) = params.repo {
        query = query.in_repo(repo);
    }

    let results = searcher
        .search(query)
        .await
        .map_err(|e| AuditError::other(format!("Failed to search issues: {}", e)))?;

    Ok(Json(results))
}

/// Get GitHub pull requests
async fn github_prs(
    State(state): State<AppState>,
    Query(params): Query<GitHubPrsQuery>,
) -> Result<Json<Vec<crate::github::search::SearchResult>>> {
    use crate::github::search::{GitHubSearcher, SearchQuery, SearchType};

    let searcher = GitHubSearcher::new(state.db_pool.clone());
    let mut query = SearchQuery::new("")
        .with_type(SearchType::PullRequests)
        .limit(params.limit);

    let state_param = params.state.as_deref().unwrap_or("open");
    if state_param == "open" {
        query = query.only_open();
    } else if state_param == "closed" {
        query = query.only_closed();
    }

    if let Some(repo) = params.repo {
        query = query.in_repo(repo);
    }

    let results = searcher
        .search(query)
        .await
        .map_err(|e| AuditError::other(format!("Failed to search pull requests: {}", e)))?;

    Ok(Json(results))
}

/// Search GitHub data
async fn github_search(
    State(state): State<AppState>,
    Query(params): Query<GitHubSearchQuery>,
) -> Result<Json<serde_json::Value>> {
    use crate::github::search::{GitHubSearcher, SearchQuery, SearchType};

    let searcher = GitHubSearcher::new(state.db_pool.clone());

    // Search all types and return combined results
    let repos_query = SearchQuery::new(&params.q)
        .with_type(SearchType::Repositories)
        .limit(params.limit.min(10));
    let repos = searcher
        .search(repos_query)
        .await
        .map_err(|e| AuditError::other(format!("Failed to search repositories: {}", e)))?;

    let issues_query = SearchQuery::new(&params.q)
        .with_type(SearchType::Issues)
        .limit(params.limit.min(10));
    let issues = searcher
        .search(issues_query)
        .await
        .map_err(|e| AuditError::other(format!("Failed to search issues: {}", e)))?;

    let prs_query = SearchQuery::new(&params.q)
        .with_type(SearchType::PullRequests)
        .limit(params.limit.min(10));
    let prs = searcher
        .search(prs_query)
        .await
        .map_err(|e| AuditError::other(format!("Failed to search pull requests: {}", e)))?;

    Ok(Json(serde_json::json!({
        "repositories": repos,
        "issues": issues,
        "pull_requests": prs,
    })))
}

/// Trigger GitHub sync
async fn github_sync(
    State(state): State<AppState>,
    Json(params): Json<GitHubSyncRequest>,
) -> Result<Json<serde_json::Value>> {
    use crate::github::{GitHubClient, SyncEngine, SyncOptions};
    use std::env;

    let token = env::var("GITHUB_TOKEN")
        .map_err(|_| AuditError::other("GITHUB_TOKEN environment variable not set"))?;

    let client = GitHubClient::new(token)
        .map_err(|e| AuditError::other(format!("Failed to create GitHub client: {}", e)))?;

    let sync_engine = SyncEngine::new(client.clone(), state.db_pool.clone());

    let options = if params.full.unwrap_or(false) {
        SyncOptions::default().force_full()
    } else {
        SyncOptions::default()
    };

    let options = if let Some(repo) = params.repo {
        options.with_repos(vec![repo])
    } else {
        options
    };

    let result = sync_engine
        .sync_with_options(options)
        .await
        .map_err(|e| AuditError::other(format!("Failed to sync: {}", e)))?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "repositories": result.repos_synced,
        "issues": result.issues_synced,
        "pull_requests": result.prs_synced,
        "duration_secs": result.duration_secs
    })))
}
