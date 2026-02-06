//! Web UI module for Rustassistant dashboard
//!
//! Provides HTML templates and handlers for the web interface.
//! Features: repository management, queue operations, auto-scanner control.

use crate::db::{add_repository, remove_repository, Database, Repository};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Form, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

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

// ============================================================================
// Data Models
// ============================================================================

/// Dashboard statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_repos: i64,
    pub auto_scan_enabled: i64,
    pub queue_pending: i64,
    pub queue_processing: i64,
    pub queue_completed: i64,
    pub queue_failed: i64,
}

/// Repository item for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub status: String,
    pub auto_scan_enabled: bool,
    pub scan_interval_minutes: i64,
    pub last_scan_check: Option<String>,
    pub created_at: String,
}

impl From<Repository> for RepoItem {
    fn from(repo: Repository) -> Self {
        Self {
            id: repo.id,
            name: repo.name,
            path: repo.path,
            status: repo.status,
            auto_scan_enabled: repo.auto_scan_enabled != 0,
            scan_interval_minutes: repo.scan_interval_minutes,
            last_scan_check: repo.last_scan_check.map(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            }),
            created_at: chrono::DateTime::from_timestamp(repo.created_at, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        }
    }
}

/// Queue item for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItemDisplay {
    pub id: String,
    pub source: String,
    pub stage: String,
    pub priority: String,
    pub content: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub processing_started_at: Option<String>,
}

/// Form data for adding repository
#[derive(Debug, Deserialize)]
pub struct AddRepoForm {
    pub path: String,
    pub name: String,
}

/// Form data for updating repository
#[derive(Debug, Deserialize)]
pub struct UpdateRepoForm {
    pub auto_scan_enabled: Option<String>,
    pub scan_interval_minutes: Option<i64>,
}

/// Response for API calls
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[allow(dead_code)]
impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg),
        }
    }
}

// ============================================================================
// HTML Templates
// ============================================================================

