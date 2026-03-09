//! Web API handlers for the RustAssistant dashboard
//!
//! Provides REST + SSE endpoints consumed by the single-page dashboard:
//!
//! | Method | Path                              | Description                           |
//! |--------|-----------------------------------|---------------------------------------|
//! | GET    | /api/web/health                   | Server health + version               |
//! | GET    | /api/web/repos                    | List cloned/tracked repos             |
//! | POST   | /api/web/repos/clone              | Clone a repo by URL                   |
//! | DELETE | /api/web/repos/:id                | Remove a tracked repo                 |
//! | GET    | /api/web/repos/:id/todo           | Read todo.md for a repo               |
//! | GET    | /api/web/repos/:id/scan           | Run todo scan (no LLM)                |
//! | POST   | /api/web/pipeline/dispatch        | Dispatch a pipeline step (background) |
//! | POST   | /api/web/pipeline/stream          | Dispatch a pipeline step (SSE stream) |
//! | POST   | /api/web/chat                     | Single-turn chat with Grok 4.1        |
//! | GET    | /api/web/jobs                     | List recent job history               |
//! | GET    | /api/web/jobs/:id                 | Get a single job result               |

use crate::db::{self, Database};
use crate::error::{AuditError, Result};
use crate::git::GitManager;
use crate::grok_client::GrokClient;
use crate::todo::{
    CommentPriority, PlannerConfig, ScaffoldConfig, ScanConfig, SyncConfig, TodoCommentScanner,
    TodoPlanner, TodoScaffolder, TodoSyncer, TodoWorker, WorkBatch, WorkConfig,
};

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response, Sse},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, TimeZone, Utc};
use futures::stream::once as stream_once;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{collections::HashMap, convert::Infallible, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_stream::StreamExt as TokioStreamExt;
use tracing::error;
use uuid::Uuid;

// ============================================================================
// Shared state
// ============================================================================

/// All mutable runtime state shared across handlers
#[derive(Clone)]
pub struct WebState {
    pub pool: PgPool,
    pub git: Arc<GitManager>,
    pub workspace: PathBuf,
    /// In-memory job store (keyed by job UUID)
    pub jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
}

impl WebState {
    pub fn new(pool: PgPool, git: Arc<GitManager>, workspace: PathBuf) -> Self {
        Self {
            pool,
            git,
            workspace,
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Build a GrokClient backed by the shared pool
    pub async fn grok(&self) -> anyhow::Result<GrokClient> {
        let db = Database::from_pool(self.pool.clone());
        GrokClient::from_env(db).await
    }

    /// Resolve a repo path from a repo_id (UUID string stored as folder name)
    pub fn repo_path(&self, repo_id: &str) -> PathBuf {
        self.workspace.join(repo_id)
    }
}

// ============================================================================
// Job tracking
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Success,
    Failed,
    DryRun,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    pub kind: String,
    pub repo_id: String,
    pub status: JobStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub log_lines: Vec<String>,
    pub result_json: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl JobRecord {
    pub fn new(kind: &str, repo_id: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            kind: kind.to_string(),
            repo_id: repo_id.to_string(),
            status: JobStatus::Queued,
            started_at: Utc::now(),
            finished_at: None,
            log_lines: Vec::new(),
            result_json: None,
            error: None,
        }
    }
}

// ============================================================================
// Router
// ============================================================================

/// Attach all `/api/web/…` routes to a router.
pub fn web_api_router(state: WebState) -> Router {
    Router::new()
        .route("/api/web/health", get(handle_health))
        .route("/api/web/repos", get(handle_list_repos))
        .route("/api/web/repos/clone", post(handle_clone_repo))
        .route("/api/web/repos/:id", delete(handle_delete_repo))
        .route("/api/web/repos/:id/todo", get(handle_get_todo))
        .route("/api/web/repos/:id/scan", get(handle_scan_repo))
        .route("/api/web/pipeline/dispatch", post(handle_dispatch))
        .route("/api/web/pipeline/stream", post(handle_dispatch_stream))
        .route("/api/web/chat", post(handle_chat))
        .route("/api/web/jobs", get(handle_list_jobs))
        .route("/api/web/jobs/:id", get(handle_get_job))
        .with_state(state)
}

// ============================================================================
// Health
// ============================================================================

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    grok_available: bool,
}

async fn handle_health(State(state): State<WebState>) -> impl IntoResponse {
    let grok_available = std::env::var("XAI_API_KEY")
        .or_else(|_| std::env::var("GROK_API_KEY"))
        .is_ok();

    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        grok_available,
    })
}

// ============================================================================
// Repo management
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoInfo {
    pub id: String,
    pub name: String,
    pub url: Option<String>,
    pub path: String,
    pub has_todo: bool,
    pub added_at: DateTime<Utc>,
}

