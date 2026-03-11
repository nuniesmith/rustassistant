//! Web UI — Full Audit
//!
//! Provides the `/audit` page family:
//!
//!   GET  /audit              — list all audit runs for all repos
//!   GET  /audit/new          — form to start a new audit (pick repo)
//!   POST /audit/start        — launch a new audit run (returns redirect)
//!   GET  /audit/:id          — live progress / final report page
//!   GET  /audit/:id/status   — HTMX partial: progress bar + live counts
//!   GET  /audit/:id/report   — raw Markdown report (download / view)
//!
//! All HTML is rendered as inline Rust format strings — same pattern used by
//! the rest of the web_ui_* modules.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use sqlx::PgPool;
use tracing::{error, info, warn};

use crate::audit::full_audit::{
    db_get_audit_report_markdown, db_get_audit_status, db_get_runs_for_repo, db_list_audit_runs,
    AuditRunStatus, AuditRunSummary, FullAuditEngine,
};
use crate::web_ui_nav;

// ============================================================================
// Shared state — thin wrapper that holds what we need
// ============================================================================

#[derive(Clone)]
pub struct AuditWebState {
    pub pool: PgPool,
    pub grok: Option<Arc<crate::grok_client::GrokClient>>,
}

impl AuditWebState {
    pub fn new(pool: PgPool, grok: Option<Arc<crate::grok_client::GrokClient>>) -> Self {
        Self { pool, grok }
    }
}

// ============================================================================
// Router
// ============================================================================

pub fn create_audit_router(state: AuditWebState) -> Router {
    Router::new()
        .route("/audit", get(audit_list_handler))
        .route("/audit/new", get(audit_new_handler))
        .route("/audit/start", post(audit_start_handler))
        .route("/audit/:id", get(audit_detail_handler))
        .route("/audit/:id/status", get(audit_status_partial_handler))
        .route("/audit/:id/report", get(audit_report_handler))
        .with_state(Arc::new(state))
}

#[derive(Deserialize, Default)]
pub struct AuditNewQuery {
    pub repo_id: Option<String>,
}

// ============================================================================
// List handler — /audit
// ============================================================================

pub async fn audit_list_handler(State(state): State<Arc<AuditWebState>>) -> impl IntoResponse {
    let runs = match db_list_audit_runs(&state.pool).await {
        Ok(r) => r,
        Err(e) => {
            error!(error = %e, "Failed to list audit runs");
            vec![]
        }
    };

    Html(render_audit_list_page(&runs))
}

