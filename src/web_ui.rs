//! Web UI module for Rustassistant dashboard
//!
//! Provides HTML templates and handlers for the web interface using Askama and HTMX.

use crate::db::Database;
use askama_axum::Template;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Application state for web UI
#[derive(Clone)]
pub struct WebAppState {
    pub db: Database,
}

impl WebAppState {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

/// Dashboard statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_notes: i64,
    pub total_repos: i64,
    pub total_cost: f64,
    pub cache_hit_rate: i64,
    pub cost_today: f64,
    pub cost_7d: f64,
    pub cost_30d: f64,
    pub estimated_savings: f64,
}

impl DashboardStats {
    pub fn avg_daily_cost(&self) -> f64 {
        self.cost_30d / 30.0
    }

    pub fn projected_monthly(&self) -> f64 {
        self.cost_today * 30.0
    }

    pub fn cost_without_cache(&self) -> f64 {
        if self.cache_hit_rate > 0 {
            self.cost_30d / (1.0 - (self.cache_hit_rate as f64 / 100.0))
        } else {
            self.cost_30d
        }
    }

    // Currency formatting helpers for templates
    pub fn total_cost_fmt(&self) -> String {
        format!("{:.2}", self.total_cost)
    }

    pub fn cost_today_fmt(&self) -> String {
        format!("{:.2}", self.cost_today)
    }

    pub fn cost_7d_fmt(&self) -> String {
        format!("{:.2}", self.cost_7d)
    }

    pub fn cost_30d_fmt(&self) -> String {
        format!("{:.2}", self.cost_30d)
    }

    pub fn estimated_savings_fmt(&self) -> String {
        format!("{:.2}", self.estimated_savings)
    }

    pub fn avg_daily_cost_fmt(&self) -> String {
        format!("{:.2}", self.avg_daily_cost())
    }
}

/// Recent note for dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentNote {
    pub id: i64,
    pub content: String,
    pub status: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

/// Activity item for dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem {
    pub activity_type: String,
    pub description: String,
    pub timestamp: String,
    pub cost: Option<f64>,
}

impl ActivityItem {
    pub fn cost_fmt(&self) -> String {
        match self.cost {
            Some(c) => format!("{:.4}", c),
            None => "0.00".to_string(),
        }
    }
}

/// Next action recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextAction {
    pub description: String,
    pub action_text: String,
    pub action_url: String,
}

/// Dashboard page template
#[derive(Template)]
#[template(path = "pages/dashboard.html")]
pub struct DashboardTemplate {
    pub page: String,
    pub stats: DashboardStats,
    pub recent_notes: Vec<RecentNote>,
    pub recent_activity: Vec<ActivityItem>,
    pub next_action: Option<NextAction>,
}

/// Notes list page template
#[derive(Template)]
#[template(path = "pages/notes.html")]
pub struct NotesTemplate {
    pub page: String,
    pub notes: Vec<RecentNote>,
    pub total: i64,
}

/// Repositories list page template
#[derive(Template)]
#[template(path = "pages/repos.html")]
pub struct ReposTemplate {
    pub page: String,
    pub repos: Vec<RepoItem>,
}

/// Repository item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoItem {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub created_at: String,
}

/// Costs page template
#[derive(Template)]
#[template(path = "pages/costs.html")]
pub struct CostsTemplate {
    pub page: String,
    pub stats: DashboardStats,
    pub recent_operations: Vec<LlmOperation>,
}

/// LLM operation for costs page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmOperation {
    pub operation: String,
    pub model: String,
    pub tokens: i64,
    pub cost: f64,
    pub timestamp: String,
}

impl LlmOperation {
    pub fn cost_fmt(&self) -> String {
        format!("{:.4}", self.cost)
    }
}

/// Analyze page template
#[derive(Template)]
#[template(path = "pages/analyze.html")]
pub struct AnalyzeTemplate {
    pub page: String,
    pub repos: Vec<RepoItem>,
}