async fn handle_list_repos(State(state): State<WebState>) -> impl IntoResponse {
    match db::list_repositories(&state.pool).await {
        Ok(repos) => {
            let infos: Vec<RepoInfo> = repos
                .iter()
                .map(|r| {
                    let path = PathBuf::from(&r.path);
                    let added_at = Utc
                        .timestamp_opt(r.created_at, 0)
                        .single()
                        .unwrap_or_else(Utc::now);
                    RepoInfo {
                        id: r.id.clone(),
                        name: r.name.clone(),
                        url: r.git_url.clone(),
                        path: r.path.clone(),
                        has_todo: path.join("todo.md").exists() || path.join("TODO.md").exists(),
                        added_at,
                    }
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!({ "repos": infos }))).into_response()
        }
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct CloneRequest {
    pub url: String,
    pub branch: Option<String>,
    pub name: Option<String>,
}

async fn handle_clone_repo(
    State(state): State<WebState>,
    Json(req): Json<CloneRequest>,
) -> impl IntoResponse {
    // Validate URL (basic SSRF guard — only https github/gitlab/bitbucket)
    if !is_allowed_git_url(&req.url) {
        return api_error(
            StatusCode::BAD_REQUEST,
            "Only HTTPS GitHub / GitLab / Bitbucket URLs are allowed",
        );
    }

    let name = req.name.unwrap_or_else(|| infer_repo_name(&req.url));

    // GitManager::clone_repo is synchronous (uses libgit2); run on blocking thread
    let git = state.git.clone();
    let url = req.url.clone();
    let name_clone = name.clone();
    let cloned_path =
        match tokio::task::spawn_blocking(move || git.clone_repo(&url, Some(&name_clone))).await {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => {
                return api_error(StatusCode::BAD_REQUEST, &format!("Clone failed: {}", e))
            }
            Err(e) => {
                return api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Clone task panicked: {}", e),
                )
            }
        };

    // Register in DB
    match db::add_repository(
        &state.pool,
        &cloned_path.to_string_lossy(),
        &name,
        Some(&req.url),
    )
    .await
    {
        Ok(repo) => {
            let has_todo =
                cloned_path.join("todo.md").exists() || cloned_path.join("TODO.md").exists();
            let added_at = Utc
                .timestamp_opt(repo.created_at, 0)
                .single()
                .unwrap_or_else(Utc::now);
            let info = RepoInfo {
                id: repo.id.clone(),
                name: repo.name.clone(),
                url: repo.git_url.clone(),
                path: cloned_path.to_string_lossy().to_string(),
                has_todo,
                added_at,
            };
            (
                StatusCode::CREATED,
                Json(serde_json::json!({ "repo": info })),
            )
                .into_response()
        }
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn handle_delete_repo(
    State(state): State<WebState>,
    AxumPath(repo_id): AxumPath<String>,
) -> impl IntoResponse {
    match db::remove_repository(&state.pool, &repo_id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({ "deleted": repo_id })),
        )
            .into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn handle_get_todo(
    State(state): State<WebState>,
    AxumPath(repo_id): AxumPath<String>,
) -> impl IntoResponse {
    let repo = match db::get_repository(&state.pool, &repo_id).await {
        Ok(r) => r,
        Err(_) => return api_error(StatusCode::NOT_FOUND, "Repo not found"),
    };

    let base = PathBuf::from(&repo.path);
    let todo_path = if base.join("todo.md").exists() {
        base.join("todo.md")
    } else if base.join("TODO.md").exists() {
        base.join("TODO.md")
    } else {
        return api_error(StatusCode::NOT_FOUND, "No todo.md found in repo");
    };

    match std::fs::read_to_string(&todo_path) {
        Ok(content) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "path": todo_path.display().to_string(),
                "content": content,
            })),
        )
            .into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ============================================================================
// Quick scan (no LLM)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ScanQuery {
    pub filter: Option<String>,
}

async fn handle_scan_repo(
    State(state): State<WebState>,
    AxumPath(repo_id): AxumPath<String>,
    Query(params): Query<ScanQuery>,
) -> impl IntoResponse {
    let repo = match db::get_repository(&state.pool, &repo_id).await {
        Ok(r) => r,
        Err(_) => return api_error(StatusCode::NOT_FOUND, "Repo not found"),
    };

    let repo_path = PathBuf::from(&repo.path);
    let config = ScanConfig {
        relative_paths: true,
        ..ScanConfig::default()
    };

    let scanner = match TodoCommentScanner::with_config(config) {
        Ok(s) => s,
        Err(e) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    match scanner.scan_repo(&repo_path) {
        Ok(output) => {
            let min_priority = match params
                .filter
                .as_deref()
                .unwrap_or("low")
                .to_lowercase()
                .as_str()
            {
                "high" => CommentPriority::High,
                "medium" => CommentPriority::Medium,
                _ => CommentPriority::Low,
            };
            let filtered = output.filter_by_priority(min_priority);

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "repo_id": repo_id,
                    "total_files_scanned": output.total_files_scanned,
                    "total_found": output.items.len(),
                    "shown": filtered.len(),
                    "summary": output.summary,
                    "items": filtered,
                })),
            )
                .into_response()
        }
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ============================================================================
// Pipeline dispatch  (fire-and-forget with job tracking)
// ============================================================================

