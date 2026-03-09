// ============================================================================
// Enhanced Scan Progress — Web UI Component
// ============================================================================
//
// Replaces the existing scan progress bar with a much more detailed view:
//   - Prominent [42/936] file counter
//   - Live cost accumulator
//   - ETA estimation based on elapsed time
//   - Current file being analyzed
//   - Cache hit vs API call breakdown
//   - HTMX auto-refresh every 2 seconds while scanning
//
// This file provides:
//   1. A standalone scan dashboard page at /scan/dashboard
//   2. An HTMX partial at /scan/status (replaces scan_progress_partial)
//   3. Helper functions to render progress bars inline on other pages
//
// Integration:
//   In src/lib.rs:
//     pub mod web_ui_scan_progress;
//
//   In src/bin/server.rs:
//     use rustassistant::web_ui_scan_progress::create_scan_progress_router;
//     let scan_router = create_scan_progress_router(Arc::new(web_state.clone()));
//     let app = Router::new()
//         .merge(web_router)
//         .merge(scan_router)   // <-- add this
//         ...;
//
// DB Requirements:
//   The existing repositories table columns are used:
//     scan_status, scan_files_processed, scan_files_total,
//     scan_current_file, last_scan_duration_ms, last_scan_issues_found,
//     last_error
//
//   NEW: Add these columns for enhanced tracking (migration):
//     ALTER TABLE repositories ADD COLUMN scan_started_at INTEGER;
//     ALTER TABLE repositories ADD COLUMN scan_cost_accumulated REAL DEFAULT 0.0;
//     ALTER TABLE repositories ADD COLUMN scan_cache_hits INTEGER DEFAULT 0;
//     ALTER TABLE repositories ADD COLUMN scan_api_calls INTEGER DEFAULT 0;

use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::sync::Arc;
use tracing::error;

use crate::web_ui::WebAppState;

// ============================================================================
// Router
// ============================================================================

pub fn create_scan_progress_router(state: Arc<WebAppState>) -> Router {
    Router::new()
        .route("/scan/dashboard", get(scan_dashboard_handler))
        .route("/scan/status", get(scan_status_partial_handler))
        .route("/scan/status/{repo_id}", get(scan_repo_status_handler))
        .route("/scan/inline/{repo_id}", get(scan_inline_status_handler))
        .with_state(state)
}

