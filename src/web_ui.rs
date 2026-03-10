//! Web UI module for Rustassistant dashboard
//!
//! Provides HTML templates and handlers for the web interface.
//! Features: repository management, queue operations, auto-scanner control.

use crate::db::{add_repository, remove_repository, Database, Repository};
use crate::git::GitManager;
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

/// Returns the shared timezone JavaScript that should be included in every page.
/// Handles timezone selection, localStorage persistence, and client-side timestamp conversion.
pub fn timezone_js() -> &'static str {
    r#"<script>
    (function() {
        const TIMEZONE_KEY = 'rustassistant_timezone';
        const DEFAULT_TZ = 'America/New_York';

        function getSavedTimezone() {
            return localStorage.getItem(TIMEZONE_KEY) || DEFAULT_TZ;
        }

        function saveTimezone(tz) {
            localStorage.setItem(TIMEZONE_KEY, tz);
        }

        function convertTimestamp(utcStr, tz) {
            // Accepts "YYYY-MM-DD HH:MM:SS" or "YYYY-MM-DD HH:MM:SS UTC"
            const cleaned = utcStr.replace(' UTC', '').trim();
            const date = new Date(cleaned + 'Z'); // append Z to parse as UTC
            if (isNaN(date.getTime())) return utcStr;
            try {
                return date.toLocaleString('en-US', {
                    timeZone: tz,
                    year: 'numeric',
                    month: '2-digit',
                    day: '2-digit',
                    hour: '2-digit',
                    minute: '2-digit',
                    second: '2-digit',
                    hour12: false
                });
            } catch(e) {
                return utcStr;
            }
        }

        function convertAllTimestamps() {
            const tz = getSavedTimezone();
            document.querySelectorAll('[data-utc]').forEach(function(el) {
                const utc = el.getAttribute('data-utc');
                el.textContent = convertTimestamp(utc, tz);
            });
            // Update selector if present
            const sel = document.getElementById('tz-select');
            if (sel) sel.value = tz;
        }

        // Initialize on DOM ready
        document.addEventListener('DOMContentLoaded', function() {
            convertAllTimestamps();
            var sel = document.getElementById('tz-select');
            if (sel) {
                sel.addEventListener('change', function() {
                    saveTimezone(this.value);
                    convertAllTimestamps();
                });
            }
        });
    })();
    </script>"#
}

/// Returns the HTML for the timezone selector dropdown, styled for the nav bar.
pub fn timezone_selector_html() -> &'static str {
    r#"<div style="margin-left: auto; display: flex; align-items: center; gap: 0.5rem;">
        <label for="tz-select" style="color: #94a3b8; font-size: 0.85rem;">🕐</label>
        <select id="tz-select" style="background: #334155; color: #e2e8f0; border: 1px solid #475569; border-radius: 4px; padding: 0.3rem 0.5rem; font-size: 0.85rem; cursor: pointer;">
            <option value="America/New_York">Eastern (EST/EDT)</option>
            <option value="America/Chicago">Central (CST/CDT)</option>
            <option value="America/Denver">Mountain (MST/MDT)</option>
            <option value="America/Los_Angeles">Pacific (PST/PDT)</option>
            <option value="UTC">UTC</option>
            <option value="Europe/London">London (GMT/BST)</option>
            <option value="Europe/Berlin">Berlin (CET/CEST)</option>
            <option value="Asia/Tokyo">Tokyo (JST)</option>
            <option value="Asia/Shanghai">Shanghai (CST)</option>
            <option value="Australia/Sydney">Sydney (AEST/AEDT)</option>
        </select>
    </div>"#
}

/// Wraps a UTC timestamp string in a `<span>` with `data-utc` attribute for JS conversion.
fn ts(utc_str: &str) -> String {
    format!(r#"<span data-utc="{}">{}</span>"#, utc_str, utc_str)
}

/// Application state for web UI
#[derive(Clone)]
pub struct WebAppState {
    pub db: Database,
    pub repos_dir: String,
}

impl WebAppState {
    pub fn new(db: Database, repos_dir: String) -> Self {
        Self { db, repos_dir }
    }
}

// ============================================================================
// Data Models
// ============================================================================

/// Dashboard statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_repos: i64,
    pub auto_scan_enabled: i32,
    pub tasks_pending: i64,
    pub tasks_in_progress: i64,
    pub tasks_completed: i64,
    pub tasks_failed: i64,
}

/// Repository item for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub git_url: Option<String>,
    pub status: String,
    pub auto_scan_enabled: bool,
    pub scan_interval_minutes: i32,
    pub last_scan_check: Option<String>,
    pub created_at: String,
    // Scan progress fields
    pub scan_status: Option<String>,
    pub scan_files_processed: Option<i32>,
    pub scan_files_total: Option<i32>,
    pub scan_current_file: Option<String>,
}