/// Which pipeline step to run
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStep {
    Scan,
    Scaffold,
    Plan,
    Work,
    Sync,
    /// Run Scan → Scaffold → Plan in sequence
    Full,
}

#[derive(Debug, Deserialize)]
pub struct DispatchRequest {
    pub repo_id: String,
    pub step: PipelineStep,
    /// For `work`: which batch ID to execute
    pub batch_id: Option<String>,
    /// For `sync`: path to the WorkResult JSON
    pub results_path: Option<String>,
    /// Perform a dry-run (no file writes)
    #[serde(default)]
    pub dry_run: bool,
    /// Overwrite existing stubs (scaffold step)
    #[serde(default)]
    pub overwrite: bool,
}

async fn handle_dispatch(
    State(state): State<WebState>,
    Json(req): Json<DispatchRequest>,
) -> impl IntoResponse {
    let repo = match db::get_repository(&state.pool, &req.repo_id).await {
        Ok(r) => r,
        Err(_) => return api_error(StatusCode::NOT_FOUND, "Repo not found"),
    };

    let repo_path = PathBuf::from(&repo.path);
    let mut job = JobRecord::new(&format!("{:?}", req.step).to_lowercase(), &req.repo_id);
    let job_id = job.id.clone();

    // Insert job as queued
    {
        let mut jobs = state.jobs.write().await;
        jobs.insert(job_id.clone(), job.clone());
    }

    // Spawn the actual work in the background
    let state2 = state.clone();
    let job_id2 = job_id.clone();
    tokio::spawn(async move {
        let result = run_pipeline_step(&state2, &repo_path, &req).await;

        let mut jobs = state2.jobs.write().await;
        if let Some(j) = jobs.get_mut(&job_id2) {
            j.finished_at = Some(Utc::now());
            match result {
                Ok((log, json_result)) => {
                    j.status = if req.dry_run {
                        JobStatus::DryRun
                    } else {
                        JobStatus::Success
                    };
                    j.log_lines = log;
                    j.result_json = json_result;
                }
                Err(e) => {
                    j.status = JobStatus::Failed;
                    j.error = Some(e.to_string());
                    error!("Pipeline job {} failed: {}", job_id2, e);
                }
            }
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "job_id": job_id })),
    )
        .into_response()
}

// ============================================================================
// Pipeline dispatch — SSE streaming version
// ============================================================================

/// Returns an SSE stream that emits `data: <json>\n\n` events as the pipeline
/// progresses.  Each event has the shape:
/// ```json
/// { "type": "log" | "result" | "error" | "done", "payload": <string|object> }
/// ```
async fn handle_dispatch_stream(
    State(state): State<WebState>,
    Json(req): Json<DispatchRequest>,
) -> impl IntoResponse {
    let repo = match db::get_repository(&state.pool, &req.repo_id).await {
        Ok(r) => r,
        Err(e) => {
            // Return a single-event SSE stream with the error
            let err_event = axum::response::sse::Event::default()
                .data(serde_json::json!({ "type": "error", "payload": e.to_string() }).to_string());
            let s = stream_once(async move { Ok::<_, Infallible>(err_event) });
            return Sse::new(s)
                .keep_alive(axum::response::sse::KeepAlive::default())
                .into_response();
        }
    };

    let repo_path = PathBuf::from(&repo.path);
    let job_id = Uuid::new_v4().to_string();

    // Channel: pipeline worker sends log lines / final result
    let (tx, rx) = tokio::sync::mpsc::channel::<SseEvent>(64);

    // Record job
    {
        let mut jobs = state.jobs.write().await;
        jobs.insert(
            job_id.clone(),
            JobRecord {
                id: job_id.clone(),
                kind: format!("{:?}", req.step).to_lowercase(),
                repo_id: req.repo_id.clone(),
                status: JobStatus::Running,
                started_at: Utc::now(),
                finished_at: None,
                log_lines: Vec::new(),
                result_json: None,
                error: None,
            },
        );
    }

    let state2 = state.clone();
    let job_id2 = job_id.clone();
    tokio::spawn(async move {
        run_pipeline_step_streaming(&state2, &repo_path, &req, tx.clone()).await;

        let mut jobs = state2.jobs.write().await;
        if let Some(j) = jobs.get_mut(&job_id2) {
            j.finished_at = Some(Utc::now());
        }
    });

    // Convert the mpsc receiver into an SSE stream
    let stream = tokio_stream::wrappers::ReceiverStream::new(rx).map(|ev| {
        let data = serde_json::to_string(&ev).unwrap_or_default();
        Ok::<_, Infallible>(axum::response::sse::Event::default().data(data))
    });

    Sse::new(stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("ping"),
        )
        .into_response()
}

#[derive(Debug, Clone, Serialize)]
struct SseEvent {
    #[serde(rename = "type")]
    kind: &'static str,
    payload: serde_json::Value,
}