// ============================================================================
// Helpers
// ============================================================================

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn nav(active: &str) -> String {
    let items = [
        ("Dashboard", "/dashboard"),
        ("Repos", "/repos"),
        ("Scan Progress", "/scan/dashboard"),
        ("Cache Viewer", "/cache"),
        ("DB Explorer", "/db"),
        ("Queue", "/queue"),
        ("Ideas", "/ideas"),
        ("Docs", "/docs"),
        ("Activity", "/activity"),
    ];
    let links: String = items
        .iter()
        .map(|(label, href)| {
            let class = if *label == active {
                " class=\"active\""
            } else {
                ""
            };
            format!(r#"<a href="{href}"{class}>{label}</a>"#)
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"<nav>
        <span style="font-weight:700;color:#0ea5e9;margin-right:1rem;">🦀 RustAssistant</span>
        {links}
    </nav>"#
    )
}

// ============================================================================
// Data structs
// ============================================================================

struct ScanRepoInfo {
    id: String,
    name: String,
    scan_status: String,
    files_processed: i64,
    files_total: i64,
    current_file: Option<String>,
    issues_found: i64,
    last_duration_ms: Option<i64>,
    last_error: Option<String>,
    scan_started_at: Option<i64>,
    cost_accumulated: f64,
    cache_hits: i64,
    api_calls: i64,
}

async fn get_scan_repos(pool: &sqlx::PgPool) -> Vec<ScanRepoInfo> {
    // Try with new columns first, fall back to without them
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        String,
        String,
        String,
        i64,
        i64,
        Option<String>,
        i64,
        Option<i64>,
        Option<String>,
        Option<i64>,
        f64,
        i64,
        i64,
    )> = sqlx::query_as(
        r#"SELECT
            id, name,
            COALESCE(scan_status, 'idle') as scan_status,
            COALESCE(scan_files_processed, 0) as files_processed,
            COALESCE(scan_files_total, 0) as files_total,
            scan_current_file,
            COALESCE(last_scan_issues_found, 0) as issues_found,
            last_scan_duration_ms,
            last_error,
            scan_started_at,
            COALESCE(scan_cost_accumulated, 0.0) as cost_accumulated,
            COALESCE(scan_cache_hits, 0) as cache_hits,
            COALESCE(scan_api_calls, 0) as api_calls
        FROM repositories
        WHERE auto_scan = 1
        ORDER BY
            CASE scan_status
                WHEN 'scanning' THEN 0
                WHEN 'analyzing' THEN 0
                WHEN 'cloning' THEN 1
                WHEN 'error' THEN 2
                ELSE 3
            END,
            name"#,
    )
    .fetch_all(pool)
    .await
    .unwrap_or_else(|e| {
        error!("Failed to fetch scan repo info: {}", e);
        Vec::new()
    });

    rows.into_iter()
        .map(
            |(
                id,
                name,
                status,
                processed,
                total,
                current,
                issues,
                dur,
                err,
                started,
                cost,
                hits,
                calls,
            )| {
                ScanRepoInfo {
                    id,
                    name,
                    scan_status: status,
                    files_processed: processed,
                    files_total: total,
                    current_file: current,
                    issues_found: issues,
                    last_duration_ms: dur,
                    last_error: err,
                    scan_started_at: started,
                    cost_accumulated: cost,
                    cache_hits: hits,
                    api_calls: calls,
                }
            },
        )
        .collect()
}

// ============================================================================
// Render: Individual repo progress card
// ============================================================================

fn render_repo_progress_card(repo: &ScanRepoInfo) -> String {
    let is_scanning = matches!(
        repo.scan_status.as_str(),
        "scanning" | "analyzing" | "cloning"
    );
    let percent = if repo.files_total > 0 {
        (repo.files_processed as f64 / repo.files_total as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    // ETA calculation
    let eta_html = if is_scanning && repo.files_processed > 5 {
        if let Some(started) = repo.scan_started_at {
            let now = chrono::Utc::now().timestamp();
            let elapsed_secs = (now - started).max(1);
            let rate = repo.files_processed as f64 / elapsed_secs as f64;
            let remaining = repo.files_total - repo.files_processed;
            if rate > 0.0 {
                let eta_secs = (remaining as f64 / rate) as i64;
                let eta_str = if eta_secs > 3600 {
                    format!("{}h {}m", eta_secs / 3600, (eta_secs % 3600) / 60)
                } else if eta_secs > 60 {
                    format!("{}m {}s", eta_secs / 60, eta_secs % 60)
                } else {
                    format!("{}s", eta_secs)
                };
                format!(
                    r#"<span style="color:#fbbf24;font-size:0.8rem;">⏱ ETA: {}</span>"#,
                    eta_str
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Cost display
    let cost_html = if repo.cost_accumulated > 0.0 || repo.api_calls > 0 {
        format!(
            r#"<div style="display:flex;gap:1.5rem;font-size:0.8rem;color:#64748b;margin-top:0.5rem;">
                <span>💰 ${:.4}</span>
                <span>🔍 {} API calls</span>
                <span>📦 {} cache hits</span>
            </div>"#,
            repo.cost_accumulated, repo.api_calls, repo.cache_hits,
        )
    } else {
        String::new()
    };

    match repo.scan_status.as_str() {
        "scanning" | "analyzing" | "cloning" => {
            let current_file = repo.current_file.as_deref().unwrap_or("starting...");
            // Truncate long file paths from the left
            let display_file = if current_file.len() > 80 {
                format!("…{}", &current_file[current_file.len() - 77..])
            } else {
                current_file.to_string()
            };

            let status_label = match repo.scan_status.as_str() {
                "cloning" => "📥 Cloning",
                "analyzing" => "🔬 Analyzing",
                _ => "🔄 Scanning",
            };

            format!(
                r#"<div class="scan-card scanning"
                     hx-get="/scan/status/{id}"
                     hx-trigger="every 2s"
                     hx-swap="outerHTML">
                    <div class="scan-header">
                        <div class="scan-repo-name">{name}</div>
                        <span class="scan-badge active">{status}</span>
                    </div>

                    <!-- Big prominent counter -->
                    <div class="scan-counter">
                        <span class="counter-current">{done}</span>
                        <span class="counter-sep">/</span>
                        <span class="counter-total">{total}</span>
                        <span class="counter-label">files</span>
                        {eta}
                    </div>

                    <!-- Progress bar -->
                    <div class="scan-progress-track">
                        <div class="scan-progress-fill" style="width:{percent:.1}%"></div>
                    </div>

                    <!-- Current file -->
                    <div class="scan-current-file">
                        {file}
                    </div>

                    {cost}
                </div>"#,
                id = repo.id,
                name = html_escape(&repo.name),
                status = status_label,
                done = repo.files_processed,
                total = repo.files_total,
                percent = percent,
                file = html_escape(&display_file),
                eta = eta_html,
                cost = cost_html,
            )
        }
        "error" => {
            let err_msg = repo.last_error.as_deref().unwrap_or("Unknown error");
            format!(
                r#"<div class="scan-card error"
                     hx-get="/scan/status/{id}"
                     hx-trigger="every 10s"
                     hx-swap="outerHTML">
                    <div class="scan-header">
                        <div class="scan-repo-name">{name}</div>
                        <span class="scan-badge error">❌ Error</span>
                    </div>
                    <div class="scan-error">{err}</div>
                    {cost}
                </div>"#,
                id = repo.id,
                name = html_escape(&repo.name),
                err = html_escape(err_msg),
                cost = cost_html,
            )
        }
        _ => {
            // Idle / completed
            let last_info = if let Some(dur) = repo.last_duration_ms {
                let dur_str = if dur > 60000 {
                    format!("{:.1}m", dur as f64 / 60000.0)
                } else {
                    format!("{:.1}s", dur as f64 / 1000.0)
                };
                format!(
                    r#"<div style="font-size:0.85rem;color:#64748b;margin-top:0.5rem;">
                        Last scan: {} issues · {} duration
                    </div>"#,
                    repo.issues_found, dur_str
                )
            } else {
                r#"<div style="font-size:0.85rem;color:#64748b;margin-top:0.5rem;">No scan data yet</div>"#.to_string()
            };

            format!(
                r#"<div class="scan-card idle"
                     hx-get="/scan/status/{id}"
                     hx-trigger="every 10s"
                     hx-swap="outerHTML">
                    <div class="scan-header">
                        <div class="scan-repo-name">{name}</div>
                        <span class="scan-badge idle">✅ Idle</span>
                    </div>
                    {last}
                    {cost}
                </div>"#,
                id = repo.id,
                name = html_escape(&repo.name),
                last = last_info,
                cost = cost_html,
            )
        }
    }
}

// ============================================================================
// GET /scan/dashboard — Full scan dashboard page
// ============================================================================

pub async fn scan_dashboard_handler(State(state): State<Arc<WebAppState>>) -> impl IntoResponse {
    let repos = get_scan_repos(&state.db.pool).await;

    let active_count = repos
        .iter()
        .filter(|r| matches!(r.scan_status.as_str(), "scanning" | "analyzing" | "cloning"))
        .count();

    let total_cost: f64 = repos.iter().map(|r| r.cost_accumulated).sum();
    let total_api: i64 = repos.iter().map(|r| r.api_calls).sum();
    let total_cache: i64 = repos.iter().map(|r| r.cache_hits).sum();
    let cache_rate = if total_api + total_cache > 0 {
        total_cache as f64 / (total_api + total_cache) as f64 * 100.0
    } else {
        0.0
    };

    let cards: String = repos.iter().map(render_repo_progress_card).collect();

    let content = format!(
        r#"<h2 style="margin-bottom:1rem;">📊 Scan Progress Dashboard</h2>

        <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:0.75rem;margin-bottom:1.5rem;">
            <div class="mini-stat">
                <div class="mini-val {active_color}">{active}</div>
                <div class="mini-label">Active Scans</div>
            </div>
            <div class="mini-stat">
                <div class="mini-val">{repos}</div>
                <div class="mini-label">Tracked Repos</div>
            </div>
            <div class="mini-stat">
                <div class="mini-val">${cost:.4}</div>
                <div class="mini-label">Total Cost</div>
            </div>
            <div class="mini-stat">
                <div class="mini-val">{cache_rate:.0}%</div>
                <div class="mini-label">Cache Hit Rate</div>
            </div>
        </div>

        <div id="scan-cards">
            {cards}
        </div>"#,
        active = active_count,
        active_color = if active_count > 0 { "scanning" } else { "" },
        repos = repos.len(),
        cost = total_cost,
        cache_rate = cache_rate,
        cards = cards,
    );

    let page = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Scan Progress — RustAssistant</title>
<script src="https://unpkg.com/htmx.org@1.9.10"></script>
<style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        background: #0f172a; color: #e2e8f0; line-height: 1.6; }}
    .container {{ max-width: 1200px; margin: 0 auto; padding: 1rem 2rem; }}
    nav {{ display: flex; gap: 0.5rem; flex-wrap: wrap; align-items: center;
        padding: 1rem 0; border-bottom: 1px solid #1e293b; margin-bottom: 1.5rem; }}
    nav a {{ color: #94a3b8; text-decoration: none; padding: 0.4rem 0.8rem;
        border-radius: 6px; font-size: 0.9rem; }}
    nav a:hover {{ color: #e2e8f0; background: #1e293b; }}
    nav a.active {{ color: #0ea5e9; background: #0c2d4a; font-weight: 600; }}
    h2 {{ font-size: 1.4rem; color: #f1f5f9; }}

    /* Mini stats */
    .mini-stat {{ background: #1e293b; border-radius: 8px; border: 1px solid #334155;
        padding: 0.75rem 1rem; text-align: center; }}
    .mini-val {{ font-size: 1.4rem; font-weight: 700; color: #0ea5e9; }}
    .mini-val.scanning {{ color: #fbbf24; animation: pulse 2s infinite; }}
    .mini-label {{ font-size: 0.75rem; color: #64748b; margin-top: 0.15rem; }}

    /* Scan cards */
    .scan-card {{ background: #1e293b; border-radius: 8px; border: 1px solid #334155;
        padding: 1.25rem; margin-bottom: 0.75rem; transition: border-color 0.3s; }}
    .scan-card.scanning {{ border-color: #0ea5e9; }}
    .scan-card.error {{ border-color: #ef4444; }}

    .scan-header {{ display: flex; justify-content: space-between; align-items: center;
        margin-bottom: 0.75rem; }}
    .scan-repo-name {{ font-size: 1.1rem; font-weight: 600; color: #f1f5f9; }}

    .scan-badge {{ padding: 0.2rem 0.6rem; border-radius: 4px; font-size: 0.75rem;
        font-weight: 600; }}
    .scan-badge.active {{ background: #0c2d4a; color: #38bdf8; animation: pulse 2s infinite; }}
    .scan-badge.error {{ background: #3b1111; color: #f87171; }}
    .scan-badge.idle {{ background: #0a2910; color: #4ade80; }}

    /* The big counter */
    .scan-counter {{ display: flex; align-items: baseline; gap: 0.25rem;
        margin-bottom: 0.75rem; }}
    .counter-current {{ font-size: 2.2rem; font-weight: 800; color: #0ea5e9;
        font-variant-numeric: tabular-nums; }}
    .counter-sep {{ font-size: 1.8rem; color: #475569; font-weight: 300; }}
    .counter-total {{ font-size: 1.8rem; font-weight: 600; color: #64748b;
        font-variant-numeric: tabular-nums; }}
    .counter-label {{ font-size: 0.85rem; color: #64748b; margin-left: 0.5rem; }}

    /* Progress bar */
    .scan-progress-track {{ width: 100%; height: 0.75rem; background: #0f172a;
        border-radius: 0.5rem; overflow: hidden; margin-bottom: 0.5rem; }}
    .scan-progress-fill {{ height: 100%; border-radius: 0.5rem; transition: width 0.5s ease;
        background: linear-gradient(90deg, #0ea5e9, #8b5cf6); }}

    /* Current file */
    .scan-current-file {{ font-family: 'JetBrains Mono', 'Fira Code', monospace;
        font-size: 0.8rem; color: #64748b; white-space: nowrap; overflow: hidden;
        text-overflow: ellipsis; }}

    .scan-error {{ background: rgba(239,68,68,0.1); border-left: 3px solid #ef4444;
        padding: 0.5rem 0.75rem; border-radius: 0 4px 4px 0; font-size: 0.85rem;
        color: #fca5a5; }}

    @keyframes pulse {{
        0%, 100% {{ opacity: 1; }}
        50% {{ opacity: 0.7; }}
    }}
</style>
</head>
<body>
<div class="container">
    {nav}
    {content}
</div>
</body>
</html>"#,
        nav = nav("Scan Progress"),
        content = content,
    );

    Html(page)
}

// ============================================================================
// GET /scan/status — HTMX partial: all repo cards
// ============================================================================

pub async fn scan_status_partial_handler(
    State(state): State<Arc<WebAppState>>,
) -> impl IntoResponse {
    let repos = get_scan_repos(&state.db.pool).await;
    let html: String = repos.iter().map(render_repo_progress_card).collect();
    Html(html)
}

// ============================================================================
// GET /scan/status/:repo_id — HTMX partial: single repo card
// ============================================================================

pub async fn scan_repo_status_handler(
    State(state): State<Arc<WebAppState>>,
    Path(repo_id): Path<String>,
) -> impl IntoResponse {
    let pool = &state.db.pool;

    #[allow(clippy::type_complexity)]
    let row: Option<(
        String,
        String,
        String,
        i64,
        i64,
        Option<String>,
        i64,
        Option<i64>,
        Option<String>,
        Option<i64>,
        f64,
        i64,
        i64,
    )> = sqlx::query_as(
        r#"SELECT
            id, name,
            COALESCE(scan_status, 'idle'),
            COALESCE(scan_files_processed, 0),
            COALESCE(scan_files_total, 0),
            scan_current_file,
            COALESCE(last_scan_issues_found, 0),
            last_scan_duration_ms,
            last_error,
            scan_started_at,
            COALESCE(scan_cost_accumulated, 0.0),
            COALESCE(scan_cache_hits, 0),
            COALESCE(scan_api_calls, 0)
        FROM repositories WHERE id = $1"#,
    )
    .bind(&repo_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    match row {
        Some((
            id,
            name,
            status,
            processed,
            total,
            current,
            issues,
            dur,
            err,
            started,
            cost,
            hits,
            calls,
        )) => {
            let repo = ScanRepoInfo {
                id,
                name,
                scan_status: status,
                files_processed: processed,
                files_total: total,
                current_file: current,
                issues_found: issues,
                last_duration_ms: dur,
                last_error: err,
                scan_started_at: started,
                cost_accumulated: cost,
                cache_hits: hits,
                api_calls: calls,
            };
            Html(render_repo_progress_card(&repo))
        }
        None => Html(
            r#"<div class="scan-card"><p style="color:#64748b;">Repo not found</p></div>"#
                .to_string(),
        ),
    }
}

// ============================================================================
// Public helper: render an inline progress bar for embedding in other pages
// (e.g., on the repos page or dashboard)
// ============================================================================

/// Render a compact inline progress indicator for a repo.
/// Call from other web_ui handlers and include the HTML directly.
pub fn render_inline_progress(
    repo_id: &str,
    _repo_name: &str,
    status: &str,
    processed: i64,
    total: i64,
    current_file: Option<&str>,
) -> String {
    let is_scanning = matches!(status, "scanning" | "analyzing" | "cloning");
    let percent = if total > 0 {
        (processed as f64 / total as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    if !is_scanning {
        return String::new();
    }

    let file_display = current_file
        .map(|f| {
            if f.len() > 60 {
                format!("…{}", &f[f.len() - 57..])
            } else {
                f.to_string()
            }
        })
        .unwrap_or_else(|| "starting...".to_string());

    format!(
        r#"<div id="inline-progress-{id}" hx-get="/scan/inline/{id}" hx-trigger="every 2s" hx-swap="outerHTML"
             style="margin-top:0.5rem;">
            <div style="display:flex;justify-content:space-between;align-items:baseline;margin-bottom:0.25rem;">
                <span style="font-size:1.1rem;font-weight:700;color:#0ea5e9;">
                    {done}<span style="color:#475569;font-weight:300;">/{total}</span>
                </span>
                <span style="font-size:0.8rem;color:#64748b;">{percent:.0}%</span>
            </div>
            <div style="width:100%;background:#0f172a;border-radius:0.25rem;height:0.5rem;overflow:hidden;">
                <div style="height:100%;background:linear-gradient(90deg,#0ea5e9,#8b5cf6);width:{percent:.1}%;transition:width 0.5s;"></div>
            </div>
            <div style="font-family:monospace;font-size:0.75rem;color:#475569;margin-top:0.25rem;
                        white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">
                {file}
            </div>
        </div>"#,
        id = html_escape(repo_id),
        done = processed,
        total = total,
        percent = percent,
        file = html_escape(&file_display),
    )
}

// ============================================================================
// GET /scan/inline/:repo_id — HTMX partial: compact inline progress
// Returns the same compact format as render_inline_progress so embedded
// progress bars don't get swapped out for the full dashboard card.
// ============================================================================

pub async fn scan_inline_status_handler(
    State(state): State<Arc<WebAppState>>,
    Path(repo_id): Path<String>,
) -> impl IntoResponse {
    let pool = &state.db.pool;

    let row: Option<(String, String, i64, i64, Option<String>)> = sqlx::query_as(
        r#"SELECT
            name,
            COALESCE(scan_status, 'idle'),
            COALESCE(scan_files_processed, 0),
            COALESCE(scan_files_total, 0),
            scan_current_file
        FROM repositories WHERE id = $1"#,
    )
    .bind(&repo_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    match row {
        Some((name, status, processed, total, current_file)) => {
            let html = render_inline_progress(
                &repo_id,
                &name,
                &status,
                processed,
                total,
                current_file.as_deref(),
            );
            // If no longer scanning, return a completion message instead
            if html.is_empty() {
                Html(format!(
                    r#"<div id="inline-progress-{}" style="margin-top:0.5rem;font-size:0.75rem;color:#4ade80;">✅ Scan complete</div>"#,
                    html_escape(&repo_id),
                ))
            } else {
                Html(html)
            }
        }
        None => Html(String::new()),
    }
}