fn render_dashboard_page(stats: DashboardStats) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>RustAssistant - Dashboard</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif; background: #0f172a; color: #e2e8f0; }}
        .container {{ max-width: 1200px; margin: 0 auto; padding: 2rem; }}
        header {{ background: #1e293b; padding: 1.5rem; margin-bottom: 2rem; border-radius: 8px; box-shadow: 0 4px 6px rgba(0,0,0,0.3); }}
        h1 {{ color: #38bdf8; font-size: 2rem; margin-bottom: 0.5rem; }}
        .subtitle {{ color: #94a3b8; font-size: 0.9rem; }}
        nav {{ display: flex; gap: 1rem; margin-top: 1rem; flex-wrap: wrap; }}
        nav a {{ background: #334155; color: #e2e8f0; padding: 0.5rem 1rem; border-radius: 6px; text-decoration: none; transition: all 0.3s; }}
        nav a:hover {{ background: #475569; transform: translateY(-2px); }}
        nav a.active {{ background: #0ea5e9; color: white; }}
        .stats-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin-bottom: 2rem; }}
        .stat-card {{ background: #1e293b; padding: 1.5rem; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.2); border-left: 4px solid #38bdf8; }}
        .stat-card h3 {{ color: #94a3b8; font-size: 0.85rem; font-weight: 500; margin-bottom: 0.5rem; text-transform: uppercase; }}
        .stat-card .value {{ color: #38bdf8; font-size: 2rem; font-weight: bold; }}
        .stat-card.success {{ border-left-color: #22c55e; }}
        .stat-card.success .value {{ color: #22c55e; }}
        .stat-card.warning {{ border-left-color: #f59e0b; }}
        .stat-card.warning .value {{ color: #f59e0b; }}
        .stat-card.danger {{ border-left-color: #ef4444; }}
        .stat-card.danger .value {{ color: #ef4444; }}
        .action-section {{ background: #1e293b; padding: 2rem; border-radius: 8px; margin-bottom: 2rem; }}
        .action-section h2 {{ color: #e2e8f0; margin-bottom: 1rem; }}
        .action-buttons {{ display: flex; gap: 1rem; flex-wrap: wrap; }}
        .btn {{ padding: 0.75rem 1.5rem; border-radius: 6px; border: none; cursor: pointer; font-size: 1rem; font-weight: 500; transition: all 0.3s; text-decoration: none; display: inline-block; }}
        .btn-primary {{ background: #0ea5e9; color: white; }}
        .btn-primary:hover {{ background: #0284c7; transform: translateY(-2px); }}
        .btn-success {{ background: #22c55e; color: white; }}
        .btn-success:hover {{ background: #16a34a; transform: translateY(-2px); }}
        .btn-danger {{ background: #ef4444; color: white; }}
        .btn-danger:hover {{ background: #dc2626; transform: translateY(-2px); }}
        footer {{ margin-top: 3rem; text-align: center; color: #64748b; font-size: 0.9rem; }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <p class="subtitle">Developer Workflow Management System</p>
            <nav>
                <a href="/dashboard" class="active">Dashboard</a>
                <a href="/repos">Repositories</a>
                <a href="/queue">Queue</a>
                <a href="/scanner">Auto-Scanner</a>
            </nav>
        </header>

        <div class="stats-grid">
            <div class="stat-card">
                <h3>Total Repositories</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card success">
                <h3>Auto-Scan Enabled</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card warning">
                <h3>Queue Pending</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card">
                <h3>Queue Processing</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card success">
                <h3>Completed</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card danger">
                <h3>Failed</h3>
                <div class="value">{}</div>
            </div>
        </div>

        <div class="action-section">
            <h2>Quick Actions</h2>
            <div class="action-buttons">
                <a href="/repos/add" class="btn btn-primary">+ Add Repository</a>
                <a href="/queue" class="btn btn-success">View Queue</a>
                <a href="/scanner" class="btn btn-primary">Scanner Settings</a>
            </div>
        </div>

        <footer>
            <p>RustAssistant v0.1.0 | Powered by Rust & Axum</p>
        </footer>
    </div>
</body>
</html>"#,
        stats.total_repos,
        stats.auto_scan_enabled,
        stats.queue_pending,
        stats.queue_processing,
        stats.queue_completed,
        stats.queue_failed
    )
}

fn render_repos_page(repos: Vec<RepoItem>) -> String {
    let repos_html = if repos.is_empty() {
        r#"<div style="text-align: center; padding: 3rem; color: #64748b;">
            <p style="font-size: 1.2rem; margin-bottom: 1rem;">No repositories yet</p>
            <a href="/repos/add" class="btn btn-primary">Add Your First Repository</a>
        </div>"#
            .to_string()
    } else {
        repos
            .iter()
            .map(|repo| {
                let scan_status = if repo.auto_scan_enabled {
                    format!("✅ Enabled ({}min)", repo.scan_interval_minutes)
                } else {
                    "❌ Disabled".to_string()
                };
                let last_scan = repo
                    .last_scan_check
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("Never");

                format!(
                    r#"<div class="repo-card">
                    <div class="repo-header">
                        <h3>{}</h3>
                        <span class="repo-status {}">{}</span>
                    </div>
                    <div class="repo-info">
                        <p><strong>Path:</strong> {}</p>
                        <p><strong>Auto-Scan:</strong> {}</p>
                        <p><strong>Last Scan:</strong> {}</p>
                        <p><strong>Created:</strong> {}</p>
                    </div>
                    <div class="repo-actions">
                        <a href="/repos/{}/toggle-scan" class="btn-small btn-primary">Toggle Scan</a>
                        <a href="/repos/{}/scan-now" class="btn-small btn-success">Scan Now</a>
                        <a href="/repos/{}/configure" class="btn-small btn-secondary">Configure</a>
                        <a href="/repos/{}/delete" class="btn-small btn-danger" onclick="return confirm('Delete this repository?')">Delete</a>
                    </div>
                </div>"#,
                    repo.name,
                    if repo.auto_scan_enabled { "status-enabled" } else { "status-disabled" },
                    repo.status,
                    repo.path,
                    scan_status,
                    last_scan,
                    repo.created_at,
                    repo.id,
                    repo.id,
                    repo.id,
                    repo.id
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Repositories - RustAssistant</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #e2e8f0; }}
        .container {{ max-width: 1200px; margin: 0 auto; padding: 2rem; }}
        header {{ background: #1e293b; padding: 1.5rem; margin-bottom: 2rem; border-radius: 8px; }}
        h1 {{ color: #38bdf8; font-size: 2rem; margin-bottom: 0.5rem; }}
        nav {{ display: flex; gap: 1rem; margin-top: 1rem; flex-wrap: wrap; }}
        nav a {{ background: #334155; color: #e2e8f0; padding: 0.5rem 1rem; border-radius: 6px; text-decoration: none; transition: all 0.3s; }}
        nav a:hover {{ background: #475569; }}
        nav a.active {{ background: #0ea5e9; color: white; }}
        .page-header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 2rem; }}
        .page-header h2 {{ color: #e2e8f0; }}
        .repo-card {{ background: #1e293b; padding: 1.5rem; border-radius: 8px; margin-bottom: 1rem; box-shadow: 0 2px 4px rgba(0,0,0,0.2); }}
        .repo-header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }}
        .repo-header h3 {{ color: #38bdf8; font-size: 1.3rem; }}
        .repo-status {{ padding: 0.25rem 0.75rem; border-radius: 4px; font-size: 0.85rem; font-weight: 500; }}
        .status-enabled {{ background: #22c55e; color: white; }}
        .status-disabled {{ background: #64748b; color: white; }}
        .repo-info {{ margin-bottom: 1rem; }}
        .repo-info p {{ color: #94a3b8; margin-bottom: 0.5rem; }}
        .repo-info strong {{ color: #e2e8f0; }}
        .repo-actions {{ display: flex; gap: 0.5rem; flex-wrap: wrap; }}
        .btn, .btn-small {{ padding: 0.75rem 1.5rem; border-radius: 6px; border: none; cursor: pointer; font-size: 1rem; font-weight: 500; transition: all 0.3s; text-decoration: none; display: inline-block; }}
        .btn-small {{ padding: 0.5rem 1rem; font-size: 0.9rem; }}
        .btn-primary {{ background: #0ea5e9; color: white; }}
        .btn-primary:hover {{ background: #0284c7; }}
        .btn-success {{ background: #22c55e; color: white; }}
        .btn-success:hover {{ background: #16a34a; }}
        .btn-secondary {{ background: #64748b; color: white; }}
        .btn-secondary:hover {{ background: #475569; }}
        .btn-danger {{ background: #ef4444; color: white; }}
        .btn-danger:hover {{ background: #dc2626; }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos" class="active">Repositories</a>
                <a href="/queue">Queue</a>
                <a href="/scanner">Auto-Scanner</a>
            </nav>
        </header>

        <div class="page-header">
            <h2>Repositories</h2>
            <a href="/repos/add" class="btn btn-primary">+ Add Repository</a>
        </div>

        <div class="repos-list">
            {}
        </div>
    </div>
</body>
</html>"#,
        repos_html
    )
}

fn render_add_repo_page() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Add Repository - RustAssistant</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #e2e8f0; }
        .container { max-width: 800px; margin: 0 auto; padding: 2rem; }
        header { background: #1e293b; padding: 1.5rem; margin-bottom: 2rem; border-radius: 8px; }
        h1 { color: #38bdf8; font-size: 2rem; margin-bottom: 0.5rem; }
        nav { display: flex; gap: 1rem; margin-top: 1rem; flex-wrap: wrap; }
        nav a { background: #334155; color: #e2e8f0; padding: 0.5rem 1rem; border-radius: 6px; text-decoration: none; transition: all 0.3s; }
        nav a:hover { background: #475569; }
        .form-container { background: #1e293b; padding: 2rem; border-radius: 8px; }
        .form-group { margin-bottom: 1.5rem; }
        label { display: block; color: #94a3b8; margin-bottom: 0.5rem; font-weight: 500; }
        input, select { width: 100%; padding: 0.75rem; border-radius: 6px; border: 1px solid #334155; background: #0f172a; color: #e2e8f0; font-size: 1rem; }
        input:focus, select:focus { outline: none; border-color: #0ea5e9; }
        .form-actions { display: flex; gap: 1rem; margin-top: 2rem; }
        .btn { padding: 0.75rem 1.5rem; border-radius: 6px; border: none; cursor: pointer; font-size: 1rem; font-weight: 500; transition: all 0.3s; text-decoration: none; display: inline-block; }
        .btn-primary { background: #0ea5e9; color: white; }
        .btn-primary:hover { background: #0284c7; }
        .btn-secondary { background: #64748b; color: white; }
        .btn-secondary:hover { background: #475569; }
        .help-text { color: #64748b; font-size: 0.9rem; margin-top: 0.25rem; }
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos">Repositories</a>
                <a href="/queue">Queue</a>
                <a href="/scanner">Auto-Scanner</a>
            </nav>
        </header>

        <div class="form-container">
            <h2 style="color: #e2e8f0; margin-bottom: 1.5rem;">Add Repository</h2>
            <form method="POST" action="/repos/add">
                <div class="form-group">
                    <label for="path">Repository Path</label>
                    <input type="text" id="path" name="path" required placeholder="/home/user/github/myproject">
                    <p class="help-text">Full path to the repository on your system</p>
                </div>
                <div class="form-group">
                    <label for="name">Repository Name</label>
                    <input type="text" id="name" name="name" required placeholder="myproject">
                    <p class="help-text">A friendly name for this repository</p>
                </div>
                <div class="form-actions">
                    <button type="submit" class="btn btn-primary">Add Repository</button>
                    <a href="/repos" class="btn btn-secondary">Cancel</a>
                </div>
            </form>
        </div>
    </div>
</body>
</html>"#.to_string()
}

fn render_queue_page(items: Vec<QueueItemDisplay>) -> String {
    let items_html = if items.is_empty() {
        r#"<div style="text-align: center; padding: 3rem; color: #64748b;">
            <p style="font-size: 1.2rem;">Queue is empty</p>
        </div>"#
            .to_string()
    } else {
        items
            .iter()
            .map(|item| {
                let error_html = if let Some(err) = &item.error_message {
                    format!(r#"<div class="error-message">❌ Error: {}</div>"#, err)
                } else {
                    String::new()
                };

                let copy_btn = format!(
                    r#"<button class="btn-small btn-primary" onclick="copyToClipboard('{}')">📋 Copy for IDE</button>"#,
                    item.content.replace('\'', "\\'").replace('\n', "\\n")
                );

                format!(
                    r#"<div class="queue-item stage-{}">
                    <div class="queue-header">
                        <span class="queue-id">{}</span>
                        <span class="queue-stage">{}</span>
                        <span class="queue-priority priority-{}">{}</span>
                    </div>
                    <div class="queue-content">
                        <pre>{}</pre>
                    </div>
                    {}
                    <div class="queue-meta">
                        <span>Source: {}</span>
                        <span>Created: {}</span>
                    </div>
                    <div class="queue-actions">
                        {}
                        <a href="/queue/{}/delete" class="btn-small btn-danger">Delete</a>
                    </div>
                </div>"#,
                    item.stage.to_lowercase(),
                    &item.id[..8],
                    item.stage,
                    item.priority.to_lowercase(),
                    item.priority,
                    item.content,
                    error_html,
                    item.source,
                    item.created_at,
                    copy_btn,
                    item.id
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Queue - RustAssistant</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #e2e8f0; }}
        .container {{ max-width: 1200px; margin: 0 auto; padding: 2rem; }}
        header {{ background: #1e293b; padding: 1.5rem; margin-bottom: 2rem; border-radius: 8px; }}
        h1 {{ color: #38bdf8; font-size: 2rem; margin-bottom: 0.5rem; }}
        nav {{ display: flex; gap: 1rem; margin-top: 1rem; flex-wrap: wrap; }}
        nav a {{ background: #334155; color: #e2e8f0; padding: 0.5rem 1rem; border-radius: 6px; text-decoration: none; transition: all 0.3s; }}
        nav a:hover {{ background: #475569; }}
        nav a.active {{ background: #0ea5e9; color: white; }}
        .page-header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 2rem; }}
        .queue-item {{ background: #1e293b; padding: 1.5rem; border-radius: 8px; margin-bottom: 1rem; border-left: 4px solid #64748b; }}
        .queue-item.stage-pending {{ border-left-color: #f59e0b; }}
        .queue-item.stage-processing {{ border-left-color: #0ea5e9; }}
        .queue-item.stage-completed {{ border-left-color: #22c55e; }}
        .queue-item.stage-failed {{ border-left-color: #ef4444; }}
        .queue-header {{ display: flex; gap: 1rem; margin-bottom: 1rem; align-items: center; flex-wrap: wrap; }}
        .queue-id {{ font-family: monospace; color: #94a3b8; }}
        .queue-stage {{ padding: 0.25rem 0.75rem; border-radius: 4px; font-size: 0.85rem; font-weight: 500; background: #334155; }}
        .queue-priority {{ padding: 0.25rem 0.75rem; border-radius: 4px; font-size: 0.85rem; font-weight: 500; }}
        .priority-critical {{ background: #dc2626; color: white; }}
        .priority-high {{ background: #ef4444; color: white; }}
        .priority-normal {{ background: #f59e0b; color: white; }}
        .priority-low {{ background: #64748b; color: white; }}
        .priority-background {{ background: #475569; color: white; }}
        .queue-content {{ background: #0f172a; padding: 1rem; border-radius: 6px; margin-bottom: 1rem; }}
        .queue-content pre {{ color: #e2e8f0; white-space: pre-wrap; word-wrap: break-word; }}
        .error-message {{ background: #7f1d1d; color: #fecaca; padding: 0.75rem; border-radius: 6px; margin-bottom: 1rem; }}
        .queue-meta {{ color: #94a3b8; font-size: 0.9rem; margin-bottom: 1rem; }}
        .queue-meta span {{ margin-right: 1rem; }}
        .queue-actions {{ display: flex; gap: 0.5rem; flex-wrap: wrap; }}
        .btn, .btn-small {{ padding: 0.75rem 1.5rem; border-radius: 6px; border: none; cursor: pointer; font-size: 1rem; font-weight: 500; transition: all 0.3s; text-decoration: none; display: inline-block; }}
        .btn-small {{ padding: 0.5rem 1rem; font-size: 0.9rem; }}
        .btn-primary {{ background: #0ea5e9; color: white; }}
        .btn-primary:hover {{ background: #0284c7; }}
        .btn-danger {{ background: #ef4444; color: white; }}
        .btn-danger:hover {{ background: #dc2626; }}
        .toast {{ position: fixed; top: 2rem; right: 2rem; background: #22c55e; color: white; padding: 1rem 1.5rem; border-radius: 6px; box-shadow: 0 4px 6px rgba(0,0,0,0.3); display: none; }}
    </style>
    <script>
        function copyToClipboard(text) {{
            navigator.clipboard.writeText(text).then(() => {{
                const toast = document.getElementById('toast');
                toast.style.display = 'block';
                setTimeout(() => {{ toast.style.display = 'none'; }}, 2000);
            }});
        }}
    </script>
</head>
<body>
    <div id="toast" class="toast">✓ Copied to clipboard!</div>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos">Repositories</a>
                <a href="/queue" class="active">Queue</a>
                <a href="/scanner">Auto-Scanner</a>
            </nav>
        </header>

        <div class="page-header">
            <h2 style="color: #e2e8f0;">Task Queue</h2>
        </div>

        <div class="queue-list">
            {}
        </div>
    </div>
</body>
</html>"#,
        items_html
    )
}

// ============================================================================
// Route Handlers
// ============================================================================

/// Dashboard handler
pub async fn dashboard_handler(State(state): State<Arc<WebAppState>>) -> impl IntoResponse {
    match get_dashboard_stats(&state.db).await {
        Ok(stats) => Html(render_dashboard_page(stats)),
        Err(e) => {
            error!("Failed to get dashboard stats: {}", e);
            Html(format!("<h1>Error loading dashboard: {}</h1>", e))
        }
    }
}

/// List repositories handler
pub async fn repos_handler(State(state): State<Arc<WebAppState>>) -> impl IntoResponse {
    match state.db.list_repositories().await {
        Ok(repos) => {
            let repo_items: Vec<RepoItem> = repos.into_iter().map(|r| r.into()).collect();
            Html(render_repos_page(repo_items))
        }
        Err(e) => {
            error!("Failed to list repositories: {}", e);
            Html(format!("<h1>Error loading repositories: {}</h1>", e))
        }
    }
}

/// Add repository form handler
pub async fn add_repo_form_handler() -> impl IntoResponse {
    Html(render_add_repo_page())
}

/// Add repository POST handler
pub async fn add_repo_handler(
    State(state): State<Arc<WebAppState>>,
    Form(form): Form<AddRepoForm>,
) -> impl IntoResponse {
    match add_repository(&state.db.pool, &form.path, &form.name).await {
        Ok(_) => {
            info!("Added repository: {} at {}", form.name, form.path);
            (
                StatusCode::SEE_OTHER,
                [("Location", "/repos")],
                "Redirecting...",
            )
        }
        Err(e) => {
            error!("Failed to add repository: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Location", "/repos")],
                "Error adding repository",
            )
        }
    }
}

/// Toggle auto-scan for repository
pub async fn toggle_scan_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match toggle_repo_autoscan(&state.db, &id).await {
        Ok(_) => (StatusCode::SEE_OTHER, [("Location", "/repos")], "OK"),
        Err(e) => {
            error!("Failed to toggle scan: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Location", "/repos")],
                "Error",
            )
        }
    }
}

/// Delete repository handler
pub async fn delete_repo_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match remove_repository(&state.db.pool, &id).await {
        Ok(_) => {
            info!("Deleted repository: {}", id);
            (StatusCode::SEE_OTHER, [("Location", "/repos")], "OK")
        }
        Err(e) => {
            error!("Failed to delete repository: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Location", "/repos")],
                "Error",
            )
        }
    }
}

/// Queue list handler
pub async fn queue_handler(State(state): State<Arc<WebAppState>>) -> impl IntoResponse {
    match get_queue_items(&state.db).await {
        Ok(items) => Html(render_queue_page(items)),
        Err(e) => {
            error!("Failed to get queue items: {}", e);
            Html(format!("<h1>Error loading queue: {}</h1>", e))
        }
    }
}

/// Delete queue item handler
pub async fn delete_queue_item_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match delete_queue_item(&state.db, &id).await {
        Ok(_) => (StatusCode::SEE_OTHER, [("Location", "/queue")], "OK"),
        Err(e) => {
            error!("Failed to delete queue item: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Location", "/queue")],
                "Error",
            )
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

async fn get_dashboard_stats(db: &Database) -> anyhow::Result<DashboardStats> {
    let total_repos = db.count_repositories().await.unwrap_or(0);

    // Count auto-scan enabled repos
    let auto_scan_enabled = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM repositories WHERE auto_scan_enabled = 1",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap_or(0);

    // Get queue stats
    let queue_pending = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM queue_items WHERE stage = 'pending' OR stage = 'inbox'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap_or(0);

    let queue_processing =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM queue_items WHERE stage = 'processing'")
            .fetch_one(&db.pool)
            .await
            .unwrap_or(0);

    let queue_completed = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM queue_items WHERE stage = 'completed' OR stage = 'done'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap_or(0);

    let queue_failed =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM queue_items WHERE stage = 'failed'")
            .fetch_one(&db.pool)
            .await
            .unwrap_or(0);

    Ok(DashboardStats {
        total_repos,
        auto_scan_enabled,
        queue_pending,
        queue_processing,
        queue_completed,
        queue_failed,
    })
}

async fn toggle_repo_autoscan(db: &Database, id: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE repositories SET auto_scan_enabled = NOT auto_scan_enabled, updated_at = ? WHERE id = ?",
    )
    .bind(chrono::Utc::now().timestamp())
    .bind(id)
    .execute(&db.pool)
    .await?;

    Ok(())
}

async fn get_queue_items(db: &Database) -> anyhow::Result<Vec<QueueItemDisplay>> {
    #[derive(sqlx::FromRow)]
    struct QueueItemRaw {
        id: String,
        source: String,
        stage: String,
        priority: i32,
        content: String,
        last_error: Option<String>,
        created_at: i64,
        processed_at: Option<i64>,
    }

    let items = sqlx::query_as::<_, QueueItemRaw>(
        "SELECT id, source, stage, priority, content, last_error, created_at, processed_at
         FROM queue_items
         ORDER BY priority ASC, created_at DESC
         LIMIT 100",
    )
    .fetch_all(&db.pool)
    .await?;

    Ok(items
        .into_iter()
        .map(|item| {
            let priority_label = match item.priority {
                1 => "Critical".to_string(),
                2 => "High".to_string(),
                3 => "Normal".to_string(),
                4 => "Low".to_string(),
                5 => "Background".to_string(),
                other => format!("P{}", other),
            };
            QueueItemDisplay {
                id: item.id,
                source: item.source,
                stage: item.stage,
                priority: priority_label,
                content: item.content,
                error_message: item.last_error,
                created_at: chrono::DateTime::from_timestamp(item.created_at, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                processing_started_at: item.processed_at.map(|ts| {
                    chrono::DateTime::from_timestamp(ts, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                }),
            }
        })
        .collect())
}

async fn delete_queue_item(db: &Database, id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM queue_items WHERE id = ?")
        .bind(id)
        .execute(&db.pool)
        .await?;
    Ok(())
}

// ============================================================================
// Router
// ============================================================================

/// Create web UI router
/// Scanner page data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScannerRepoItem {
    id: String,
    name: String,
    path: String,
    auto_scan_enabled: bool,
    scan_interval_minutes: i64,
    last_scan_check: Option<String>,
    last_commit_hash: Option<String>,
    last_analyzed: Option<String>,
}

/// Scanner page handler
pub async fn scanner_handler(State(state): State<Arc<WebAppState>>) -> impl IntoResponse {
    match get_scanner_repos(&state.db).await {
        Ok(repos) => Html(render_scanner_page(repos)),
        Err(e) => {
            error!("Failed to get scanner data: {}", e);
            Html(format!("<h1>Error loading scanner: {}</h1>", e))
        }
    }
}

/// Force scan handler
pub async fn force_scan_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match sqlx::query("UPDATE repositories SET last_scan_check = NULL WHERE id = ?")
        .bind(&id)
        .execute(&state.db.pool)
        .await
    {
        Ok(_) => {
            info!("Forced scan for repo {}", id);
            (StatusCode::SEE_OTHER, [("Location", "/scanner")], "OK")
        }
        Err(e) => {
            error!("Failed to force scan: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Location", "/scanner")],
                "Error",
            )
        }
    }
}

async fn get_scanner_repos(db: &Database) -> anyhow::Result<Vec<ScannerRepoItem>> {
    let repos = db.list_repositories().await?;
    Ok(repos
        .into_iter()
        .map(|r| ScannerRepoItem {
            id: r.id,
            name: r.name,
            path: r.path,
            auto_scan_enabled: r.auto_scan_enabled != 0,
            scan_interval_minutes: r.scan_interval_minutes,
            last_scan_check: r.last_scan_check.map(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            }),
            last_commit_hash: r.last_commit_hash.map(|h| {
                if h.len() > 8 {
                    h[..8].to_string()
                } else {
                    h
                }
            }),
            last_analyzed: r.last_analyzed.map(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            }),
        })
        .collect())
}

fn render_scanner_page(repos: Vec<ScannerRepoItem>) -> String {
    let repos_html = if repos.is_empty() {
        r#"<div style="text-align: center; padding: 3rem; color: #64748b;">
            <p style="font-size: 1.2rem;">No repositories configured</p>
            <a href="/repos/add" class="btn btn-primary" style="margin-top: 1rem;">+ Add Repository</a>
        </div>"#
            .to_string()
    } else {
        repos
            .iter()
            .map(|repo| {
                let status_badge = if repo.auto_scan_enabled {
                    r#"<span style="background: #22c55e; color: white; padding: 0.25rem 0.75rem; border-radius: 4px; font-size: 0.85rem;">Enabled</span>"#
                } else {
                    r#"<span style="background: #64748b; color: white; padding: 0.25rem 0.75rem; border-radius: 4px; font-size: 0.85rem;">Disabled</span>"#
                };

                let last_scan = repo
                    .last_scan_check
                    .as_deref()
                    .unwrap_or("Never");

                let last_hash = repo
                    .last_commit_hash
                    .as_deref()
                    .unwrap_or("—");

                let last_analyzed = repo
                    .last_analyzed
                    .as_deref()
                    .unwrap_or("Never");

                format!(
                    r#"<div class="scanner-repo" style="background: #1e293b; padding: 1.5rem; border-radius: 8px; margin-bottom: 1rem; border-left: 4px solid {};">
                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; flex-wrap: wrap; gap: 0.5rem;">
                        <div>
                            <h3 style="color: #f8fafc; margin-bottom: 0.25rem;">{}</h3>
                            <span style="color: #64748b; font-family: monospace; font-size: 0.85rem;">{}</span>
                        </div>
                        <div style="display: flex; gap: 0.5rem; align-items: center;">
                            {}
                        </div>
                    </div>
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin-bottom: 1rem;">
                        <div style="background: #0f172a; padding: 0.75rem; border-radius: 6px;">
                            <div style="color: #94a3b8; font-size: 0.8rem; margin-bottom: 0.25rem;">Scan Interval</div>
                            <div style="color: #e2e8f0; font-weight: 500;">{} minutes</div>
                        </div>
                        <div style="background: #0f172a; padding: 0.75rem; border-radius: 6px;">
                            <div style="color: #94a3b8; font-size: 0.8rem; margin-bottom: 0.25rem;">Last Scan Check</div>
                            <div style="color: #e2e8f0; font-weight: 500;">{}</div>
                        </div>
                        <div style="background: #0f172a; padding: 0.75rem; border-radius: 6px;">
                            <div style="color: #94a3b8; font-size: 0.8rem; margin-bottom: 0.25rem;">Last Commit Hash</div>
                            <div style="color: #e2e8f0; font-family: monospace; font-weight: 500;">{}</div>
                        </div>
                        <div style="background: #0f172a; padding: 0.75rem; border-radius: 6px;">
                            <div style="color: #94a3b8; font-size: 0.8rem; margin-bottom: 0.25rem;">Last Analyzed</div>
                            <div style="color: #e2e8f0; font-weight: 500;">{}</div>
                        </div>
                    </div>
                    <div style="display: flex; gap: 0.5rem; flex-wrap: wrap;">
                        <a href="/repos/{}/toggle-scan" class="btn-small {}">{}</a>
                        <a href="/scanner/{}/force" class="btn-small btn-primary">🔄 Force Scan</a>
                    </div>
                </div>"#,
                    if repo.auto_scan_enabled { "#22c55e" } else { "#64748b" },
                    repo.name,
                    repo.path,
                    status_badge,
                    repo.scan_interval_minutes,
                    last_scan,
                    last_hash,
                    last_analyzed,
                    repo.id,
                    if repo.auto_scan_enabled { "btn-danger" } else { "btn-success" },
                    if repo.auto_scan_enabled { "⏸ Disable Auto-Scan" } else { "▶ Enable Auto-Scan" },
                    repo.id,
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Auto-Scanner - RustAssistant</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #e2e8f0; }}
        .container {{ max-width: 1200px; margin: 0 auto; padding: 2rem; }}
        header {{ background: #1e293b; padding: 1.5rem; margin-bottom: 2rem; border-radius: 8px; }}
        h1 {{ color: #38bdf8; font-size: 2rem; margin-bottom: 0.5rem; }}
        h2 {{ color: #e2e8f0; margin-bottom: 1.5rem; }}
        nav {{ display: flex; gap: 1rem; margin-top: 1rem; flex-wrap: wrap; }}
        nav a {{ background: #334155; color: #e2e8f0; padding: 0.5rem 1rem; border-radius: 6px; text-decoration: none; transition: all 0.3s; }}
        nav a:hover {{ background: #475569; }}
        nav a.active {{ background: #0ea5e9; color: white; }}
        .btn, .btn-small {{ padding: 0.75rem 1.5rem; border-radius: 6px; border: none; cursor: pointer; font-size: 1rem; font-weight: 500; transition: all 0.3s; text-decoration: none; display: inline-block; }}
        .btn-small {{ padding: 0.5rem 1rem; font-size: 0.9rem; }}
        .btn-primary {{ background: #0ea5e9; color: white; }}
        .btn-primary:hover {{ background: #0284c7; }}
        .btn-success {{ background: #22c55e; color: white; }}
        .btn-success:hover {{ background: #16a34a; }}
        .btn-danger {{ background: #ef4444; color: white; }}
        .btn-danger:hover {{ background: #dc2626; }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos">Repositories</a>
                <a href="/queue">Queue</a>
                <a href="/scanner" class="active">Auto-Scanner</a>
            </nav>
        </header>

        <h2>🔍 Auto-Scanner Status</h2>

        <div class="scanner-list">
            {}
        </div>
    </div>
</body>
</html>"#,
        repos_html
    )
}

pub fn create_router(state: WebAppState) -> Router {
    let shared_state = Arc::new(state);

    Router::new()
        .route("/", get(dashboard_handler))
        .route("/dashboard", get(dashboard_handler))
        .route("/repos", get(repos_handler))
        .route("/repos/add", get(add_repo_form_handler))
        .route("/repos/add", post(add_repo_handler))
        .route("/repos/:id/toggle-scan", get(toggle_scan_handler))
        .route("/repos/:id/delete", get(delete_repo_handler))
        .route("/queue", get(queue_handler))
        .route("/queue/:id/delete", get(delete_queue_item_handler))
        .route("/scanner", get(scanner_handler))
        .route("/scanner/:id/force", get(force_scan_handler))
        .with_state(shared_state)
}