impl SseEvent {
    fn log(msg: impl Into<String>) -> Self {
        Self {
            kind: "log",
            payload: serde_json::Value::String(msg.into()),
        }
    }

    fn result(v: serde_json::Value) -> Self {
        Self {
            kind: "result",
            payload: v,
        }
    }

    fn error(msg: impl Into<String>) -> Self {
        Self {
            kind: "error",
            payload: serde_json::Value::String(msg.into()),
        }
    }

    fn done() -> Self {
        Self {
            kind: "done",
            payload: serde_json::Value::Null,
        }
    }
}

// ============================================================================
// Core pipeline execution (shared between dispatch and stream endpoints)
// ============================================================================

/// Fire-and-forget version — collects logs and returns them
fn run_pipeline_step<'a>(
    state: &'a WebState,
    repo_path: &'a PathBuf,
    req: &'a DispatchRequest,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<Output = Result<(Vec<String>, Option<serde_json::Value>)>>
            + Send
            + 'a,
    >,
> {
    Box::pin(async move {
        let db = Database::from_pool(state.pool.clone());
        let mut log: Vec<String> = Vec::new();
        let mut result_json: Option<serde_json::Value> = None;

        match req.step {
            PipelineStep::Scan => {
                log.push(format!("Scanning {}…", repo_path.display()));
                let config = ScanConfig {
                    relative_paths: true,
                    ..ScanConfig::default()
                };
                let scanner = TodoCommentScanner::with_config(config)?;
                let output = scanner.scan_repo(repo_path)?;
                log.push(format!(
                    "Found {} items across {} files",
                    output.items.len(),
                    output.summary.files_with_todos
                ));

                // Write to .rustassistant/scan.json
                let out_dir = repo_path.join(".rustassistant");
                std::fs::create_dir_all(&out_dir).map_err(AuditError::Io)?;
                let scan_path = out_dir.join("scan.json");
                let json_str = output.to_json_pretty()?;
                std::fs::write(&scan_path, &json_str).map_err(AuditError::Io)?;
                log.push(format!("Wrote scan → {}", scan_path.display()));

                result_json = serde_json::from_str(&json_str).ok();
            }

            PipelineStep::Scaffold => {
                log.push(format!("Scaffolding {}…", repo_path.display()));
                let config = ScaffoldConfig {
                    dry_run: req.dry_run,
                    overwrite: req.overwrite,
                    ..ScaffoldConfig::default()
                };
                let scaffolder = TodoScaffolder::from_env(config, db).await?;
                let result = scaffolder.scaffold(repo_path).await?;

                log.push(format!(
                    "Created {} file(s), {} dir(s), skipped {}",
                    result.files_created.len(),
                    result.dirs_created.len(),
                    result.files_skipped.len()
                ));

                let out_dir = repo_path.join(".rustassistant");
                std::fs::create_dir_all(&out_dir).map_err(AuditError::Io)?;
                let plan_path = out_dir.join("scaffold.json");
                let json_str = result.plan.to_json_pretty()?;
                std::fs::write(&plan_path, &json_str).map_err(AuditError::Io)?;
                log.push(format!("Wrote scaffold plan → {}", plan_path.display()));

                result_json = serde_json::from_str(&json_str).ok();
            }

            PipelineStep::Plan => {
                let todo_path = resolve_todo_md(repo_path)?;
                log.push(format!("Planning from {}…", todo_path.display()));
                let config = PlannerConfig::default();
                let planner = TodoPlanner::from_env(config, db).await?;
                let gameplan = planner.plan(&todo_path, Some(repo_path.as_ref())).await?;

                let out_dir = repo_path.join(".rustassistant");
                std::fs::create_dir_all(&out_dir).map_err(AuditError::Io)?;
                let plan_path = out_dir.join("gameplan.json");
                let json_str = gameplan.to_json_pretty()?;
                std::fs::write(&plan_path, &json_str).map_err(AuditError::Io)?;

                log.push(format!(
                    "GamePlan: {} batches, {} items",
                    gameplan.batches.len(),
                    gameplan.total_items_planned
                ));
                log.push(format!("Wrote gameplan → {}", plan_path.display()));
                result_json = serde_json::from_str(&json_str).ok();
            }

            PipelineStep::Work => {
                let batch_id = req
                    .batch_id
                    .as_deref()
                    .ok_or_else(|| AuditError::other("batch_id required for 'work' step"))?;

                let gameplan_path = repo_path.join(".rustassistant").join("gameplan.json");
                if !gameplan_path.exists() {
                    return Err(AuditError::other(
                        "No gameplan.json found — run 'plan' first",
                    ));
                }

                log.push(format!(
                    "Executing batch {} in {}…",
                    batch_id,
                    repo_path.display()
                ));
                let work_batch = WorkBatch::load_from_gameplan(&gameplan_path, batch_id)?;
                let mut config = WorkConfig::for_repo(repo_path);
                if req.dry_run {
                    config = config.as_dry_run();
                }
                let worker = TodoWorker::from_env(config, db).await?;
                let result = worker.execute(&work_batch).await?;

                let results_dir = repo_path.join(".rustassistant").join("results");
                std::fs::create_dir_all(&results_dir).map_err(AuditError::Io)?;
                let result_path = results_dir.join(format!("{}.json", result.batch_id));
                let json_str = result.to_json_pretty()?;

                if !req.dry_run {
                    std::fs::write(&result_path, &json_str).map_err(AuditError::Io)?;
                    log.push(format!("Wrote result → {}", result_path.display()));
                }

                log.push(format!(
                    "Batch {}: {} succeeded / {} failed / {} skipped",
                    result.batch_id,
                    result.items_succeeded,
                    result.items_failed,
                    result.items_skipped,
                ));
                result_json = serde_json::from_str(&json_str).ok();
            }

            PipelineStep::Sync => {
                let todo_path = resolve_todo_md(repo_path)?;
                let results_path = req
                    .results_path
                    .as_ref()
                    .map(PathBuf::from)
                    .ok_or_else(|| AuditError::other("results_path required for 'sync' step"))?;

                log.push(format!(
                    "Syncing {} ← {}…",
                    todo_path.display(),
                    results_path.display()
                ));
                let config = SyncConfig {
                    dry_run: req.dry_run,
                    append_summary: true,
                    ..SyncConfig::default()
                };
                let syncer = TodoSyncer::new(config);
                let sync_result = syncer.sync_from_file(&todo_path, &results_path)?;

                log.push(format!(
                    "Updated {} item(s), {} not found",
                    sync_result.items_updated, sync_result.items_not_found
                ));
                result_json = serde_json::to_value(&sync_result).ok();
            }

            PipelineStep::Full => {
                // Scan
                log.push("── STEP 0: Scan ──".to_string());
                let scan_req = DispatchRequest {
                    repo_id: req.repo_id.clone(),
                    step: PipelineStep::Scan,
                    batch_id: None,
                    results_path: None,
                    dry_run: req.dry_run,
                    overwrite: req.overwrite,
                };
                let (scan_log, scan_json) = run_pipeline_step(state, repo_path, &scan_req).await?;
                log.extend(scan_log);

                // Scaffold
                log.push("── STEP 1: Scaffold ──".to_string());
                let scaffold_req = DispatchRequest {
                    repo_id: req.repo_id.clone(),
                    step: PipelineStep::Scaffold,
                    batch_id: None,
                    results_path: None,
                    dry_run: req.dry_run,
                    overwrite: req.overwrite,
                };
                let (scaffold_log, _) = run_pipeline_step(state, repo_path, &scaffold_req).await?;
                log.extend(scaffold_log);

                // Plan
                log.push("── STEP 2: Plan ──".to_string());
                let plan_req = DispatchRequest {
                    repo_id: req.repo_id.clone(),
                    step: PipelineStep::Plan,
                    batch_id: None,
                    results_path: None,
                    dry_run: req.dry_run,
                    overwrite: req.overwrite,
                };
                let (plan_log, plan_json) = run_pipeline_step(state, repo_path, &plan_req).await?;
                log.extend(plan_log);

                result_json = plan_json;
            }
        }

        Ok((log, result_json))
    }) // end Box::pin
}

