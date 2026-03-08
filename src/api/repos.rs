// src/api/repos.rs
// STUB: Repo management + chat endpoints for RustAssistant
// TODO: wire AppState to real RepoSyncService and ModelRouter instances
// TODO: add auth middleware (reuse existing API key layer)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::model_router::{CompletionRequest, ModelRouter};
use crate::repo_sync::{RegisteredRepo, RepoSyncService, SyncResult};

// ---------------------------------------------------------------------------
// AppState (extend your existing AppState or create a sub-state)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct RepoAppState {
    pub sync_service: Arc<RwLock<RepoSyncService>>,
    pub model_router: Arc<ModelRouter>,
    // TODO: add redis client for response caching
    // TODO: add ollama_client: Arc<OllamaClient>
    // TODO: add xai_client: Arc<XaiClient>
}

// ---------------------------------------------------------------------------
// Router builder — call this from your main router setup
// ---------------------------------------------------------------------------

pub fn repo_router(state: RepoAppState) -> Router {
    Router::new()
        // Repo management
        .route("/repos", get(list_repos).post(register_repo))
        .route("/repos/:id", get(get_repo).delete(remove_repo))
        .route("/repos/:id/sync", post(sync_repo))
        .route("/repos/:id/context", get(get_repo_context))
        .route("/repos/:id/todos", get(get_repo_todos))
        .route("/repos/:id/symbols", get(get_repo_symbols))
        .route("/repos/:id/tree", get(get_repo_tree))
        // Chat interface
        .route("/chat", post(chat))
        .route("/chat/repos/:id", post(chat_with_repo))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterRepoRequest {
    pub name: String,
    pub local_path: String,
    pub remote_url: Option<String>,
    pub branch: Option<String>,
    /// If true, immediately run a sync after registration
    pub sync_on_register: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct RegisterRepoResponse {
    pub id: String,
    pub name: String,
    pub message: String,
    pub sync_result: Option<SyncResult>,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    /// Optional: inject context from a specific registered repo
    pub repo_id: Option<String>,
    /// If true, force the remote model regardless of task classification
    pub force_remote: Option<bool>,
    /// Conversation history for multi-turn chat
    pub history: Option<Vec<ChatMessage>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String, // "user" | "assistant"
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub reply: String,
    pub task_kind: String,
    pub model_used: String,
    pub used_fallback: bool,
    pub repo_context_injected: bool,
    pub tokens_used: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub code: String,
}

impl ApiError {
    fn not_found(msg: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::NOT_FOUND,
            Json(Self {
                error: msg.into(),
                code: "NOT_FOUND".to_string(),
            }),
        )
    }

    fn internal(msg: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Self {
                error: msg.into(),
                code: "INTERNAL_ERROR".to_string(),
            }),
        )
    }
}

// ---------------------------------------------------------------------------
// Repo handlers
// ---------------------------------------------------------------------------

async fn list_repos(State(state): State<RepoAppState>) -> impl IntoResponse {
    let service = state.sync_service.read().await;
    let repos: Vec<_> = service
        .list_repos()
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "name": r.name,
                "path": r.local_path,
                "branch": r.branch,
                "last_synced": r.last_synced,
                "remote_url": r.remote_url,
            })
        })
        .collect();
    Json(repos)
}

async fn register_repo(
    State(state): State<RepoAppState>,
    Json(req): Json<RegisterRepoRequest>,
) -> impl IntoResponse {
    info!(name = %req.name, path = %req.local_path, "Registering repo via API");

    let mut repo = RegisteredRepo::new(&req.name, &req.local_path);
    if let Some(url) = req.remote_url {
        repo.remote_url = Some(url);
    }
    if let Some(branch) = req.branch {
        repo.branch = branch;
    }

    let mut service = state.sync_service.write().await;
    let id = match service.register(repo).await {
        Ok(id) => id,
        Err(e) => {
            error!(error = %e, "Failed to register repo");
            return ApiError::internal(e.to_string()).into_response();
        }
    };

    // Optionally run immediate sync
    let sync_result = if req.sync_on_register.unwrap_or(false) {
        match service.sync(&id).await {
            Ok(r) => Some(r),
            Err(e) => {
                error!(error = %e, "Sync after register failed");
                None
            }
        }
    } else {
        None
    };

    Json(RegisterRepoResponse {
        id: id.clone(),
        name: req.name,
        message: format!("Repo '{}' registered successfully", id),
        sync_result,
    })
    .into_response()
}

async fn get_repo(State(state): State<RepoAppState>, Path(id): Path<String>) -> impl IntoResponse {
    let service = state.sync_service.read().await;
    match service.get_repo(&id) {
        Some(repo) => Json(serde_json::json!({
            "id": repo.id,
            "name": repo.name,
            "path": repo.local_path,
            "branch": repo.branch,
            "last_synced": repo.last_synced,
            "remote_url": repo.remote_url,
            "cache_dir": repo.cache_dir(),
        }))
        .into_response(),
        None => ApiError::not_found(format!("Repo '{}' not found", id)).into_response(),
    }
}