fn render_audit_list_page(runs: &[AuditRunSummary]) -> String {
    let rows = if runs.is_empty() {
        r#"<tr><td colspan="8" style="text-align:center;color:#64748b;padding:2rem;">
            No audit runs yet — <a href="/audit/new" style="color:#38bdf8;">start one</a>.
        </td></tr>"#
            .to_string()
    } else {
        runs.iter()
            .map(|r| {
                let status_badge = status_badge(&r.status);
                let health = severity_health_summary(
                    r.findings_critical,
                    r.findings_high,
                    r.findings_medium,
                    r.findings_low,
                    r.findings_info,
                );
                let created = fmt_ts(r.created_at);
                let completed = r
                    .completed_at
                    .map(fmt_ts)
                    .unwrap_or_else(|| "—".to_string());
                let cost = if r.estimated_cost_usd > 0.001 {
                    format!("${:.4}", r.estimated_cost_usd)
                } else {
                    "—".to_string()
                };
                let progress = if r.files_total > 0 {
                    format!(
                        "{}/{}",
                        r.files_done, r.files_total
                    )
                } else {
                    "—".to_string()
                };
                format!(
                    r#"<tr>
                        <td><a href="/audit/{id}" style="color:#38bdf8;font-family:monospace;font-size:0.8rem;">{short_id}</a></td>
                        <td style="max-width:200px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;"
                            title="{path}">{repo}</td>
                        <td>{status_badge}</td>
                        <td>{progress}</td>
                        <td>{health}</td>
                        <td>{cost}</td>
                        <td>{created}</td>
                        <td>{completed}</td>
                    </tr>"#,
                    id = r.id,
                    short_id = short_id(&r.id),
                    path = html_esc(&r.repo_path),
                    repo = html_esc(&r.repo_name),
                    status_badge = status_badge,
                    progress = progress,
                    health = health,
                    cost = cost,
                    created = created,
                    completed = completed,
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let content = format!(
        r#"<div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1.5rem;">
    <div>
        <h1 style="font-size:1.6rem;color:#f1f5f9;margin-bottom:0.25rem;">🔬 Full Code Audits</h1>
        <p style="color:#94a3b8;font-size:0.9rem;">Deep per-file LLM analysis with master synthesis report</p>
    </div>
    <a href="/audit/new" class="btn btn-primary">+ New Audit</a>
</div>

<div class="card" style="overflow-x:auto;">
<table>
<thead>
<tr>
    <th>Run ID</th>
    <th>Repository</th>
    <th>Status</th>
    <th>Files</th>
    <th>Findings</th>
    <th>Cost</th>
    <th>Started</th>
    <th>Completed</th>
</tr>
</thead>
<tbody>
{rows}
</tbody>
</table>
</div>"#,
        rows = rows,
    );

    web_ui_nav::page_shell("Audits", "Audits", AUDIT_EXTRA_STYLES, &content)
}

// ============================================================================
// New audit form — /audit/new
// ============================================================================

pub async fn audit_new_handler(
    State(state): State<Arc<AuditWebState>>,
    axum::extract::Query(query): axum::extract::Query<AuditNewQuery>,
) -> impl IntoResponse {
    // Load registered repos from the DB so the user can pick one
    let repos = load_repos(&state.pool).await;
    Html(render_new_audit_page(&repos, query.repo_id.as_deref()))
}

struct RepoOption {
    id: String,
    name: String,
    path: String,
}

async fn load_repos(pool: &PgPool) -> Vec<RepoOption> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: String,
        name: String,
        local_path: Option<String>,
    }

    match sqlx::query_as::<_, Row>(
        "SELECT id, name, local_path FROM repositories ORDER BY name ASC",
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows
            .into_iter()
            .map(|r| RepoOption {
                id: r.id,
                name: r.name,
                path: r.local_path.unwrap_or_default(),
            })
            .collect(),
        Err(e) => {
            warn!(error = %e, "Failed to load repositories for audit form");
            vec![]
        }
    }
}

fn render_new_audit_page(repos: &[RepoOption], preselect_repo_id: Option<&str>) -> String {
    let repo_options = if repos.is_empty() {
        r#"<option disabled selected>— No repositories found — add one first —</option>"#
            .to_string()
    } else {
        repos
            .iter()
            .map(|r| {
                let selected = if preselect_repo_id == Some(r.id.as_str()) {
                    " selected"
                } else {
                    ""
                };
                format!(
                    r#"<option value="{id}" data-path="{path}"{selected}>{name}</option>"#,
                    id = html_esc(&r.id),
                    path = html_esc(&r.path),
                    selected = selected,
                    name = html_esc(&r.name),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    // Pre-fill the path input if a repo was pre-selected
    let prefill_path = preselect_repo_id
        .and_then(|rid| repos.iter().find(|r| r.id == rid))
        .map(|r| r.path.as_str())
        .unwrap_or("");

    let content = format!(
        r#"<div style="max-width:680px;margin:0 auto;">
<div style="margin-bottom:1.5rem;">
    <h1 style="font-size:1.6rem;color:#f1f5f9;margin-bottom:0.25rem;">🔬 New Full Audit</h1>
    <p style="color:#94a3b8;font-size:0.9rem;">
        A full audit reads every source file, scores it with the LLM, then produces a
        master synthesis covering broken code, scope drift, consolidation opportunities,
        and a prioritised action list.
    </p>
</div>

<div class="card">
<form method="post" action="/audit/start" id="audit-form">

    <div style="margin-bottom:1.25rem;">
        <label style="display:block;color:#94a3b8;font-size:0.85rem;margin-bottom:0.4rem;">
            Repository
        </label>
        <select name="repo_id" id="repo-select" required
                style="width:100%;padding:0.6rem 0.75rem;background:#0f172a;border:1px solid #334155;
                       border-radius:6px;color:#e2e8f0;font-size:0.9rem;"
                onchange="syncPath(this)">
            <option value="" disabled selected>— Select a repository —</option>
            {repo_options}
        </select>
    </div>

    <div style="margin-bottom:1.25rem;">
        <label style="display:block;color:#94a3b8;font-size:0.85rem;margin-bottom:0.4rem;">
            Repository Path
            <span style="color:#64748b;font-size:0.75rem;">(auto-filled or override)</span>
        </label>
        <input type="text" name="repo_path" id="repo-path" required
               placeholder="/path/to/repo"
               value="{prefill_path}"
               style="width:100%;padding:0.6rem 0.75rem;background:#0f172a;border:1px solid #334155;
                      border-radius:6px;color:#e2e8f0;font-size:0.9rem;font-family:monospace;" />
    </div>

    <div style="margin-bottom:1.25rem;">
        <label style="display:block;color:#94a3b8;font-size:0.85rem;margin-bottom:0.4rem;">
            Max Files to LLM-Score
            <span style="color:#64748b;font-size:0.75rem;">(larger = more cost)</span>
        </label>
        <input type="number" name="max_llm_files" value="200" min="1" max="2000"
               style="width:100%;padding:0.6rem 0.75rem;background:#0f172a;border:1px solid #334155;
                      border-radius:6px;color:#e2e8f0;font-size:0.9rem;" />
    </div>

    <div style="margin-bottom:1.25rem;">
        <label style="display:block;color:#94a3b8;font-size:0.85rem;margin-bottom:0.4rem;">
            Cost Cap (USD)
        </label>
        <input type="number" name="max_cost_usd" value="5.00" min="0.10" max="100" step="0.10"
               style="width:100%;padding:0.6rem 0.75rem;background:#0f172a;border:1px solid #334155;
                      border-radius:6px;color:#e2e8f0;font-size:0.9rem;" />
    </div>

    <div style="background:#0f172a;border:1px solid #1e3a5f;border-radius:6px;padding:1rem;margin-bottom:1.5rem;">
        <p style="color:#7dd3fc;font-size:0.85rem;font-weight:600;margin-bottom:0.5rem;">ℹ️ What this does</p>
        <ul style="color:#94a3b8;font-size:0.82rem;margin-left:1rem;line-height:1.8;">
            <li>Walks every source file in the repo (skips binaries, target/, node_modules/)</li>
            <li>Sends each file to the LLM for scoring (0-100) across 5 dimensions</li>
            <li>Tracks live progress — you can watch files being processed in real time</li>
            <li>Generates a master synthesis: broken code, scope drift, consolidation, deletion candidates</li>
            <li>Final report is stored and viewable as formatted Markdown</li>
        </ul>
    </div>

    <div style="display:flex;gap:1rem;flex-wrap:wrap;">
        <button type="submit" class="btn btn-primary" id="start-btn">🚀 Start Full Audit</button>
        <a href="/audit" class="btn btn-muted">Cancel</a>
    </div>
</form>
</div>
</div>

<script>
function syncPath(sel) {{
    const opt = sel.options[sel.selectedIndex];
    const path = opt.getAttribute('data-path') || '';
    document.getElementById('repo-path').value = path;
}}
document.getElementById('audit-form').addEventListener('submit', function() {{
    document.getElementById('start-btn').textContent = '⏳ Starting…';
    document.getElementById('start-btn').disabled = true;
}});
</script>"#,
        repo_options = repo_options,
        prefill_path = html_esc(prefill_path),
    );

    web_ui_nav::page_shell("New Audit", "Audits", AUDIT_EXTRA_STYLES, &content)
}

// ============================================================================
// Start handler — POST /audit/start
// ============================================================================

#[derive(Deserialize)]
pub struct StartAuditForm {
    pub repo_id: String,
    pub repo_path: String,
    pub max_llm_files: Option<usize>,
    pub max_cost_usd: Option<f64>,
}

pub async fn audit_start_handler(
    State(state): State<Arc<AuditWebState>>,
    Form(form): Form<StartAuditForm>,
) -> Response {
    // Validate path exists
    if form.repo_path.trim().is_empty() {
        return Html(render_error_page("Repository path cannot be empty.")).into_response();
    }
    if !std::path::Path::new(&form.repo_path).exists() {
        return Html(render_error_page(&format!(
            "Path does not exist: {}",
            form.repo_path
        )))
        .into_response();
    }

    // Look up repo name
    let repo_name = {
        #[derive(sqlx::FromRow)]
        struct Row {
            name: String,
        }
        match sqlx::query_as::<_, Row>("SELECT name FROM repositories WHERE id = $1")
            .bind(&form.repo_id)
            .fetch_optional(&state.pool)
            .await
        {
            Ok(Some(r)) => r.name,
            _ => {
                // Use last component of path as name
                std::path::Path::new(&form.repo_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            }
        }
    };

    // Build config
    let mut config = crate::audit::full_audit::FullAuditConfig::default();
    if let Some(max) = form.max_llm_files {
        config.max_llm_files = max.max(1);
    }
    if let Some(cost) = form.max_cost_usd {
        config.max_cost_usd = cost.max(0.01);
    }

    // Build engine and start
    let engine =
        Arc::new(FullAuditEngine::new(state.pool.clone(), state.grok.clone()).with_config(config));

    let repo_id = if form.repo_id.is_empty() {
        None
    } else {
        Some(form.repo_id.clone())
    };

    match engine
        .start_background(repo_id, form.repo_path.clone(), repo_name)
        .await
    {
        Ok(run_id) => {
            info!(run_id = %run_id, path = %form.repo_path, "Audit started");
            Redirect::to(&format!("/audit/{}", run_id)).into_response()
        }
        Err(e) => {
            error!(error = %e, "Failed to start audit");
            Html(render_error_page(&format!("Failed to start audit: {}", e))).into_response()
        }
    }
}

// ============================================================================
// Detail page — /audit/:id  (full page with live HTMX polling)
// ============================================================================

pub async fn audit_detail_handler(
    Path(run_id): Path<String>,
    State(state): State<Arc<AuditWebState>>,
) -> Response {
    let status = match db_get_audit_status(&state.pool, &run_id).await {
        Ok(Some(s)) => s,
        Ok(None) => return Html(render_error_page("Audit run not found.")).into_response(),
        Err(e) => {
            error!(error = %e, "Failed to load audit status");
            return Html(render_error_page("Database error loading audit run.")).into_response();
        }
    };

    Html(render_audit_detail_page(&status, &run_id)).into_response()
}

fn render_audit_detail_page(status: &AuditRunStatus, run_id: &str) -> String {
    let is_terminal = matches!(status.status.as_str(), "completed" | "failed");
    let poll_attr = if is_terminal {
        String::new()
    } else {
        // Build the HTMX polling attributes — split to avoid the # prefix ambiguity in raw strings
        let target = "#audit-progress-area";
        format!(
            r#"hx-get="/audit/{run_id}/status" hx-trigger="every 2s" hx-target="{target}" hx-swap="innerHTML""#,
            run_id = run_id,
            target = target,
        )
    };

    let progress_html = render_progress_area(status);

    // Report section (only when completed)
    let report_section = if status.status == "completed" {
        format!(
            r#"<div class="card" style="margin-top:1.5rem;">
    <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem;">
        <h2 style="font-size:1.2rem;color:#f1f5f9;">📄 Final Report</h2>
        <a href="/audit/{id}/report" class="btn btn-primary btn-sm" target="_blank">
            ⬇ View Raw Markdown
        </a>
    </div>
    <div id="report-embed"
         hx-get="/audit/{id}/report?embed=1"
         hx-trigger="load"
         hx-swap="innerHTML"
         style="min-height:200px;">
        <div style="color:#64748b;text-align:center;padding:2rem;">Loading report…</div>
    </div>
</div>"#,
            id = run_id,
        )
    } else {
        String::new()
    };

    let content = format!(
        r#"<div style="margin-bottom:1.5rem;">
    <div style="display:flex;align-items:center;gap:1rem;flex-wrap:wrap;">
        <div>
            <h1 style="font-size:1.5rem;color:#f1f5f9;margin-bottom:0.2rem;">
                🔬 Audit: {repo}
            </h1>
            <p style="color:#64748b;font-size:0.82rem;font-family:monospace;">{run_id}</p>
        </div>
        <div style="margin-left:auto;">
            {status_badge}
        </div>
    </div>
</div>

<div id="audit-progress-area" {poll_attr}>
    {progress_html}
</div>

{report_section}

<div style="margin-top:1.5rem;">
    <a href="/audit" class="btn btn-muted btn-sm">← All Audits</a>
    <a href="/audit/new" class="btn btn-primary btn-sm" style="margin-left:0.5rem;">+ New Audit</a>
</div>"#,
        repo = html_esc(&status.repo_name),
        run_id = html_esc(run_id),
        status_badge = status_badge(&status.status),
        poll_attr = poll_attr,
        progress_html = progress_html,
        report_section = report_section,
    );

    let extra = format!(
        "{}\n{}",
        AUDIT_EXTRA_STYLES,
        if !is_terminal {
            // Auto-reload the page when the run transitions to terminal
            r#"<script>
            document.addEventListener('htmx:afterSwap', function(e) {
                var area = document.getElementById('audit-progress-area');
                if (area && area.querySelector('.status-completed, .status-failed')) {
                    setTimeout(function() { location.reload(); }, 800);
                }
            });
            </script>"#
        } else {
            ""
        }
    );

    web_ui_nav::page_shell(
        &format!("Audit — {}", &status.repo_name),
        "Audits",
        &extra,
        &content,
    )
}

// ============================================================================
// HTMX status partial — /audit/:id/status
// ============================================================================

pub async fn audit_status_partial_handler(
    Path(run_id): Path<String>,
    State(state): State<Arc<AuditWebState>>,
) -> impl IntoResponse {
    match db_get_audit_status(&state.pool, &run_id).await {
        Ok(Some(status)) => Html(render_progress_area(&status)),
        Ok(None) => Html(
            r#"<div class="card" style="color:#ef4444;">Audit run not found.</div>"#.to_string(),
        ),
        Err(e) => Html(format!(
            r#"<div class="card" style="color:#ef4444;">Error: {}</div>"#,
            html_esc(&e.to_string())
        )),
    }
}

fn render_progress_area(status: &AuditRunStatus) -> String {
    let pct = if status.files_total > 0 {
        (status.files_done as f64 / status.files_total as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    let bar_color = match status.status.as_str() {
        "completed" => "linear-gradient(90deg,#22c55e,#16a34a)",
        "failed" => "linear-gradient(90deg,#ef4444,#dc2626)",
        _ => "linear-gradient(90deg,#0ea5e9,#8b5cf6)",
    };

    let current_file_html = match &status.current_file {
        Some(f) => format!(
            r#"<div style="font-family:monospace;font-size:0.75rem;color:#64748b;
                           margin-top:0.4rem;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">
                ↳ {}
            </div>"#,
            html_esc(f)
        ),
        None => String::new(),
    };

    let error_html = match &status.error_message {
        Some(e) => format!(
            r#"<div style="background:#2d1b1b;border:1px solid #7f1d1d;border-radius:6px;
                           padding:0.75rem 1rem;margin-top:1rem;color:#fca5a5;font-size:0.85rem;">
                ⚠️ <strong>Error:</strong> {}
            </div>"#,
            html_esc(e)
        ),
        None => String::new(),
    };

    let timing_html = {
        let started = status
            .started_at
            .map(fmt_ts)
            .unwrap_or_else(|| "—".to_string());
        let completed = status
            .completed_at
            .map(fmt_ts)
            .unwrap_or_else(|| "—".to_string());
        let duration = match (status.started_at, status.completed_at) {
            (Some(s), Some(c)) => format!("{:.1}s", (c - s) as f64),
            _ => "—".to_string(),
        };
        format!(
            r#"<div style="display:flex;gap:2rem;flex-wrap:wrap;margin-top:0.75rem;
                           font-size:0.82rem;color:#64748b;">
                <span>Started: <strong style="color:#94a3b8;">{started}</strong></span>
                <span>Completed: <strong style="color:#94a3b8;">{completed}</strong></span>
                <span>Duration: <strong style="color:#94a3b8;">{duration}</strong></span>
                <span>Cost: <strong style="color:#94a3b8;">${cost:.4}</strong></span>
            </div>"#,
            started = started,
            completed = completed,
            duration = duration,
            cost = status.estimated_cost_usd,
        )
    };

    // Severity pill row
    let severity_row = format!(
        r#"<div style="display:flex;gap:0.5rem;flex-wrap:wrap;margin-top:1rem;">
            {crit}
            {high}
            {med}
            {low}
            {info}
        </div>"#,
        crit = sev_pill(
            "Critical",
            "🔴",
            status.findings_critical,
            "#7f1d1d",
            "#fca5a5"
        ),
        high = sev_pill("High", "🟠", status.findings_high, "#7c2d12", "#fdba74"),
        med = sev_pill("Medium", "🟡", status.findings_medium, "#713f12", "#fde68a"),
        low = sev_pill("Low", "🔵", status.findings_low, "#1e3a5f", "#7dd3fc"),
        info = sev_pill("Info", "⚪", status.findings_info, "#1e293b", "#94a3b8"),
    );

    format!(
        r#"<div class="card status-{status_class}">
    <div style="display:flex;justify-content:space-between;align-items:baseline;margin-bottom:0.5rem;">
        <span style="font-size:1rem;font-weight:600;color:#e2e8f0;">
            {icon} {files_done}<span style="color:#475569;font-weight:300;"> / {files_total}</span> files
        </span>
        <span style="font-size:0.85rem;color:#94a3b8;">{pct:.1}%</span>
    </div>
    <div style="width:100%;background:#0f172a;border-radius:4px;height:10px;overflow:hidden;">
        <div style="height:100%;background:{bar_color};width:{pct:.2}%;transition:width 0.4s;border-radius:4px;"></div>
    </div>
    {current_file_html}
    {severity_row}
    {timing_html}
    {error_html}
</div>"#,
        status_class = html_esc(&status.status),
        icon = if status.status == "running" {
            "⏳"
        } else if status.status == "completed" {
            "✅"
        } else {
            "❌"
        },
        files_done = status.files_done,
        files_total = status.files_total,
        pct = pct,
        bar_color = bar_color,
        current_file_html = current_file_html,
        severity_row = severity_row,
        timing_html = timing_html,
        error_html = error_html,
    )
}

fn sev_pill(label: &str, emoji: &str, count: i32, bg: &str, color: &str) -> String {
    format!(
        r#"<span style="background:{bg};color:{color};padding:0.2rem 0.7rem;border-radius:999px;
                        font-size:0.78rem;font-weight:600;white-space:nowrap;">
            {emoji} {label}: {count}
        </span>"#,
        bg = bg,
        color = color,
        emoji = emoji,
        label = label,
        count = count,
    )
}

// ============================================================================
// Report viewer — /audit/:id/report
// ============================================================================

pub async fn audit_report_handler(
    Path(run_id): Path<String>,
    State(state): State<Arc<AuditWebState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let is_embed = params.get("embed").map(|v| v == "1").unwrap_or(false);

    match db_get_audit_report_markdown(&state.pool, &run_id).await {
        Ok(Some(md)) => {
            if is_embed {
                // Return HTML-rendered Markdown for HTMX embed
                Html(render_markdown_to_html(&md)).into_response()
            } else {
                // Return raw Markdown as plain text download
                axum::response::Response::builder()
                    .status(200)
                    .header("Content-Type", "text/markdown; charset=utf-8")
                    .header(
                        "Content-Disposition",
                        format!("inline; filename=\"audit-{}.md\"", &run_id[..8]),
                    )
                    .body(axum::body::Body::from(md))
                    .unwrap_or_else(|_| Html("Error building response".to_string()).into_response())
            }
        }
        Ok(None) => {
            // Run exists but report not yet written
            Html(
                r#"<div style="color:#64748b;text-align:center;padding:2rem;">
                ⏳ Report not yet available — audit still running or failed before completion.
            </div>"#
                    .to_string(),
            )
            .into_response()
        }
        Err(e) => {
            error!(error = %e, "Failed to fetch audit report");
            Html(render_error_page("Failed to load audit report.")).into_response()
        }
    }
}

/// Rudimentary Markdown → HTML renderer for the embedded report view.
/// Handles the subset of Markdown produced by `FullAuditReport::render_markdown`.
fn render_markdown_to_html(md: &str) -> String {
    let mut html = String::with_capacity(md.len() * 2);

    html.push_str(r#"<div class="md-report">"#);

    let mut in_table = false;
    let mut in_code = false;
    let mut in_ul = false;

    for line in md.lines() {
        let trimmed = line.trim();

        // Fenced code blocks
        if trimmed.starts_with("```") {
            if in_code {
                html.push_str("</code></pre>\n");
                in_code = false;
            } else {
                close_list(&mut html, &mut in_ul);
                close_table(&mut html, &mut in_table);
                html.push_str("<pre><code>");
                in_code = true;
            }
            continue;
        }
        if in_code {
            html.push_str(&html_esc(line));
            html.push('\n');
            continue;
        }

        // Tables
        if trimmed.starts_with('|') {
            close_list(&mut html, &mut in_ul);
            if !in_table {
                html.push_str(r#"<div style="overflow-x:auto;"><table>"#);
                in_table = true;
            }
            // Skip separator rows: |---|---|
            if trimmed
                .chars()
                .all(|c| c == '|' || c == '-' || c == ' ' || c == ':')
            {
                continue;
            }
            let cells: Vec<&str> = trimmed
                .trim_matches('|')
                .split('|')
                .map(|c| c.trim())
                .collect();
            html.push_str("<tr>");
            for cell in cells {
                let rendered = inline_md(cell);
                html.push_str(&format!("<td>{}</td>", rendered));
            }
            html.push_str("</tr>\n");
            continue;
        } else {
            close_table(&mut html, &mut in_table);
        }

        // Headings
        if let Some(rest) = trimmed.strip_prefix("#### ") {
            close_list(&mut html, &mut in_ul);
            html.push_str(&format!(
                "<h4 style='color:#94a3b8;margin:1.2rem 0 0.4rem;'>{}</h4>\n",
                inline_md(rest)
            ));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            close_list(&mut html, &mut in_ul);
            html.push_str(&format!(
                "<h3 style='color:#e2e8f0;margin:1.5rem 0 0.5rem;font-size:1rem;'>{}</h3>\n",
                inline_md(rest)
            ));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            close_list(&mut html, &mut in_ul);
            html.push_str(&format!(
                "<h2 style='color:#f1f5f9;margin:2rem 0 0.75rem;font-size:1.2rem;border-bottom:1px solid #334155;padding-bottom:0.4rem;'>{}</h2>\n",
                inline_md(rest)
            ));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            close_list(&mut html, &mut in_ul);
            html.push_str(&format!(
                "<h1 style='color:#38bdf8;margin:0 0 1rem;font-size:1.5rem;'>{}</h1>\n",
                inline_md(rest)
            ));
            continue;
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" {
            close_list(&mut html, &mut in_ul);
            html.push_str(
                "<hr style='border:none;border-top:1px solid #334155;margin:1.5rem 0;'>\n",
            );
            continue;
        }

        // Blockquote (> lines in header)
        if let Some(rest) = trimmed.strip_prefix("> ") {
            close_list(&mut html, &mut in_ul);
            html.push_str(&format!(
                "<p style='color:#64748b;font-size:0.85rem;margin:0.2rem 0;'>{}</p>\n",
                inline_md(rest)
            ));
            continue;
        }

        // List items
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if !in_ul {
                html.push_str(
                    "<ul style='margin:0.5rem 0 0.5rem 1.5rem;color:#94a3b8;line-height:1.8;'>\n",
                );
                in_ul = true;
            }
            let rest = &trimmed[2..];
            html.push_str(&format!("<li>{}</li>\n", inline_md(rest)));
            continue;
        } else {
            close_list(&mut html, &mut in_ul);
        }

        // Empty line
        if trimmed.is_empty() {
            html.push_str("<br>\n");
            continue;
        }

        // Paragraph
        html.push_str(&format!(
            "<p style='color:#cbd5e1;margin:0.4rem 0;font-size:0.9rem;'>{}</p>\n",
            inline_md(trimmed)
        ));
    }

    close_list(&mut html, &mut in_ul);
    close_table(&mut html, &mut in_table);
    if in_code {
        html.push_str("</code></pre>\n");
    }

    html.push_str("</div>");
    html
}

fn close_table(html: &mut String, in_table: &mut bool) {
    if *in_table {
        html.push_str("</table></div>\n");
        *in_table = false;
    }
}

fn close_list(html: &mut String, in_ul: &mut bool) {
    if *in_ul {
        html.push_str("</ul>\n");
        *in_ul = false;
    }
}

/// Render inline Markdown: **bold**, `code`, and bare text.
fn inline_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Bold: **...**
        if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            let end = s[i + 2..].find("**");
            if let Some(e) = end {
                out.push_str("<strong style='color:#e2e8f0;'>");
                out.push_str(&html_esc(&s[i + 2..i + 2 + e]));
                out.push_str("</strong>");
                i += e + 4; // skip **...** (open + content + close)
                continue;
            }
        }
        // Inline code: `...`
        if bytes[i] == b'`' {
            let end = s[i + 1..].find('`');
            if let Some(e) = end {
                out.push_str(
                    "<code style='background:#0f172a;padding:1px 5px;border-radius:3px;font-size:0.85em;color:#7dd3fc;'>",
                );
                out.push_str(&html_esc(&s[i + 1..i + 1 + e]));
                out.push_str("</code>");
                i += e + 2;
                continue;
            }
        }
        // Plain char — HTML-escape it
        let ch = s[i..].chars().next().unwrap_or(' ');
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
        i += ch.len_utf8();
    }
    out
}

// ============================================================================
// Audit list for repos page (used by web_ui.rs to embed in repo cards)
// ============================================================================

/// Returns a small HTML snippet showing the last N audits for a repo.
/// Used to embed in the repository detail card.
pub async fn render_repo_audit_history(pool: &PgPool, repo_id: &str) -> String {
    let runs = match db_get_runs_for_repo(pool, repo_id).await {
        Ok(r) => r,
        Err(_) => return String::new(),
    };

    if runs.is_empty() {
        return r#"<div style="color:#475569;font-size:0.82rem;">No audits yet.
            <a href="/audit/new" style="color:#38bdf8;">Start one</a></div>"#
            .to_string();
    }

    let rows: String = runs
        .iter()
        .take(5)
        .map(|r| {
            let status_b = status_badge(&r.status);
            let created = fmt_ts(r.created_at);
            let health = if r.status == "completed" {
                severity_health_summary(
                    r.findings_critical,
                    r.findings_high,
                    r.findings_medium,
                    r.findings_low,
                    r.findings_info,
                )
            } else {
                String::new()
            };
            format!(
                r#"<div style="display:flex;align-items:center;gap:0.75rem;padding:0.4rem 0;
                               border-bottom:1px solid #1e293b;font-size:0.82rem;">
                    <a href="/audit/{id}" style="color:#38bdf8;font-family:monospace;">{short}</a>
                    {status_b}
                    <span style="color:#64748b;">{created}</span>
                    <span style="margin-left:auto;">{health}</span>
                </div>"#,
                id = r.id,
                short = short_id(&r.id),
                status_b = status_b,
                created = created,
                health = health,
            )
        })
        .collect();

    format!(
        r#"<div style="margin-top:0.75rem;">
        <div style="font-size:0.8rem;color:#64748b;margin-bottom:0.4rem;text-transform:uppercase;
                    letter-spacing:0.05em;">Recent Audits</div>
        {rows}
        <a href="/audit/new" class="btn btn-primary btn-sm"
           style="margin-top:0.75rem;font-size:0.75rem;padding:0.3rem 0.8rem;">
            🔬 New Audit
        </a>
    </div>"#,
        rows = rows,
    )
}

// ============================================================================
// Utility helpers
// ============================================================================

fn status_badge(status: &str) -> String {
    let (bg, color, label) = match status {
        "running" => ("#1e3a5f", "#7dd3fc", "⏳ Running"),
        "completed" => ("#14532d", "#86efac", "✅ Completed"),
        "failed" => ("#7f1d1d", "#fca5a5", "❌ Failed"),
        "pending" => ("#1e293b", "#94a3b8", "⏸ Pending"),
        _ => ("#1e293b", "#94a3b8", status),
    };
    format!(
        r#"<span style="background:{bg};color:{color};padding:0.2rem 0.65rem;
                        border-radius:999px;font-size:0.75rem;font-weight:600;white-space:nowrap;">
            {label}
        </span>"#,
        bg = bg,
        color = color,
        label = label,
    )
}

fn severity_health_summary(crit: i32, high: i32, med: i32, _low: i32, _info: i32) -> String {
    if crit > 0 {
        format!(
            r#"<span style="color:#fca5a5;font-size:0.78rem;font-weight:600;">🔴 {} crit</span>"#,
            crit
        )
    } else if high > 0 {
        format!(
            r#"<span style="color:#fdba74;font-size:0.78rem;font-weight:600;">🟠 {} high</span>"#,
            high
        )
    } else if med > 0 {
        format!(
            r#"<span style="color:#fde68a;font-size:0.78rem;font-weight:600;">🟡 {} med</span>"#,
            med
        )
    } else {
        r#"<span style="color:#86efac;font-size:0.78rem;font-weight:600;">✅ Clean</span>"#
            .to_string()
    }
}

fn html_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fmt_ts(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt: chrono::DateTime<chrono::Utc>| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn short_id(id: &str) -> &str {
    &id[..id.len().min(8)]
}

fn render_error_page(msg: &str) -> String {
    let content = format!(
        r#"<div class="card" style="border-left-color:#ef4444;max-width:600px;margin:2rem auto;">
    <h2 style="color:#ef4444;margin-bottom:0.75rem;">❌ Error</h2>
    <p style="color:#94a3b8;">{}</p>
    <div style="margin-top:1.5rem;">
        <a href="/audit" class="btn btn-muted btn-sm">← Back to Audits</a>
    </div>
</div>"#,
        html_esc(msg)
    );
    web_ui_nav::page_shell("Audit Error", "Audits", AUDIT_EXTRA_STYLES, &content)
}

const AUDIT_EXTRA_STYLES: &str = r#"<style>
/* ── Audit-specific overrides ─────────────────────────────────── */
.md-report {
    font-size: 0.9rem;
    line-height: 1.7;
    color: #cbd5e1;
}
.md-report pre {
    background: #0f172a;
    border: 1px solid #334155;
    border-radius: 6px;
    padding: 1rem;
    overflow-x: auto;
    font-size: 0.82rem;
    color: #7dd3fc;
    margin: 0.75rem 0;
}
.md-report code {
    font-family: 'JetBrains Mono', 'Fira Code', monospace;
}
.md-report table {
    font-size: 0.82rem;
}
.md-report td, .md-report th {
    border-bottom: 1px solid #1e293b;
}
.status-running  { border-left: 4px solid #0ea5e9; }
.status-completed{ border-left: 4px solid #22c55e; }
.status-failed   { border-left: 4px solid #ef4444; }
.status-pending  { border-left: 4px solid #475569; }
</style>"#;