impl From<Repository> for RepoItem {
    fn from(repo: Repository) -> Self {
        Self {
            id: repo.id,
            name: repo.name,
            path: repo.path,
            git_url: repo.git_url,
            status: repo.status,
            auto_scan_enabled: repo.auto_scan_enabled != 0,
            scan_interval_minutes: repo.scan_interval_minutes as i32,
            last_scan_check: repo.last_scan_check.map(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            }),
            created_at: chrono::DateTime::from_timestamp(repo.created_at, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            scan_status: repo.scan_status,
            scan_files_processed: repo.scan_files_processed,
            scan_files_total: repo.scan_files_total,
            scan_current_file: repo.scan_current_file,
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
    pub description: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub repo_id: Option<String>,
    pub file_path: Option<String>,
}

/// Form data for adding repository
#[derive(Debug, Deserialize)]
pub struct AddRepoForm {
    pub git_url: String,
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
                <a href="/queue">Tasks</a>
                <a href="/ideas">Ideas</a>
                <a href="/docs">Docs</a>
                <a href="/activity">Activity</a>
                <a href="/scanner">Auto-Scanner</a>
                <a href="/db">DB Explorer</a>
                <a href="/scan/dashboard">Scan Progress</a>
                <a href="/cache">Cache</a>
                {}
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
                <h3>Tasks Pending</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card">
                <h3>Tasks In Progress</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card success">
                <h3>Tasks Completed</h3>
                <div class="value">{}</div>
            </div>
            <div class="stat-card danger">
                <h3>Tasks Failed</h3>
                <div class="value">{}</div>
            </div>
        </div>

        <div class="action-section">
            <h2>Quick Actions</h2>
            <div class="action-buttons">
                <a href="/repos/add" class="btn btn-primary">+ Add Repository</a>
                <a href="/queue" class="btn btn-success">View Tasks</a>
                <a href="/scanner" class="btn btn-primary">Scanner Settings</a>
            </div>
        </div>

        <footer>
            <p>RustAssistant v0.1.0 | Powered by Rust & Axum</p>
        </footer>
    </div>
    {}
</body>
</html>"#,
        timezone_selector_html(),
        stats.total_repos,
        stats.auto_scan_enabled,
        stats.tasks_pending,
        stats.tasks_in_progress,
        stats.tasks_completed,
        stats.tasks_failed,
        timezone_js()
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
                    .map(|s| ts(s))
                    .unwrap_or_else(|| "Never".to_string());

                // Build inline scan progress if actively scanning
                let is_scanning = matches!(
                    repo.scan_status.as_deref(),
                    Some("scanning") | Some("analyzing") | Some("cloning")
                );
                let inline_progress = if is_scanning {
                    let processed = repo.scan_files_processed.unwrap_or(0);
                    let total = repo.scan_files_total.unwrap_or(0);
                    let percent = if total > 0 {
                        (processed as f64 / total as f64 * 100.0).min(100.0)
                    } else {
                        0.0
                    };
                    let file_display = repo.scan_current_file.as_deref()
                        .map(|f| if f.len() > 60 { format!("…{}", &f[f.len().saturating_sub(57)..]) } else { f.to_string() })
                        .unwrap_or_else(|| "starting...".to_string());
                    format!(
                        r#"<div id="inline-progress-{id}" hx-get="/scan/inline/{id}" hx-trigger="every 2s" hx-swap="outerHTML"
                             style="margin-top:0.75rem;">
                            <div style="display:flex;justify-content:space-between;align-items:baseline;margin-bottom:0.25rem;">
                                <span style="font-size:1.1rem;font-weight:700;color:#0ea5e9;">
                                    &#x1f504; {done}<span style="color:#475569;font-weight:300;">/{total}</span> files
                                </span>
                                <span style="font-size:0.85rem;color:#94a3b8;">{percent:.0}%</span>
                            </div>
                            <div style="width:100%;background:#0f172a;border-radius:0.25rem;height:0.5rem;overflow:hidden;">
                                <div style="height:100%;background:linear-gradient(90deg,#0ea5e9,#8b5cf6);width:{percent:.1}%;transition:width 0.5s;"></div>
                            </div>
                            <div style="font-family:monospace;font-size:0.75rem;color:#64748b;margin-top:0.25rem;
                                        white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">
                                {file}
                            </div>
                        </div>"#,
                        id = repo.id,
                        done = processed,
                        total = total,
                        percent = percent,
                        file = file_display,
                    )
                } else {
                    String::new()
                };

                format!(
                    r#"<div class="repo-card">
                    <div class="repo-header">
                        <h3>{name}</h3>
                        <span class="repo-status {status_class}">{status}</span>
                    </div>
                    <div class="repo-info">
                        <p><strong>Source:</strong> {source}</p>
                        <p><strong>Auto-Scan:</strong> {scan_status}</p>
                        <p><strong>Last Scan:</strong> {last_scan}</p>
                        <p><strong>Created:</strong> {created}</p>
                    </div>{inline_progress}
                    <div class="repo-actions">
                        <a href="/repos/{id}/toggle-scan" class="btn-small btn-primary">Toggle Scan</a>
                        <a href="/scanner/{id}/force" class="btn-small btn-success">Scan Now</a>
                        <a href="/repos/{id}/delete" class="btn-small btn-danger" onclick="return confirm('Delete this repository?')">Delete</a>
                    </div>
                </div>"#,
                    name = repo.name,
                    status_class = if repo.auto_scan_enabled { "status-enabled" } else { "status-disabled" },
                    status = repo.status,
                    source = repo.git_url.as_deref().unwrap_or(&repo.path),
                    scan_status = scan_status,
                    last_scan = last_scan,
                    created = ts(&repo.created_at),
                    id = repo.id,
                    inline_progress = inline_progress,
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
    {tz_js}
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
                <a href="/queue">Tasks</a>
                <a href="/ideas">Ideas</a>
                <a href="/docs">Docs</a>
                <a href="/activity">Activity</a>
                <a href="/scanner">Auto-Scanner</a>
                <a href="/db">DB Explorer</a>
                <a href="/scan/dashboard">Scan Progress</a>
                <a href="/cache">Cache</a>
                {tz_selector}
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
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
</body>
</html>"#,
        repos_html,
        tz_js = timezone_js(),
        tz_selector = timezone_selector_html()
    )
}

fn render_add_repo_page() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Add Repository - RustAssistant</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #e2e8f0; }}
        .container {{ max-width: 800px; margin: 0 auto; padding: 2rem; }}
        header {{ background: #1e293b; padding: 1.5rem; margin-bottom: 2rem; border-radius: 8px; }}
        h1 {{ color: #38bdf8; font-size: 2rem; margin-bottom: 0.5rem; }}
        nav {{ display: flex; gap: 1rem; margin-top: 1rem; flex-wrap: wrap; }}
        nav a {{ background: #334155; color: #e2e8f0; padding: 0.5rem 1rem; border-radius: 6px; text-decoration: none; transition: all 0.3s; }}
        nav a:hover {{ background: #475569; }}
        .form-container {{ background: #1e293b; padding: 2rem; border-radius: 8px; }}
        .form-group {{ margin-bottom: 1.5rem; }}
        label {{ display: block; color: #94a3b8; margin-bottom: 0.5rem; font-weight: 500; }}
        input, select {{ width: 100%; padding: 0.75rem; border-radius: 6px; border: 1px solid #334155; background: #0f172a; color: #e2e8f0; font-size: 1rem; }}
        input:focus, select:focus {{ outline: none; border-color: #0ea5e9; }}
        .form-actions {{ display: flex; gap: 1rem; margin-top: 2rem; }}
        .btn {{ padding: 0.75rem 1.5rem; border-radius: 6px; border: none; cursor: pointer; font-size: 1rem; font-weight: 500; transition: all 0.3s; text-decoration: none; display: inline-block; }}
        .btn-primary {{ background: #0ea5e9; color: white; }}
        .btn-primary:hover {{ background: #0284c7; }}
        .btn-secondary {{ background: #64748b; color: white; }}
        .btn-secondary:hover {{ background: #475569; }}
        .help-text {{ color: #64748b; font-size: 0.9rem; margin-top: 0.25rem; }}
    </style>
    <script>
        function autoFillName() {{
            const urlInput = document.getElementById('git_url').value;
            const nameInput = document.getElementById('name');
            if (nameInput.value === '' || nameInput.dataset.autoFilled === 'true') {{
                // Extract repo name from URL: https://github.com/user/repo.git -> repo
                const match = urlInput.match(/\/([^\/]+?)(\.git)?$/);
                if (match) {{
                    nameInput.value = match[1];
                    nameInput.dataset.autoFilled = 'true';
                }}
            }}
        }}
        document.addEventListener('DOMContentLoaded', function() {{
            const nameInput = document.getElementById('name');
            nameInput.addEventListener('input', function() {{
                nameInput.dataset.autoFilled = 'false';
            }});
        }});
    </script>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos">Repositories</a>
                <a href="/queue">Tasks</a>
                <a href="/ideas">Ideas</a>
                <a href="/docs">Docs</a>
                <a href="/activity">Activity</a>
                <a href="/scanner">Auto-Scanner</a>
                <a href="/db">DB Explorer</a>
                <a href="/scan/dashboard">Scan Progress</a>
                <a href="/cache">Cache</a>
                {tz_selector}
            </nav>
        </header>

        <div class="form-container">
            <h2 style="color: #e2e8f0; margin-bottom: 1.5rem;">Add Repository</h2>
            <form method="POST" action="/repos/add">
                <div class="form-group">
                    <label for="git_url">GitHub URL</label>
                    <input type="text" id="git_url" name="git_url" required
                           placeholder="https://github.com/user/repo"
                           oninput="autoFillName()">
                    <p class="help-text">GitHub repository URL — the repo will be cloned automatically</p>
                </div>
                <div class="form-group">
                    <label for="name">Repository Name</label>
                    <input type="text" id="name" name="name" required placeholder="myproject" data-auto-filled="false">
                    <p class="help-text">A friendly name for this repository (auto-filled from URL)</p>
                </div>
                <div class="form-actions">
                    <button type="submit" class="btn btn-primary">Clone &amp; Add Repository</button>
                    <a href="/repos" class="btn btn-secondary">Cancel</a>
                </div>
            </form>
        </div>
    </div>
    {tz_js}
</body>
</html>"#,
        tz_js = timezone_js(),
        tz_selector = timezone_selector_html()
    )
}

fn render_queue_page(items: Vec<QueueItemDisplay>) -> String {
    let items_html = if items.is_empty() {
        r#"<div style="text-align: center; padding: 3rem; color: #64748b;">
            <p style="font-size: 1.2rem;">No tasks yet</p>
            <p style="color: #475569;">Tasks are generated automatically when the scanner completes a project review.</p>
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

                let desc_html = if let Some(desc) = &item.description {
                    let preview = if desc.len() > 300 {
                        format!("{}…", &desc[..300])
                    } else {
                        desc.clone()
                    };
                    format!(r#"<div class="queue-content"><pre>{}</pre></div>"#, preview)
                } else {
                    String::new()
                };

                let location_html = match (&item.repo_id, &item.file_path) {
                    (Some(repo), Some(file)) => format!(r#"<span>📁 {}:{}</span>"#, repo, file),
                    (Some(repo), None) => format!(r#"<span>📁 {}</span>"#, repo),
                    (_, Some(file)) => format!(r#"<span>📁 {}</span>"#, file),
                    _ => String::new(),
                };

                // Build a rich copy payload: title + description + file path
                let mut copy_text = item.content.clone();
                if let Some(ref desc) = item.description {
                    copy_text.push_str("\\n\\n");
                    copy_text.push_str(&desc.replace('\'', "\\'").replace('\n', "\\n"));
                }
                if let Some(ref fp) = item.file_path {
                    copy_text.push_str(&format!("\\n\\nFile: {}", fp));
                }
                let copy_btn = format!(
                    r#"<button class="btn-small btn-primary" onclick="copyToClipboard('{}')">📋 Copy for IDE</button>"#,
                    copy_text.replace('\'', "\\'").replace('\n', "\\n")
                );

                format!(
                    r#"<div class="queue-item stage-{}">
                    <div class="queue-header">
                        <span class="queue-id">{}</span>
                        <span class="queue-stage">{}</span>
                        <span class="queue-priority priority-{}">{}</span>
                    </div>
                    <div class="queue-content">
                        <strong>{}</strong>
                    </div>
                    {}
                    {}
                    <div class="queue-meta">
                        <span>Source: {}</span>
                        {}
                        <span>Created: {}</span>
                    </div>
                    <div class="queue-actions">
                        {}
                        <a href="/queue/{}/delete" class="btn-small btn-danger">Delete</a>
                    </div>
                </div>"#,
                    item.stage.to_lowercase(),
                    &item.id,
                    item.stage,
                    item.priority.to_lowercase(),
                    item.priority,
                    item.content,
                    desc_html,
                    error_html,
                    item.source,
                    location_html,
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
    <title>Tasks - RustAssistant</title>
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
        .queue-item.stage-review {{ border-left-color: #a78bfa; }}
        .queue-item.stage-ready {{ border-left-color: #2dd4bf; }}
        .queue-item.stage-done {{ border-left-color: #22c55e; }}
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
                <a href="/queue" class="active">Tasks</a>
                <a href="/ideas">Ideas</a>
                <a href="/docs">Docs</a>
                <a href="/activity">Activity</a>
                <a href="/scanner">Auto-Scanner</a>
                <a href="/db">DB Explorer</a>
                <a href="/scan/dashboard">Scan Progress</a>
                <a href="/cache">Cache</a>
                {}
            </nav>
        </header>

        <div class="page-header">
            <h2 style="color: #e2e8f0;">Tasks</h2>
        </div>

        <div class="queue-list">
            {}
        </div>
    </div>
    {}
</body>
</html>"#,
        timezone_selector_html(),
        items_html,
        timezone_js()
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

/// Add repository POST handler — clones from GitHub URL
pub async fn add_repo_handler(
    State(state): State<Arc<WebAppState>>,
    Form(form): Form<AddRepoForm>,
) -> impl IntoResponse {
    // Normalize the URL: ensure it ends with .git for cloning
    let git_url = if form.git_url.ends_with(".git") {
        form.git_url.clone()
    } else {
        format!("{}.git", form.git_url.trim_end_matches('/'))
    };

    // Clone the repo into the repos directory
    let repos_dir = std::path::PathBuf::from(&state.repos_dir);
    let clone_path = repos_dir.join(&form.name);

    match GitManager::new(repos_dir.clone(), false) {
        Ok(git) => match git.clone_repo(&git_url, Some(&form.name)) {
            Ok(cloned_path) => {
                let path_str = cloned_path.to_string_lossy().to_string();
                match add_repository(&state.db.pool, &path_str, &form.name, Some(&git_url)).await {
                    Ok(_) => {
                        info!(
                            "Cloned and added repository: {} from {} to {}",
                            form.name, git_url, path_str
                        );
                        (
                            StatusCode::SEE_OTHER,
                            [("Location", "/repos")],
                            "Redirecting...",
                        )
                    }
                    Err(e) => {
                        error!("Cloned repo but failed to save to DB: {}", e);
                        // Clean up the cloned directory on DB failure
                        let _ = std::fs::remove_dir_all(&clone_path);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            [("Location", "/repos")],
                            "Error saving repository",
                        )
                    }
                }
            }
            Err(e) => {
                error!("Failed to clone repository {}: {}", git_url, e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [("Location", "/repos")],
                    "Error cloning repository",
                )
            }
        },
        Err(e) => {
            error!("Failed to initialize git manager: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Location", "/repos")],
                "Error initializing git manager",
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

/// Repository settings update request
#[derive(Debug, Deserialize)]
pub struct UpdateRepoSettingsRequest {
    pub scan_interval_minutes: Option<i64>,
    pub auto_scan_enabled: Option<bool>,
}

/// Update repository settings handler (API endpoint)
pub async fn update_repo_settings_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
    Form(settings): Form<UpdateRepoSettingsRequest>,
) -> impl IntoResponse {
    // Validate scan interval
    if let Some(interval) = settings.scan_interval_minutes {
        if !(5..=1440).contains(&interval) {
            return (
                StatusCode::BAD_REQUEST,
                [("HX-Trigger", r#"{"showToast": {"message": "Scan interval must be between 5 and 1440 minutes", "type": "error"}}"#)],
                "Invalid scan interval"
            ).into_response();
        }
    }

    match update_repo_settings(&state.db, &id, settings).await {
        Ok(_) => {
            info!("Updated settings for repo {}", id);
            (
                StatusCode::OK,
                [
                    ("HX-Trigger", r#"{"showToast": {"message": "Settings updated successfully", "type": "success"}}"#),
                    ("HX-Refresh", "true")
                ],
                "OK"
            ).into_response()
        }
        Err(e) => {
            error!("Failed to update repo settings: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(
                    "HX-Trigger",
                    r#"{"showToast": {"message": "Failed to update settings", "type": "error"}}"#,
                )],
                "Error",
            )
                .into_response()
        }
    }
}

/// Get repository scan progress (for HTMX polling)
pub async fn get_repo_progress_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match crate::db::core::get_repository(&state.db.pool, &id).await {
        Ok(repo) => {
            let progress_html = format!(
                r#"<div id="progress-{}" hx-get="/repos/{}/progress" hx-trigger="every 3s" hx-swap="outerHTML">
                    {}
                </div>"#,
                id,
                id,
                render_progress_bar(&repo)
            );
            Html(progress_html).into_response()
        }
        Err(e) => {
            error!("Failed to get repo progress: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Error").into_response()
        }
    }
}

/// Render progress bar HTML for a repository
fn render_progress_bar(repo: &Repository) -> String {
    let status = repo.scan_status.as_deref().unwrap_or("idle");

    match status {
        "scanning" => {
            let percentage = repo.progress_percentage();
            let processed = repo.scan_files_processed.unwrap_or(0);
            let total = repo.scan_files_total.unwrap_or(0);
            let current_file = repo.scan_current_file.as_deref().unwrap_or("...");

            format!(
                r#"<div style="margin-top: 0.5rem;">
                    <div style="display: flex; justify-content: space-between; margin-bottom: 0.25rem; font-size: 0.75rem;">
                        <span>🔄 Scanning... ({}/{})</span>
                        <span>{}%</span>
                    </div>
                    <div style="width: 100%; background: var(--bg); border-radius: 0.25rem; height: 1rem; overflow: hidden;">
                        <div style="height: 100%; background: linear-gradient(90deg, #3b82f6, #8b5cf6); width: {}%; transition: width 0.3s;"></div>
                    </div>
                    <div style="font-size: 0.7rem; color: var(--text-muted); margin-top: 0.25rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">
                        {}</div>
                </div>"#,
                processed, total, percentage, percentage, current_file
            )
        }
        "error" => {
            let error_msg = repo.last_error.as_deref().unwrap_or("Unknown error");
            format!(
                r#"<div style="margin-top: 0.5rem; padding: 0.5rem; background: rgba(239, 68, 68, 0.1); border-left: 3px solid #ef4444; border-radius: 0.25rem;">
                    <div style="font-size: 0.75rem; color: #ef4444;">❌ Scan failed</div>
                    <div style="font-size: 0.7rem; color: var(--text-muted); margin-top: 0.25rem;">{}</div>
                </div>"#,
                error_msg
            )
        }
        _ => {
            // Idle state - show last scan metrics if available
            if let (Some(duration), Some(files), Some(issues)) = (
                repo.last_scan_duration_ms,
                repo.last_scan_files_found,
                repo.last_scan_issues_found,
            ) {
                format!(
                    r#"<div style="margin-top: 0.5rem; font-size: 0.75rem; color: var(--text-muted);">
                        ✅ Last scan: {} files, {} issues in {}ms
                    </div>"#,
                    files, issues, duration
                )
            } else {
                String::from(
                    r#"<div style="margin-top: 0.5rem; font-size: 0.75rem; color: var(--text-muted);">No scan data available</div>"#,
                )
            }
        }
    }
}

/// Delete repository handler — also removes cloned repo from disk
pub async fn delete_repo_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Look up the repo before deleting so we can clean up the clone directory
    let repo_path = sqlx::query_scalar::<_, String>("SELECT path FROM repositories WHERE id = $1")
        .bind(&id)
        .fetch_optional(&state.db.pool)
        .await
        .ok()
        .flatten();

    match remove_repository(&state.db.pool, &id).await {
        Ok(_) => {
            // Clean up the cloned directory if it lives inside our repos_dir
            if let Some(path) = repo_path {
                let path = std::path::Path::new(&path);
                let repos_dir = std::path::Path::new(&state.repos_dir);
                if path.starts_with(repos_dir) && path.exists() {
                    if let Err(e) = std::fs::remove_dir_all(path) {
                        error!("Failed to remove cloned repo at {}: {}", path.display(), e);
                    } else {
                        info!("Removed cloned repo directory: {}", path.display());
                    }
                }
            }
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

/// Task queue handler — reads from the consolidated `tasks` table
pub async fn queue_handler(State(state): State<Arc<WebAppState>>) -> impl IntoResponse {
    match get_task_items(&state.db).await {
        Ok(items) => Html(render_queue_page(items)),
        Err(e) => {
            error!("Failed to get tasks: {}", e);
            Html(format!("<h1>Error loading tasks: {}</h1>", e))
        }
    }
}

/// Delete task handler
pub async fn delete_queue_item_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match delete_task_item(&state.db, &id).await {
        Ok(_) => (StatusCode::SEE_OTHER, [("Location", "/queue")], "OK"),
        Err(e) => {
            error!("Failed to delete task: {}", e);
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
    let auto_scan_enabled =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM repositories WHERE auto_scan = 1")
            .fetch_one(&db.pool)
            .await
            .unwrap_or(0) as i32;

    // Get task stats from the consolidated tasks table
    let tasks_pending =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks WHERE status = 'pending'")
            .fetch_one(&db.pool)
            .await
            .unwrap_or(0);

    let tasks_in_progress = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM tasks WHERE status = 'processing' OR status = 'review' OR status = 'ready'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap_or(0);

    let tasks_completed =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks WHERE status = 'done'")
            .fetch_one(&db.pool)
            .await
            .unwrap_or(0);

    let tasks_failed =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks WHERE status = 'failed'")
            .fetch_one(&db.pool)
            .await
            .unwrap_or(0);

    Ok(DashboardStats {
        total_repos,
        auto_scan_enabled,
        tasks_pending,
        tasks_in_progress,
        tasks_completed,
        tasks_failed,
    })
}

async fn toggle_repo_autoscan(db: &Database, id: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE repositories SET auto_scan = CASE WHEN auto_scan = 1 THEN 0 ELSE 1 END, updated_at = $1 WHERE id = $2"
    )
        .bind(chrono::Utc::now().timestamp())
        .bind(id)
        .execute(&db.pool)
        .await?;

    Ok(())
}

/// Update repository settings
async fn update_repo_settings(
    db: &Database,
    id: &str,
    settings: UpdateRepoSettingsRequest,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().timestamp();
    let mut set_clauses: Vec<String> = vec!["updated_at = $1".to_string()];
    let mut param_idx: u32 = 2;

    if settings.scan_interval_minutes.is_some() {
        set_clauses.push(format!("scan_interval_mins = ${}", param_idx));
        param_idx += 1;
    }
    if settings.auto_scan_enabled.is_some() {
        set_clauses.push(format!("auto_scan = ${}", param_idx));
        param_idx += 1;
    }

    let query_str = format!(
        "UPDATE repositories SET {} WHERE id = ${}",
        set_clauses.join(", "),
        param_idx
    );

    let mut query = sqlx::query(&query_str).bind(now);

    if let Some(interval) = settings.scan_interval_minutes {
        query = query.bind(interval);
    }
    if let Some(enabled) = settings.auto_scan_enabled {
        query = query.bind(if enabled { 1i64 } else { 0i64 });
    }
    query = query.bind(id);

    query.execute(&db.pool).await?;

    Ok(())
}

/// Fetch tasks from the consolidated `tasks` table for the queue page.
async fn get_task_items(db: &Database) -> anyhow::Result<Vec<QueueItemDisplay>> {
    #[derive(sqlx::FromRow)]
    struct TaskRow {
        id: String,
        title: String,
        description: Option<String>,
        priority: i32,
        status: String,
        source: String,
        repo_id: Option<String>,
        file_path: Option<String>,
        created_at: i64,
    }

    let items = sqlx::query_as::<_, TaskRow>(
        "SELECT id,
                COALESCE(title, content, 'Untitled') as title,
                COALESCE(description, context) as description,
                priority,
                status,
                COALESCE(source, source_type, 'unknown') as source,
                COALESCE(repo_id, source_repo) as repo_id,
                COALESCE(file_path, source_file) as file_path,
                created_at
         FROM tasks
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
                stage: item.status,
                priority: priority_label,
                content: item.title,
                description: item.description,
                error_message: None,
                created_at: {
                    let formatted = chrono::DateTime::from_timestamp(item.created_at, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    ts(&formatted)
                },
                repo_id: item.repo_id,
                file_path: item.file_path,
            }
        })
        .collect())
}

/// Delete a task from the consolidated `tasks` table.
async fn delete_task_item(db: &Database, id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM tasks WHERE id = $1")
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
    scan_interval_minutes: i32,
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
    match sqlx::query(
        "UPDATE repositories SET last_scanned_at = NULL, last_commit_hash = NULL WHERE id = $1",
    )
    .bind(&id)
    .execute(&state.db.pool)
    .await
    {
        Ok(_) => {
            info!(
                "Forced full rescan for repo {} (cleared commit hash + scan time)",
                id
            );
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

/// Request a project review re-run for a repo.
///
/// Sets the `review_requested` flag in the DB so the scanner loop picks it
/// up on its next 60-second cycle and calls `generate_project_review` using
/// the existing cached file analyses (no re-scan needed, ~$0.01).
pub async fn request_review_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match sqlx::query("UPDATE repositories SET review_requested = TRUE WHERE id = $1")
        .bind(&id)
        .execute(&state.db.pool)
        .await
    {
        Ok(_) => {
            info!("Project review requested for repo {}", id);
            (StatusCode::SEE_OTHER, [("Location", "/scanner")], "OK")
        }
        Err(e) => {
            error!("Failed to request review for repo {}: {}", id, e);
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
                    .as_ref()
                    .map(|s| ts(s))
                    .unwrap_or_else(|| "Never".to_string());

                let last_hash = repo
                    .last_commit_hash
                    .as_deref()
                    .unwrap_or("—");

                let last_analyzed = repo
                    .last_analyzed
                    .as_ref()
                    .map(|s| ts(s))
                    .unwrap_or_else(|| "Never".to_string());

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
                        <a href="/scanner/{}/review" class="btn-small btn-success">📋 Re-run Review</a>
                        <a href="/queue?repo={}" class="btn-small" style="background:#6366f1;color:white;">📝 View Tasks</a>
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
                    repo.id,
                    repo.name,
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
                <a href="/queue">Tasks</a>
                <a href="/ideas">Ideas</a>
                <a href="/docs">Docs</a>
                <a href="/activity">Activity</a>
                <a href="/scanner" class="active">Auto-Scanner</a>
                <a href="/db">DB Explorer</a>
                <a href="/scan/dashboard">Scan Progress</a>
                <a href="/cache">Cache</a>
                {tz_selector}
            </nav>
        </header>

        <h2>🔍 Auto-Scanner Status</h2>

        <div class="scanner-list">
            {repos_html}
        </div>
    </div>
    {tz_js}
</body>
</html>"#,
        tz_selector = timezone_selector_html(),
        repos_html = repos_html,
        tz_js = timezone_js()
    )
}

// ============================================================================
// Notes Handlers
// ============================================================================

/// Notes page handler - display all notes with filtering
pub async fn notes_handler(State(state): State<Arc<WebAppState>>) -> impl IntoResponse {
    let notes = match crate::db::core::list_notes(&state.db.pool, 100, None, None, None).await {
        Ok(notes) => notes,
        Err(e) => {
            error!("Failed to fetch notes: {}", e);
            vec![]
        }
    };

    let total = notes.len();
    let tz_selector = timezone_selector_html();
    let tz_js = timezone_js();

    let notes_html = if notes.is_empty() {
        r#"<div class="card">
    <div style="text-align: center; padding: 4rem 2rem;">
        <div style="font-size: 4rem; margin-bottom: 1rem;">📝</div>
        <h2 style="margin-bottom: 1rem;">No Notes Yet</h2>
        <p class="text-muted" style="margin-bottom: 2rem;">
            Start capturing your ideas, tasks, and thoughts with Rustassistant notes.
        </p>
        <button onclick="document.getElementById('note-capture-modal').style.display='flex'" class="btn btn-primary">Create Your First Note</button>
    </div>
</div>"#.to_string()
    } else {
        format!(
            r#"<div class="card">
    <div class="card-header">
        <h2 class="card-title">All Notes ({})</h2>
    </div>
    <div class="notes-grid">
        {}
    </div>
</div>"#,
            total,
            notes
                .iter()
                .map(|note| {
                    let tags_html = note
                        .tags
                        .as_ref()
                        .map(|t| {
                            t.split(',')
                                .map(|tag| format!(r#"<span class="badge badge-primary">{}</span>"#, tag.trim()))
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .unwrap_or_default();

                    let status_class = match note.status.as_str() {
                        "inbox" => "badge-warning",
                        "active" => "badge-primary",
                        "done" => "badge-success",
                        _ => "badge-secondary",
                    };

                    format!(
                        r#"<div class="note-card" style="padding: 1.5rem; border-bottom: 1px solid var(--border);">
    <div style="display: flex; justify-content: space-between; gap: 1rem; margin-bottom: 1rem;">
        <div style="flex: 1;">
            <p style="margin-bottom: 0.75rem; font-size: 1rem; line-height: 1.6;">
                {}
            </p>
            {}
            <div class="text-muted" style="font-size: 0.75rem;">
                Created: {}
            </div>
        </div>
        <div style="display: flex; flex-direction: column; gap: 0.5rem; align-items: flex-end;">
            <span class="badge {}">
                {}
            </span>
            <div style="display: flex; gap: 0.25rem;">
                <a href="/notes/{}/edit" class="btn btn-sm btn-secondary">Edit</a>
                <button class="btn btn-sm btn-danger" onclick="deleteNote('{}')">Delete</button>
            </div>
        </div>
    </div>
</div>"#,
                        note.content,
                        if !tags_html.is_empty() {
                            format!(r#"<div style="display: flex; gap: 0.5rem; flex-wrap: wrap; margin-bottom: 0.5rem;">{}</div>"#, tags_html)
                        } else {
                            String::new()
                        },
                        note.created_at_formatted(),
                        status_class,
                        note.status,
                        note.id,
                        note.id
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Notes - Rustassistant</title>
    <link rel="stylesheet" href="/static/styles.css">
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 Rustassistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos">Repositories</a>
                <a href="/queue">Tasks</a>
                <a href="/ideas">Ideas</a>
                <a href="/docs">Docs</a>
                <a href="/activity">Activity</a>
                <a href="/scanner">Scanner</a>
                <a href="/notes" class="active">Notes</a>
                <a href="/db">DB Explorer</a>
                <a href="/scan/dashboard">Scan Progress</a>
                <a href="/cache">Cache</a>
                {tz_selector}
            </nav>
        </header>

        <div class="page-header mb-4">
            <div style="display: flex; justify-content: space-between; align-items: center;">
                <div>
                    <h1>Notes</h1>
                    <p class="text-muted">Capture ideas, tasks, and thoughts</p>
                </div>
                <button onclick="document.getElementById('note-capture-modal').style.display='flex'" class="btn btn-primary">+ Quick Note</button>
            </div>
        </div>

        <!-- Quick Capture Modal -->
        <div id="note-capture-modal" style="display: none; position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.5); align-items: center; justify-content: center; z-index: 1000;">
            <div style="background: var(--card-bg); padding: 2rem; border-radius: 0.5rem; max-width: 600px; width: 90%;">
                <h2 style="margin-bottom: 1rem;">Quick Note</h2>
                <form id="note-form" hx-post="/api/notes" hx-swap="none" onsubmit="return false;">
                    <textarea name="content" placeholder="Write your note... Use #tags for categorization" style="width: 100%; min-height: 120px; margin-bottom: 1rem; padding: 0.75rem; border-radius: 0.25rem;" required></textarea>
                    <div style="display: flex; gap: 0.5rem; justify-content: flex-end;">
                        <button type="button" class="btn btn-secondary" onclick="document.getElementById('note-capture-modal').style.display='none'">Cancel</button>
                        <button type="submit" class="btn btn-primary">Save Note</button>
                    </div>
                </form>
            </div>
        </div>

        {notes_html}
    </div>
    <script src="https://unpkg.com/htmx.org@1.9.10"></script>
    <script>
        // Handle note form submission
        document.getElementById('note-form').addEventListener('submit', async function(e) {{
            e.preventDefault();
            const formData = new FormData(this);
            const response = await fetch('/api/notes', {{
                method: 'POST',
                body: formData
            }});
            if (response.ok) {{
                document.getElementById('note-capture-modal').style.display = 'none';
                this.reset();
                window.location.reload();
            }}
        }});

        function deleteNote(id) {{
            if (confirm('Are you sure you want to delete this note?')) {{
                fetch('/api/notes/' + id, {{ method: 'DELETE' }})
                    .then(r => r.ok ? window.location.reload() : alert('Failed to delete'));
            }}
        }}
    </script>
    {tz_js}
</body>
</html>"#,
        tz_selector = tz_selector,
        notes_html = notes_html,
        tz_js = tz_js
    ))
}

/// Create note API endpoint
#[derive(Debug, Deserialize)]
pub struct CreateNoteRequest {
    pub content: String,
    pub repo_id: Option<String>,
}

pub async fn create_note_handler(
    State(state): State<Arc<WebAppState>>,
    Form(request): Form<CreateNoteRequest>,
) -> impl IntoResponse {
    // Extract tags from content (anything starting with #)
    let tags: Vec<&str> = request
        .content
        .split_whitespace()
        .filter(|word| word.starts_with('#'))
        .map(|tag| &tag[1..]) // Remove the # prefix
        .collect();

    match crate::db::core::create_note_with_tags(
        &state.db.pool,
        &request.content,
        &tags,
        None,
        request.repo_id.as_deref(),
    )
    .await
    {
        Ok(_) => (
            StatusCode::OK,
            [(
                "HX-Trigger",
                r#"{"showToast": {"message": "Note created", "type": "success"}}"#,
            )],
            "OK",
        )
            .into_response(),
        Err(e) => {
            error!("Failed to create note: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(
                    "HX-Trigger",
                    r#"{"showToast": {"message": "Failed to create note", "type": "error"}}"#,
                )],
                "Error",
            )
                .into_response()
        }
    }
}

/// Delete note API endpoint
pub async fn delete_note_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match crate::db::core::delete_note(&state.db.pool, &id).await {
        Ok(_) => (StatusCode::OK, "OK").into_response(),
        Err(e) => {
            error!("Failed to delete note: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Error").into_response()
        }
    }
}

// ============================================================================
// Router
// ============================================================================

pub fn create_router(state: WebAppState) -> Router {
    let shared_state = Arc::new(state);

    Router::new()
        .route("/", get(dashboard_handler))
        .route("/dashboard", get(dashboard_handler))
        .route("/repos", get(repos_handler))
        .route("/repos/add", get(add_repo_form_handler))
        .route("/repos/add", post(add_repo_handler))
        .route("/repos/:id/toggle-scan", get(toggle_scan_handler))
        .route("/repos/:id/progress", get(get_repo_progress_handler))
        .route("/repos/:id/delete", get(delete_repo_handler))
        .route("/notes", get(notes_handler))
        .route("/api/notes", post(create_note_handler))
        .route("/api/notes/:id", axum::routing::delete(delete_note_handler))
        .route("/queue", get(queue_handler))
        .route("/queue/:id/delete", get(delete_queue_item_handler))
        .route("/scanner", get(scanner_handler))
        .route("/scanner/:id/force", get(force_scan_handler))
        .route("/scanner/:id/review", get(request_review_handler))
        .with_state(shared_state)
}
