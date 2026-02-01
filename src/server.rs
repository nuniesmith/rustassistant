//! Axum API server for the audit service (API-only, no web UI)

use crate::config::Config;
use crate::error::{AuditError, Result};
use crate::git::GitManager;
use crate::llm::LlmClient;
// Neuromorphic mapper removed - feature not currently implemented
use crate::research;
use crate::scanner::Scanner;
use crate::tags::TagScanner;
use crate::tasks::TaskGenerator;
use crate::types::{AuditReport, AuditRequest, AuditTag, Task};
use axum::{
    extract::{Json, Path, State},
    http::{header, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    config: Arc<Config>,
    git_manager: Arc<GitManager>,
    llm_client: Option<Arc<LlmClient>>,
}

impl AppState {
    /// Create new application state
    pub fn new(config: Config) -> Result<Self> {
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

        Ok(Self {
            config: Arc::new(config),
            git_manager,
            llm_client,
        })
    }
}

/// Run the audit server
pub async fn run_server(config: Config) -> Result<()> {
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let socket_addr: SocketAddr = addr
        .parse()
        .map_err(|e| AuditError::config(format!("Invalid server address: {}", e)))?;

    info!("Starting API-only audit server on {}", socket_addr);

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create application state
    let state = AppState::new(config.clone())?;

    // SECURITY: Configure restrictive CORS policy instead of permissive
    // Only allow requests from trusted origins
    let cors = build_cors_layer();

    // Build API-only router (no static file serving or web UI)
    // SECURITY: Middleware stack includes:
    // - Restrictive CORS (not permissive)
    // - Request tracing for audit logging
    // - URL validation happens in handlers (SSRF prevention)
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/audit", post(create_audit))
        .route("/api/audit/{id}", get(get_audit))
        .route("/api/audit/{id}/tasks", get(get_audit_tasks))
        .route("/api/clone", post(clone_repository))
        .route("/api/scan/tags", post(scan_tags))
        .route("/api/scan/static", post(scan_static))
        .route("/api/research/analyze", post(analyze_research))
        .route("/api/research/file", post(analyze_research_file))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    info!("API server listening on {} (API-only mode)", socket_addr);
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

#[derive(Debug, Deserialize)]
struct ResearchAnalyzeRequest {
    content: String,
    title: String,
    #[serde(default)]
    generate_tasks: bool,
}

#[derive(Debug, Deserialize)]
struct ResearchFileRequest {
    file_path: String,
    #[serde(default)]
    generate_tasks: bool,
}

#[derive(Debug, Serialize)]
struct ResearchAnalyzeResponse {
    breakdown: research::ResearchBreakdown,
    #[serde(skip_serializing_if = "Option::is_none")]
    tasks: Option<Vec<research::ResearchTask>>,
    breakdown_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tasks_path: Option<String>,
}

// ===== Research Endpoints =====

/// Analyze research content (direct text input)
async fn analyze_research(
    State(state): State<AppState>,
    Json(request): Json<ResearchAnalyzeRequest>,
) -> Result<Json<ResearchAnalyzeResponse>> {
    info!("Analyzing research content: {}", request.title);

    // Check if LLM is available
    let llm_client = state
        .llm_client
        .as_ref()
        .ok_or_else(|| AuditError::config("LLM is not enabled. Set LLM_ENABLED=true"))?;

    // Check if research is enabled
    let research_config = state
        .config
        .research
        .as_ref()
        .ok_or_else(|| AuditError::config("Research pipeline is not configured"))?;

    if !research_config.enabled {
        return Err(AuditError::config("Research pipeline is disabled"));
    }

    // Analyze content
    let breakdown =
        research::analyze_content(&request.content, &request.title, llm_client, &state.config)
            .await?;

    // Save breakdown
    let output_dir = std::path::PathBuf::from(&research_config.output_dir);
    let breakdown_path = research::save_breakdown(&breakdown, &output_dir, None)?;

    // Extract tasks if requested
    let (tasks, tasks_path) = if request.generate_tasks {
        info!("Extracting tasks from research content");

        let extracted_tasks =
            research::extract_tasks(&breakdown.markdown_content, llm_client, &state.config).await?;

        let task_file = breakdown_path.with_extension("tasks.json");
        research::save_tasks(&extracted_tasks, &task_file)?;

        (
            Some(extracted_tasks),
            Some(task_file.to_string_lossy().to_string()),
        )
    } else {
        (None, None)
    };

    Ok(Json(ResearchAnalyzeResponse {
        breakdown,
        tasks,
        breakdown_path: breakdown_path.to_string_lossy().to_string(),
        tasks_path,
    }))
}

/// Analyze research file (file path input)
async fn analyze_research_file(
    State(state): State<AppState>,
    Json(request): Json<ResearchFileRequest>,
) -> Result<Json<ResearchAnalyzeResponse>> {
    info!("Analyzing research file: {}", request.file_path);

    // Check if LLM is available
    let llm_client = state
        .llm_client
        .as_ref()
        .ok_or_else(|| AuditError::config("LLM is not enabled. Set LLM_ENABLED=true"))?;

    // Check if research is enabled
    let research_config = state
        .config
        .research
        .as_ref()
        .ok_or_else(|| AuditError::config("Research pipeline is not configured"))?;

    if !research_config.enabled {
        return Err(AuditError::config("Research pipeline is disabled"));
    }

    let file_path = std::path::PathBuf::from(&request.file_path);

    // Verify file exists
    if !file_path.exists() {
        return Err(AuditError::FileNotFound(file_path));
    }

    // Analyze file
    let breakdown = research::analyze_file(&file_path, llm_client, &state.config).await?;

    // Save breakdown
    let output_dir = std::path::PathBuf::from(&research_config.output_dir);
    let file_stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("research");
    let breakdown_path = research::save_breakdown(&breakdown, &output_dir, Some(file_stem))?;

    // Extract tasks if requested
    let (tasks, tasks_path) = if request.generate_tasks {
        info!("Extracting tasks from research file");

        let extracted_tasks =
            research::extract_tasks(&breakdown.markdown_content, llm_client, &state.config).await?;

        let task_file = breakdown_path.with_extension("tasks.json");
        research::save_tasks(&extracted_tasks, &task_file)?;

        (
            Some(extracted_tasks),
            Some(task_file.to_string_lossy().to_string()),
        )
    } else {
        (None, None)
    };

    Ok(Json(ResearchAnalyzeResponse {
        breakdown,
        tasks,
        breakdown_path: breakdown_path.to_string_lossy().to_string(),
        tasks_path,
    }))
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