/// Streaming version — sends progress through an mpsc channel
fn run_pipeline_step_streaming<'a>(
    state: &'a WebState,
    repo_path: &'a PathBuf,
    req: &'a DispatchRequest,
    tx: tokio::sync::mpsc::Sender<SseEvent>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        // Helper closures avoid the macro trailing-semicolon issue entirely
        let log = |msg: String| {
            let tx = tx.clone();
            async move {
                let _ = tx.send(SseEvent::log(msg)).await;
            }
        };
        let done_tx = tx.clone();

        let db = Database::from_pool(state.pool.clone());

        match req.step {
            PipelineStep::Scan => {
                log(format!("🔍 Scanning {}…", repo_path.display())).await;

                let config = ScanConfig {
                    relative_paths: true,
                    ..ScanConfig::default()
                };

                let scanner = match TodoCommentScanner::with_config(config) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = tx
                            .send(SseEvent::error(format!("Scanner init failed: {}", e)))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                let output = match scanner.scan_repo(repo_path) {
                    Ok(o) => o,
                    Err(e) => {
                        let _ = tx
                            .send(SseEvent::error(format!("Scan failed: {}", e)))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                log(format!(
                    "Found {} items across {} files ({} high / {} medium / {} low)",
                    output.items.len(),
                    output.summary.files_with_todos,
                    output.summary.by_priority.get("high").copied().unwrap_or(0),
                    output
                        .summary
                        .by_priority
                        .get("medium")
                        .copied()
                        .unwrap_or(0),
                    output.summary.by_priority.get("low").copied().unwrap_or(0),
                ))
                .await;

                let out_dir = repo_path.join(".rustassistant");
                let _ = std::fs::create_dir_all(&out_dir);
                let scan_path = out_dir.join("scan.json");
                if let Ok(json_str) = output.to_json_pretty() {
                    let _ = std::fs::write(&scan_path, &json_str);
                    log(format!("✅ Wrote scan → {}", scan_path.display())).await;
                    if let Ok(v) = serde_json::from_str(&json_str) {
                        let _ = tx.send(SseEvent::result(v)).await;
                    }
                }
            }

            PipelineStep::Scaffold => {
                let action = if req.dry_run {
                    "[dry-run] Scaffolding"
                } else {
                    "Scaffolding"
                };
                log(format!("🔧 {} {}…", action, repo_path.display())).await;

                let config = ScaffoldConfig {
                    dry_run: req.dry_run,
                    overwrite: req.overwrite,
                    ..ScaffoldConfig::default()
                };

                let scaffolder = match TodoScaffolder::from_env(config, db).await {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = tx
                            .send(SseEvent::error(format!("Failed to init scaffolder: {}", e)))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                let result = match scaffolder.scaffold(repo_path).await {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx
                            .send(SseEvent::error(format!("Scaffold failed: {}", e)))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                for entry in &result.entry_results {
                    log(format!(
                        "  {:?} {:?} → {:?}",
                        entry.outcome, entry.kind, entry.path
                    ))
                    .await;
                }

                log(format!(
                    "✅ Created {} files, {} dirs, skipped {}",
                    result.files_created.len(),
                    result.dirs_created.len(),
                    result.files_skipped.len()
                ))
                .await;

                let out_dir = repo_path.join(".rustassistant");
                let _ = std::fs::create_dir_all(&out_dir);
                let plan_path = out_dir.join("scaffold.json");
                if let Ok(json_str) = result.plan.to_json_pretty() {
                    let _ = std::fs::write(&plan_path, &json_str);
                    if let Ok(v) = serde_json::from_str(&json_str) {
                        let _ = tx.send(SseEvent::result(v)).await;
                    }
                }
            }

            PipelineStep::Plan => {
                let todo_path = match resolve_todo_md(repo_path) {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = tx.send(SseEvent::error(e.to_string())).await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };
                log(format!("📋 Planning from {}…", todo_path.display())).await;

                let planner = match TodoPlanner::from_env(PlannerConfig::default(), db).await {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = tx
                            .send(SseEvent::error(format!("Failed to init planner: {}", e)))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                log("Calling Grok 4.1 to generate GamePlan…".to_string()).await;
                let gameplan = match planner.plan(&todo_path, Some(repo_path.as_ref())).await {
                    Ok(g) => g,
                    Err(e) => {
                        let _ = tx
                            .send(SseEvent::error(format!("Plan failed: {}", e)))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                log(format!(
                    "✅ {} batches, {} items planned, {} skipped",
                    gameplan.batches.len(),
                    gameplan.total_items_planned,
                    gameplan.skipped_items.len(),
                ))
                .await;

                for batch in gameplan.ordered_batches() {
                    log(format!(
                        "  ▸ [{}] {} — {} item(s), effort: {}",
                        batch.id,
                        batch.title,
                        batch.items.len(),
                        batch.estimated_effort,
                    ))
                    .await;
                }

                let out_dir = repo_path.join(".rustassistant");
                let _ = std::fs::create_dir_all(&out_dir);
                let plan_path = out_dir.join("gameplan.json");
                if let Ok(json_str) = gameplan.to_json_pretty() {
                    let _ = std::fs::write(&plan_path, &json_str);
                    log(format!("✅ Wrote gameplan → {}", plan_path.display())).await;
                    if let Ok(v) = serde_json::from_str(&json_str) {
                        let _ = tx.send(SseEvent::result(v)).await;
                    }
                }
            }

            PipelineStep::Work => {
                let batch_id = match req.batch_id.as_deref() {
                    Some(b) => b.to_string(),
                    None => {
                        let _ = tx
                            .send(SseEvent::error(
                                "batch_id is required for the 'work' step".to_string(),
                            ))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                let gameplan_path = repo_path.join(".rustassistant").join("gameplan.json");
                if !gameplan_path.exists() {
                    let _ = tx
                        .send(SseEvent::error(
                            "No gameplan.json found — run 'plan' first".to_string(),
                        ))
                        .await;
                    let _ = done_tx.send(SseEvent::done()).await;
                    return;
                }

                let action = if req.dry_run {
                    "[dry-run] Executing"
                } else {
                    "Executing"
                };
                log(format!("⚙️  {} batch {}…", action, batch_id)).await;

                let work_batch = match WorkBatch::load_from_gameplan(&gameplan_path, &batch_id) {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = tx
                            .send(SseEvent::error(format!("Failed to load batch: {}", e)))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                let mut config = WorkConfig::for_repo(repo_path);
                if req.dry_run {
                    config = config.as_dry_run();
                }

                let worker = match TodoWorker::from_env(config, db).await {
                    Ok(w) => w,
                    Err(e) => {
                        let _ = tx
                            .send(SseEvent::error(format!("Failed to init worker: {}", e)))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                log("Calling Grok 4.1 to generate code changes…".to_string()).await;
                let result = match worker.execute(&work_batch).await {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx
                            .send(SseEvent::error(format!("Work failed: {}", e)))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                for fc in &result.file_changes {
                    log(format!(
                        "  {:?} {} (+{} / -{})",
                        fc.change_type, fc.file, fc.lines_added, fc.lines_removed,
                    ))
                    .await;
                }

                let status_icon = if result.is_fully_successful() {
                    "✅"
                } else {
                    "⚠️ "
                };
                log(format!(
                    "{} batch {} — {} succeeded / {} failed / {} skipped",
                    status_icon,
                    result.batch_id,
                    result.items_succeeded,
                    result.items_failed,
                    result.items_skipped,
                ))
                .await;

                if !req.dry_run {
                    let results_dir = repo_path.join(".rustassistant").join("results");
                    let _ = std::fs::create_dir_all(&results_dir);
                    let result_path = results_dir.join(format!("{}.json", result.batch_id));
                    if let Ok(json_str) = result.to_json_pretty() {
                        let _ = std::fs::write(&result_path, &json_str);
                        log(format!("✅ Wrote result → {}", result_path.display())).await;
                        if let Ok(v) = serde_json::from_str(&json_str) {
                            let _ = tx.send(SseEvent::result(v)).await;
                        }
                    }
                }
            }

            PipelineStep::Sync => {
                let todo_path = match resolve_todo_md(repo_path) {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = tx.send(SseEvent::error(e.to_string())).await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };
                let results_path = match req.results_path.as_ref() {
                    Some(p) => PathBuf::from(p),
                    None => {
                        let _ = tx
                            .send(SseEvent::error(
                                "results_path required for sync step".to_string(),
                            ))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                let action = if req.dry_run {
                    "[dry-run] Syncing"
                } else {
                    "Syncing"
                };
                log(format!(
                    "🔄 {} {} ← {}…",
                    action,
                    todo_path.display(),
                    results_path.display()
                ))
                .await;

                let config = SyncConfig {
                    dry_run: req.dry_run,
                    append_summary: true,
                    ..SyncConfig::default()
                };
                let syncer = TodoSyncer::new(config);
                let sync_result = match syncer.sync_from_file(&todo_path, &results_path) {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx
                            .send(SseEvent::error(format!("Sync failed: {}", e)))
                            .await;
                        let _ = done_tx.send(SseEvent::done()).await;
                        return;
                    }
                };

                log(format!(
                    "✅ Updated {} item(s), {} not found",
                    sync_result.items_updated, sync_result.items_not_found
                ))
                .await;

                if let Ok(v) = serde_json::to_value(&sync_result) {
                    let _ = tx.send(SseEvent::result(v)).await;
                }
            }

            PipelineStep::Full => {
                for step in [
                    PipelineStep::Scan,
                    PipelineStep::Scaffold,
                    PipelineStep::Plan,
                ] {
                    let label = format!("{:?}", step);
                    log(format!("━━ {} ━━", label.to_uppercase())).await;

                    let sub_req = DispatchRequest {
                        repo_id: req.repo_id.clone(),
                        step,
                        batch_id: None,
                        results_path: None,
                        dry_run: req.dry_run,
                        overwrite: req.overwrite,
                    };

                    run_pipeline_step_streaming(state, repo_path, &sub_req, tx.clone()).await;
                }
            }
        }

        let _ = done_tx.send(SseEvent::done()).await;
    }) // end Box::pin
}

// ============================================================================
// Chat endpoint
// ============================================================================

/// A single message in the conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user" | "assistant" | "system"
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    /// The user's new message
    pub message: String,
    /// Optional conversation history (for multi-turn context)
    pub history: Option<Vec<ChatMessage>>,
    /// Optional repo_id — if set, the repo's todo.md is injected as context
    pub repo_id: Option<String>,
    /// Optional extra context string to prepend
    pub context: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub reply: String,
    pub tokens_used: i64,
    pub cost_usd: f64,
    pub model: String,
}

/// System prompt that makes Grok aware of RustAssistant's capabilities
fn system_prompt(todo_context: Option<&str>) -> String {
    let todo_section = todo_context
        .map(|t| format!("\n\n## Current todo.md\n\n```markdown\n{}\n```\n", t))
        .unwrap_or_default();

    format!(
        r#"You are RustAssistant, an expert Rust developer and code-workflow automation agent.

## Your capabilities

You control a TODO-driven pipeline with these steps:
1. **todo scan** — Walk a repo and extract every TODO/FIXME/HACK/XXX comment as structured JSON (no LLM needed).
2. **todo scaffold** — Read todo.md, ask Grok 4.1 which files/dirs need to exist, create stubs on disk, update todo.md.
3. **todo plan** — Read todo.md + source context, ask Grok 4.1 to produce a batched, prioritised GamePlan JSON.
4. **todo work** — Execute one GamePlan batch: generate real Rust code, apply hunks/replacements, create backups, write WorkResult JSON.
5. **todo sync** — Apply WorkResult back to todo.md, marking items ✅ / ⚠️ / ❌.
6. **full pipeline** — Runs scan → scaffold → plan in one shot.

## How to respond

- When the user asks to review a repo, summarise the todo.md and suggest which pipeline step to run next.
- When the user asks to plan tasks, suggest running `todo plan` and explain what the GamePlan will contain.
- When the user asks to generate code, suggest running `todo work` for a specific batch.
- Be concise and action-oriented. Offer specific `dispatch` button actions the user can click.
- If you want to suggest a pipeline action, end your reply with a JSON block like:
  ```json
  {{ "suggest_action": {{ "step": "plan", "dry_run": false }} }}
  ```
  The UI will render this as a clickable button.{todo_section}
"#
    )
}

async fn handle_chat(
    State(state): State<WebState>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    // Optionally load todo.md for context
    let todo_context: Option<String> = if let Some(ref repo_id) = req.repo_id {
        if let Ok(repo) = db::get_repository(&state.pool, repo_id).await {
            let base = PathBuf::from(&repo.path);
            let todo_path = if base.join("todo.md").exists() {
                Some(base.join("todo.md"))
            } else if base.join("TODO.md").exists() {
                Some(base.join("TODO.md"))
            } else {
                None
            };
            todo_path.and_then(|p| std::fs::read_to_string(p).ok())
        } else {
            None
        }
    } else {
        None
    };

    // Build full prompt
    let sys = system_prompt(todo_context.as_deref());

    // Build conversation context: system + history + extra context + new message
    let mut full_prompt = format!("{}\n\n", sys);

    if let Some(ref history) = req.history {
        for msg in history.iter().take(20) {
            // cap history at 20 turns
            full_prompt.push_str(&format!("**{}**: {}\n\n", msg.role, msg.content));
        }
    }

    if let Some(ref ctx) = req.context {
        full_prompt.push_str(&format!("## Additional context\n\n{}\n\n", ctx));
    }

    full_prompt.push_str(&format!("**user**: {}", req.message));

    let grok = match state.grok().await {
        Ok(g) => g,
        Err(e) => {
            return api_error(
                StatusCode::SERVICE_UNAVAILABLE,
                &format!("Grok unavailable: {}", e),
            )
        }
    };

    match grok.ask_tracked(&full_prompt, None, "dashboard_chat").await {
        Ok(resp) => {
            let chat_resp = ChatResponse {
                reply: resp.content,
                tokens_used: resp.total_tokens,
                cost_usd: resp.cost_usd,
                model: "grok-4-1-fast-reasoning".to_string(),
            };
            (StatusCode::OK, Json(chat_resp)).into_response()
        }
        Err(e) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Chat failed: {}", e),
        ),
    }
}

// ============================================================================
// Sync result needs serde::Serialize — declare it here since SyncResult is
// in our own crate and we can verify it derives Serialize
// ============================================================================

// ============================================================================
// Job history
// ============================================================================

async fn handle_list_jobs(State(state): State<WebState>) -> impl IntoResponse {
    let jobs = state.jobs.read().await;
    let mut list: Vec<&JobRecord> = jobs.values().collect();
    // Most recent first
    list.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    let list: Vec<&JobRecord> = list.into_iter().take(50).collect();
    Json(serde_json::json!({ "jobs": list })).into_response()
}

async fn handle_get_job(
    State(state): State<WebState>,
    AxumPath(job_id): AxumPath<String>,
) -> impl IntoResponse {
    let jobs = state.jobs.read().await;
    match jobs.get(&job_id) {
        Some(job) => Json(serde_json::json!({ "job": job })).into_response(),
        None => api_error(StatusCode::NOT_FOUND, "Job not found"),
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn api_error(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

fn resolve_todo_md(repo_path: &PathBuf) -> Result<PathBuf> {
    if repo_path.join("todo.md").exists() {
        Ok(repo_path.join("todo.md"))
    } else if repo_path.join("TODO.md").exists() {
        Ok(repo_path.join("TODO.md"))
    } else {
        Err(AuditError::other(format!(
            "No todo.md found in {}",
            repo_path.display()
        )))
    }
}

fn infer_repo_name(url: &str) -> String {
    url.trim_end_matches('/')
        .split('/')
        .last()
        .unwrap_or("unknown")
        .trim_end_matches(".git")
        .to_string()
}

fn is_allowed_git_url(url: &str) -> bool {
    // Allow HTTPS GitHub, GitLab, Bitbucket, Codeberg
    let allowed_hosts = ["github.com", "gitlab.com", "bitbucket.org", "codeberg.org"];
    if let Ok(parsed) = url::Url::parse(url) {
        if parsed.scheme() != "https" {
            return false;
        }
        if let Some(host) = parsed.host_str() {
            return allowed_hosts
                .iter()
                .any(|h| host == *h || host.ends_with(&format!(".{}", h)));
        }
    }
    false
}

// The skipped_items field on GamePlan is Vec<String> — confirm display works:
// (used in format! above via .len() only, so no Display needed)