/// Dashboard handler
pub async fn dashboard_handler(
    State(state): State<Arc<WebAppState>>,
) -> Result<impl IntoResponse, AppError> {
    // Get statistics
    let total_notes = state.db.count_notes().await.unwrap_or(0);
    let total_repos = state.db.count_repositories().await.unwrap_or(0);
    let total_cost = state.db.get_total_llm_cost().await.unwrap_or(0.0);
    let cost_today = state.db.get_llm_cost_by_period(1).await.unwrap_or(0.0);
    let cost_7d = state.db.get_llm_cost_by_period(7).await.unwrap_or(0.0);
    let cost_30d = state.db.get_llm_cost_by_period(30).await.unwrap_or(0.0);

    // Get actual cache hit rate from database
    let cache_hit_rate = state.db.get_cache_hit_rate().await.unwrap_or(0);

    // Calculate estimated savings based on actual cache hit rate
    let cache_rate_fraction = cache_hit_rate as f64 / 100.0;
    let estimated_savings = cost_30d * cache_rate_fraction;

    let stats = DashboardStats {
        total_notes,
        total_repos,
        total_cost,
        cache_hit_rate,
        cost_today,
        cost_7d,
        cost_30d,
        estimated_savings,
    };

    // Get recent notes
    let notes = state
        .db
        .list_notes(None, None, Some(5))
        .await
        .unwrap_or_default();

    let recent_notes: Vec<RecentNote> = notes
        .into_iter()
        .map(|note| RecentNote {
            id: note.id,
            content: note.content.clone(),
            status: note.status_str(),
            tags: note.tags.clone(),
            created_at: note.created_at_formatted(),
        })
        .collect();

    // Get recent activity (LLM operations)
    let operations = state
        .db
        .get_recent_llm_operations(5)
        .await
        .unwrap_or_default();

    let recent_activity: Vec<ActivityItem> = operations
        .into_iter()
        .map(|op| ActivityItem {
            activity_type: "analysis".to_string(),
            description: format!("{} - {} tokens", op.operation.clone(), op.total_tokens),
            timestamp: op.created_at_formatted(),
            cost: Some(op.estimated_cost_usd),
        })
        .collect();

    // Generate next action recommendation
    let next_action = if total_notes == 0 {
        Some(NextAction {
            description: "Get started by creating your first note!".to_string(),
            action_text: "Create Note".to_string(),
            action_url: "/notes/new".to_string(),
        })
    } else if total_repos == 0 {
        Some(NextAction {
            description: "Add a repository to start analyzing your code.".to_string(),
            action_text: "Add Repository".to_string(),
            action_url: "/repos/new".to_string(),
        })
    } else {
        Some(NextAction {
            description: "Analyze your code for quality insights.".to_string(),
            action_text: "Run Analysis".to_string(),
            action_url: "/analyze".to_string(),
        })
    };

    Ok(DashboardTemplate {
        page: "dashboard".to_string(),
        stats,
        recent_notes,
        recent_activity,
        next_action,
    })
}

/// Notes handler
pub async fn notes_handler(
    State(state): State<Arc<WebAppState>>,
) -> Result<impl IntoResponse, AppError> {
    let notes = state
        .db
        .list_notes(None, None, None)
        .await
        .unwrap_or_default();
    let total = notes.len() as i64;

    let notes_list: Vec<RecentNote> = notes
        .into_iter()
        .map(|note| RecentNote {
            id: note.id,
            content: note.content.clone(),
            status: note.status_str(),
            tags: note.tags.clone(),
            created_at: note.created_at_formatted(),
        })
        .collect();

    Ok(NotesTemplate {
        page: "notes".to_string(),
        notes: notes_list,
        total,
    })
}