async fn remove_repo(
    State(state): State<RepoAppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut service = state.sync_service.write().await;
    if service.remove_repo(&id) {
        Json(serde_json::json!({ "message": format!("Repo '{}' removed", id) })).into_response()
    } else {
        ApiError::not_found(format!("Repo '{}' not found", id)).into_response()
    }
}

async fn sync_repo(State(state): State<RepoAppState>, Path(id): Path<String>) -> impl IntoResponse {
    info!(repo = %id, "Manual sync triggered via API");
    let mut service = state.sync_service.write().await;
    match service.sync(&id).await {
        Ok(result) => Json(result).into_response(),
        Err(e) => ApiError::internal(e.to_string()).into_response(),
    }
}

async fn get_repo_context(
    State(state): State<RepoAppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let service = state.sync_service.read().await;
    match service.build_prompt_context(&id).await {
        Ok(ctx) => (StatusCode::OK, ctx).into_response(),
        Err(e) => ApiError::not_found(e.to_string()).into_response(),
    }
}

async fn get_repo_todos(
    State(state): State<RepoAppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    serve_cache_file(&state, &id, |r| r.todos_path()).await
}

async fn get_repo_symbols(
    State(state): State<RepoAppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    serve_cache_file(&state, &id, |r| r.symbols_path()).await
}

async fn get_repo_tree(
    State(state): State<RepoAppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    serve_cache_file(&state, &id, |r| r.tree_path()).await
}

// ---------------------------------------------------------------------------
// Chat handlers
// ---------------------------------------------------------------------------

async fn chat(
    State(state): State<RepoAppState>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    handle_chat(state, req, None).await
}

async fn chat_with_repo(
    State(state): State<RepoAppState>,
    Path(repo_id): Path<String>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    handle_chat(state, req, Some(repo_id)).await
}

async fn handle_chat(
    state: RepoAppState,
    req: ChatRequest,
    repo_id_override: Option<String>,
) -> impl IntoResponse {
    let effective_repo_id = repo_id_override.or(req.repo_id.clone());

    // 1. Classify and route
    let (task_kind, target) = state.model_router.route_prompt(&req.message);

    // 2. Build repo context if a repo is specified
    let (repo_context, context_injected) = if let Some(ref rid) = effective_repo_id {
        let service = state.sync_service.read().await;
        match service.build_prompt_context(rid).await {
            Ok(ctx) => (Some(ctx), true),
            Err(e) => {
                error!(repo = %rid, error = %e, "Failed to build repo context");
                (None, false)
            }
        }
    } else {
        (None, false)
    };

    // 3. Build completion request
    let completion_req = CompletionRequest::for_stub(&req.message, repo_context);

    // 4. Call model
    // TODO: replace this stub with real OllamaClient / XaiClient dispatch
    // TODO: check Redis cache before calling model (key: hash of prompt + repo_id)
    // TODO: cache response in Redis after successful call
    let (reply, model_used, used_fallback) = call_model_stub(&completion_req, &target).await;

    Json(ChatResponse {
        reply,
        task_kind: format!("{:?}", task_kind),
        model_used,
        used_fallback,
        repo_context_injected: context_injected,
        tokens_used: None, // TODO: populate from model response
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// Placeholder model call — replace with real clients
// ---------------------------------------------------------------------------

// STUB: generated by rustassistant
// TODO: replace with actual OllamaClient::complete() and XaiClient::complete()
async fn call_model_stub(
    req: &CompletionRequest,
    target: &crate::model_router::ModelTarget,
) -> (String, String, bool) {
    use crate::model_router::ModelTarget;

    let model_name = match target {
        ModelTarget::Local { model, .. } => model.clone(),
        ModelTarget::Remote { model, .. } => model.clone(),
    };

    // TODO: actual HTTP call to Ollama or xAI API
    let reply = format!(
        "// STUB: model call not yet implemented\n// Target: {}\n// Prompt: {}...",
        model_name,
        &req.user_prompt.chars().take(80).collect::<String>()
    );

    (reply, model_name, false)
}

// ---------------------------------------------------------------------------
// Helper: serve a .rustassistant/ cache file as JSON or text
// ---------------------------------------------------------------------------

async fn serve_cache_file<F>(state: &RepoAppState, repo_id: &str, path_fn: F) -> impl IntoResponse
where
    F: FnOnce(&RegisteredRepo) -> std::path::PathBuf,
{
    let service = state.sync_service.read().await;
    let repo = match service.get_repo(repo_id) {
        Some(r) => r,
        None => {
            return ApiError::not_found(format!("Repo '{}' not found", repo_id)).into_response()
        }
    };

    let file_path = path_fn(repo);
    match tokio::fs::read_to_string(&file_path).await {
        Ok(content) => (StatusCode::OK, content).into_response(),
        Err(_) => ApiError::not_found(format!(
            "Cache not found at {:?} — run /repos/{}/sync first",
            file_path, repo_id
        ))
        .into_response(),
    }
}