/// Repositories handler
pub async fn repos_handler(
    State(state): State<Arc<WebAppState>>,
) -> Result<impl IntoResponse, AppError> {
    let repos = state.db.list_repositories().await.unwrap_or_default();

    let repos_list: Vec<RepoItem> = repos
        .into_iter()
        .map(|repo| RepoItem {
            id: repo.id,
            name: repo.name.clone(),
            path: repo.path.clone(),
            created_at: repo.created_at_formatted(),
        })
        .collect();

    Ok(ReposTemplate {
        page: "repos".to_string(),
        repos: repos_list,
    })
}

/// Costs handler
pub async fn costs_handler(
    State(state): State<Arc<WebAppState>>,
) -> Result<impl IntoResponse, AppError> {
    let total_cost = state.db.get_total_llm_cost().await.unwrap_or(0.0);
    let cost_today = state.db.get_llm_cost_by_period(1).await.unwrap_or(0.0);
    let cost_7d = state.db.get_llm_cost_by_period(7).await.unwrap_or(0.0);
    let cost_30d = state.db.get_llm_cost_by_period(30).await.unwrap_or(0.0);

    let stats = DashboardStats {
        total_notes: 0,
        total_repos: 0,
        total_cost,
        cache_hit_rate: 70,
        cost_today,
        cost_7d,
        cost_30d,
        estimated_savings: cost_30d * 0.7,
    };

    let operations = state
        .db
        .get_recent_llm_operations(20)
        .await
        .unwrap_or_default();

    let recent_operations: Vec<LlmOperation> = operations
        .into_iter()
        .map(|op| LlmOperation {
            operation: op.operation.clone(),
            model: op.model.clone(),
            tokens: op.total_tokens,
            cost: op.estimated_cost_usd,
            timestamp: op.created_at_formatted(),
        })
        .collect();

    Ok(CostsTemplate {
        page: "costs".to_string(),
        stats,
        recent_operations,
    })
}

/// Analyze handler
pub async fn analyze_handler(
    State(state): State<Arc<WebAppState>>,
) -> Result<impl IntoResponse, AppError> {
    let repos = state.db.list_repositories().await.unwrap_or_default();

    let repos_list: Vec<RepoItem> = repos
        .into_iter()
        .map(|repo| RepoItem {
            id: repo.id,
            name: repo.name.clone(),
            path: repo.path.clone(),
            created_at: repo.created_at_formatted(),
        })
        .collect();

    Ok(AnalyzeTemplate {
        page: "analyze".to_string(),
        repos: repos_list,
    })
}

/// Coming soon page template
#[derive(Template)]
#[template(path = "pages/coming_soon.html")]
pub struct ComingSoonTemplate {
    pub page: String,
    pub feature_name: String,
    pub back_url: String,
}

/// Coming soon handler for unimplemented features
pub async fn coming_soon_handler(
    feature: String,
    back_url: String,
) -> Result<impl IntoResponse, AppError> {
    Ok(ComingSoonTemplate {
        page: "coming_soon".to_string(),
        feature_name: feature,
        back_url,
    })
}

/// Notes new handler
pub async fn notes_new_handler() -> Result<impl IntoResponse, AppError> {
    coming_soon_handler("Create Note".to_string(), "/notes".to_string()).await
}

/// Repos new handler
pub async fn repos_new_handler() -> Result<impl IntoResponse, AppError> {
    coming_soon_handler("Add Repository".to_string(), "/repos".to_string()).await
}

/// Error type for web UI
pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Internal server error: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

/// Create web UI router
pub fn create_router(state: WebAppState) -> Router {
    let shared_state = Arc::new(state);

    Router::new()
        .route("/", get(dashboard_handler))
        .route("/notes", get(notes_handler))
        .route("/notes/new", get(notes_new_handler))
        .route("/repos", get(repos_handler))
        .route("/repos/new", get(repos_new_handler))
        .route("/costs", get(costs_handler))
        .route("/analyze", get(analyze_handler))
        .with_state(shared_state)
}
