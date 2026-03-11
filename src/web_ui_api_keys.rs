// ============================================================================
// API Keys Page — Web UI Module
// ============================================================================
//
// Provides a web UI for managing RustAssistant proxy API keys.
// Keys generated here are stored in the `api_keys` table and their SHA-256
// hashes are accepted by the `/v1/chat/completions` proxy endpoint.
//
// Routes:
//   GET  /api-keys                — Full page
//   GET  /api-keys/list           — HTMX partial: key table
//   POST /api-keys/create         — HTMX partial: create + refresh table
//   DELETE /api-keys/:id/revoke   — HTMX partial: refresh table
//
// Integration:
//   In src/lib.rs:
//     pub mod web_ui_api_keys;
//
//   In src/server.rs:
//     use rustassistant::web_ui_api_keys::create_api_keys_router;
//     .merge(create_api_keys_router(db_pool))

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
    Form, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info};

use crate::web_ui_nav;

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
pub struct ApiKeysState {
    pub db: PgPool,
    /// Base URL that Zed / external clients should point at.
    /// Read from `PUBLIC_BASE_URL` env var; defaults to the Tailscale IP.
    pub public_base_url: String,
}

impl ApiKeysState {
    pub fn new(db: PgPool) -> Self {
        let public_base_url = std::env::var("PUBLIC_BASE_URL")
            .unwrap_or_else(|_| "http://100.113.72.63:8080".to_string());
        Self {
            db,
            public_base_url,
        }
    }
}

// ============================================================================
// Router
// ============================================================================

pub fn create_api_keys_router(state: Arc<ApiKeysState>) -> Router {
    Router::new()
        .route("/api-keys", get(page_handler))
        .route("/api-keys/list", get(list_partial_handler))
        .route("/api-keys/create", post(create_handler))
        .route("/api-keys/:id/revoke", delete(revoke_handler))
        .with_state(state)
}

// ============================================================================
// DB types
// ============================================================================

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ApiKeyRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub key_prefix: String,
    pub created_at: String,
    pub last_used: Option<String>,
    pub request_count: i64,
}

// ============================================================================
// Form types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateKeyForm {
    pub name: String,
    pub description: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api-keys — full page
pub async fn page_handler(State(state): State<Arc<ApiKeysState>>) -> impl IntoResponse {
    let keys = fetch_keys(&state.db).await;
    let table_html = render_key_table(&keys);
    let zed_snippet = render_zed_snippet(&state.public_base_url);

    let content = format!(
        r##"
        <div class="page-header">
            <h1>🔑 API Keys</h1>
            <p class="subtitle">Manage bearer tokens for the OpenAI-compatible proxy endpoint
                (<code>/v1/chat/completions</code>).
                Use these keys with Zed, <code>curl</code>, or any OpenAI-SDK client.</p>
        </div>

        <!-- ── Zed Configuration ──────────────────────────────────────── -->
        <div class="card zed-card">
            <h2>🧩 Zed IDE Configuration</h2>
            <p class="card-desc">
                Add this to your Zed <code>settings.json</code>
                (<kbd>Cmd/Ctrl+Shift+P</kbd> → <em>Open Settings</em>),
                replacing <code>YOUR_KEY_HERE</code> with a key you generate below.
            </p>
            {zed_snippet}
            <div class="zed-note">
                <span class="badge badge-info">ℹ️ Tip</span>
                In Zed, set the provider to <strong>OpenAI</strong> and paste the base URL above.
                Streaming is fully supported — Zed's default <code>stream: true</code> works out of the box.
            </div>
        </div>

        <!-- ── Create New Key ─────────────────────────────────────────── -->
        <div class="card">
            <h2>➕ Generate New Key</h2>
            <form hx-post="/api-keys/create"
                  hx-target="#keys-table-container"
                  hx-swap="innerHTML"
                  hx-on::after-request="this.reset(); showNewKeyBanner(event)"
                  class="create-form">
                <div class="form-row">
                    <div class="form-group">
                        <label for="key-name">Key Name <span class="required">*</span></label>
                        <input type="text" id="key-name" name="name" required
                               placeholder="e.g. zed-oryx, curl-test, futures-bot"
                               class="form-input" maxlength="80">
                    </div>
                    <div class="form-group">
                        <label for="key-desc">Description <span class="optional">(optional)</span></label>
                        <input type="text" id="key-desc" name="description"
                               placeholder="What is this key used for?"
                               class="form-input" maxlength="200">
                    </div>
                    <div class="form-group form-group-btn">
                        <label>&nbsp;</label>
                        <button type="submit" class="btn btn-primary">
                            🔑 Generate Key
                        </button>
                    </div>
                </div>
            </form>
        </div>

        <!-- ── New Key Banner (shown once after creation) ─────────────── -->
        <div id="new-key-banner" class="new-key-banner hidden" role="alert">
            <div class="banner-header">
                <span class="banner-icon">✅</span>
                <strong>Key created — copy it now, it will not be shown again.</strong>
            </div>
            <div class="key-display">
                <code id="new-key-value" class="key-value"></code>
                <button class="btn btn-sm btn-muted copy-btn" onclick="copyKey()">📋 Copy</button>
            </div>
            <button class="banner-close" onclick="dismissBanner()">✕</button>
        </div>

        <!-- ── Keys Table ─────────────────────────────────────────────── -->
        <div class="card">
            <div class="card-header-row">
                <h2>🗝️ Active Keys</h2>
                <button class="btn btn-sm btn-muted"
                        hx-get="/api-keys/list"
                        hx-target="#keys-table-container"
                        hx-swap="innerHTML">
                    🔄 Refresh
                </button>
            </div>
            <div id="keys-table-container">
                {table_html}
            </div>
        </div>

        <!-- ── Environment Variable Info ──────────────────────────────── -->
        <div class="card info-card">
            <h2>⚙️ Server Configuration</h2>
            <p class="card-desc">
                Keys generated here are stored in the database with their SHA-256 hashes.
                The proxy endpoint also accepts keys set directly via the
                <code>RA_PROXY_API_KEYS</code> environment variable (comma-separated).
                Both sources are checked independently.
            </p>
            <div class="env-block">
                <code>RA_PROXY_API_KEYS=key1,key2,...</code>
                <span class="env-note">Set this in your <code>.env</code> file or systemd unit for static keys.</span>
            </div>
            <div class="env-block">
                <code>PUBLIC_BASE_URL=http://100.113.72.63:8080</code>
                <span class="env-note">Override the base URL shown in the Zed snippet above.</span>
            </div>
        </div>
        "##,
        zed_snippet = zed_snippet,
        table_html = table_html,
    );

    let extra_head = api_keys_styles();

    Html(web_ui_nav::page_shell(
        "API Keys", "API Keys", extra_head, &content,
    ))
}

/// GET /api-keys/list — HTMX partial: just the table
pub async fn list_partial_handler(State(state): State<Arc<ApiKeysState>>) -> impl IntoResponse {
    let keys = fetch_keys(&state.db).await;
    Html(render_key_table(&keys))
}

/// POST /api-keys/create — creates a key, returns updated table HTML + banner data
pub async fn create_handler(
    State(state): State<Arc<ApiKeysState>>,
    Form(form): Form<CreateKeyForm>,
) -> Response {
    let name = form.name.trim().to_string();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Html("<p class='error-msg'>Key name is required.</p>".to_string()),
        )
            .into_response();
    }

    let raw_key = crate::api::auth::generate_api_key();
    let key_hash = crate::api::auth::hash_api_key(&raw_key);
    let prefix = raw_key[..8].to_string();
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();

    let insert = sqlx::query(
        "INSERT INTO api_keys \
         (id, name, description, key_hash, key_prefix, created_at, request_count) \
         VALUES ($1, $2, $3, $4, $5, $6, 0)",
    )
    .bind(&id)
    .bind(&name)
    .bind(&form.description)
    .bind(&key_hash)
    .bind(&prefix)
    .bind(&created_at)
    .execute(&state.db)
    .await;

    if let Err(e) = insert {
        error!(error = %e, "Failed to insert API key");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("<p class='error-msg'>Database error — could not create key.</p>".to_string()),
        )
            .into_response();
    }

    info!(name = %name, prefix = %prefix, "API key created");

    // Return the refreshed table with an HX-Trigger header so the banner JS fires.
    let keys = fetch_keys(&state.db).await;
    let table_html = render_key_table(&keys);

    // Embed the raw key in a JSON trigger so the frontend can display it.
    let trigger_json = format!(
        r#"{{"newKeyCreated":{{"key":"{}","name":"{}"}}}}"#,
        raw_key, name
    );

    let mut response = (StatusCode::OK, Html(table_html)).into_response();
    if let Ok(header_val) = trigger_json.parse() {
        response.headers_mut().insert("HX-Trigger", header_val);
    }
    response
}

/// DELETE /api-keys/:id/revoke — revokes a key, returns updated table HTML
pub async fn revoke_handler(
    State(state): State<Arc<ApiKeysState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let rows = sqlx::query("DELETE FROM api_keys WHERE id = $1")
        .bind(&id)
        .execute(&state.db)
        .await;

    match rows {
        Err(e) => {
            error!(error = %e, id = %id, "Failed to revoke API key");
            return Html(
                "<p class='error-msg'>Database error — could not revoke key.</p>".to_string(),
            );
        }
        Ok(r) if r.rows_affected() == 0 => {
            return Html("<p class='error-msg'>Key not found.</p>".to_string());
        }
        Ok(_) => {
            info!(id = %id, "API key revoked");
        }
    }

    let keys = fetch_keys(&state.db).await;
    Html(render_key_table(&keys))
}

// ============================================================================
// DB helpers
// ============================================================================

async fn fetch_keys(db: &PgPool) -> Vec<ApiKeyRow> {
    sqlx::query_as::<_, ApiKeyRow>(
        "SELECT id, name, description, key_prefix, created_at, last_used, request_count \
         FROM api_keys \
         ORDER BY created_at DESC",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default()
}

// ============================================================================
// HTML rendering helpers
// ============================================================================

fn render_key_table(keys: &[ApiKeyRow]) -> String {
    if keys.is_empty() {
        return r#"
            <div class="empty-state">
                <span class="empty-icon">🔑</span>
                <p>No API keys yet. Generate one above to get started.</p>
            </div>"#
            .to_string();
    }

    let rows: String = keys
        .iter()
        .map(|k| {
            let last_used = k
                .last_used
                .as_deref()
                .and_then(|s| s.parse::<DateTime<Utc>>().ok())
                .map(|dt| format_relative_time(dt))
                .unwrap_or_else(|| "Never".to_string());

            let created = k
                .created_at
                .parse::<DateTime<Utc>>()
                .map(|dt| format_relative_time(dt))
                .unwrap_or_else(|_| k.created_at.clone());

            let desc = k
                .description
                .as_deref()
                .map(|d| format!(r#"<span class="key-desc">{}</span>"#, html_escape(d)))
                .unwrap_or_default();

            let target = "#keys-table-container";
            let confirm_msg = format!(
                "Revoke key &apos;{}&apos;? This cannot be undone.",
                html_escape(&k.name)
            );
            format!(
                r#"<tr>
                    <td>
                        <div class="key-name">{name}</div>
                        {desc}
                    </td>
                    <td><code class="key-prefix">{prefix}&#8230;</code></td>
                    <td class="ts-cell" title="{created_raw}">{created}</td>
                    <td class="ts-cell">{last_used}</td>
                    <td class="count-cell">{count}</td>
                    <td>
                        <button class="btn btn-sm btn-danger"
                                hx-delete="/api-keys/{id}/revoke"
                                hx-target="{target}"
                                hx-swap="innerHTML"
                                hx-confirm="{confirm_msg}">
                            Revoke
                        </button>
                    </td>
                </tr>"#,
                name = html_escape(&k.name),
                desc = desc,
                prefix = html_escape(&k.key_prefix),
                created = created,
                created_raw = html_escape(&k.created_at),
                last_used = last_used,
                count = k.request_count,
                id = html_escape(&k.id),
                target = target,
                confirm_msg = confirm_msg,
            )
        })
        .collect();

    format!(
        r#"<div class="table-wrapper">
            <table>
                <thead>
                    <tr>
                        <th>Name</th>
                        <th>Key Prefix</th>
                        <th>Created</th>
                        <th>Last Used</th>
                        <th>Requests</th>
                        <th>Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {rows}
                </tbody>
            </table>
        </div>"#,
        rows = rows,
    )
}

fn render_zed_snippet(base_url: &str) -> String {
    let v1_url = format!("{}/v1", base_url.trim_end_matches('/'));
    let json = format!(
        r#"{{
  "language_models": {{
    "openai": {{
      "api_url": "{v1_url}",
      "available_models": [
        {{
          "name": "auto",
          "display_name": "RustAssistant (auto)",
          "max_tokens": 8192
        }},
        {{
          "name": "local",
          "display_name": "RustAssistant (local/Ollama)",
          "max_tokens": 8192
        }},
        {{
          "name": "remote",
          "display_name": "RustAssistant (remote/Grok)",
          "max_tokens": 32768
        }}
      ]
    }}
  }},
  "assistant": {{
    "default_model": {{
      "provider": "openai",
      "model": "auto"
    }},
    "version": "2"
  }}
}}"#,
        v1_url = v1_url,
    );

    format!(
        r#"<div class="snippet-block">
            <div class="snippet-header">
                <span class="snippet-label">~/.config/zed/settings.json</span>
                <button class="btn btn-sm btn-muted" onclick="copySnippet()">📋 Copy</button>
            </div>
            <pre id="zed-snippet-text" class="snippet-pre"><code>{json_escaped}</code></pre>
            <div class="snippet-footer">
                <span class="badge badge-warning">⚠️ Replace <code>YOUR_KEY_HERE</code> with a generated key</span>
                &nbsp;&mdash;&nbsp;
                <span class="badge badge-success">✅ Streaming (SSE) supported</span>
            </div>
        </div>
        <div class="snippet-block snippet-secondary">
            <div class="snippet-header">
                <span class="snippet-label">Zed AI Provider Settings (UI path)</span>
            </div>
            <ol class="zed-steps">
                <li>Open Zed → <kbd>Cmd/Ctrl+Shift+P</kbd> → <em>Open Settings</em></li>
                <li>Paste the JSON above (or merge with your existing settings)</li>
                <li>Add your API key: <kbd>Cmd/Ctrl+Shift+P</kbd> → <em>Assistant: Configure</em> → set the OpenAI key to your generated key</li>
                <li>Open the assistant panel with <kbd>Cmd/Ctrl+?</kbd></li>
            </ol>
        </div>"#,
        json_escaped = html_escape(&json),
    )
}

// ============================================================================
// Utilities
// ============================================================================

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn format_relative_time(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);
    let secs = diff.num_seconds();
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 86400 * 30 {
        format!("{}d ago", secs / 86400)
    } else {
        dt.format("%Y-%m-%d").to_string()
    }
}

// ============================================================================
// Styles + JS
// ============================================================================

fn api_keys_styles() -> &'static str {
    r#"<style>
    /* ── Page layout ──────────────────────────────────────────────── */
    .page-header { margin: 1.5rem 0 1rem; }
    .page-header h1 { font-size: 1.8rem; color: #f1f5f9; margin-bottom: 0.35rem; }
    .page-header .subtitle { color: #94a3b8; font-size: 0.95rem; }
    .page-header code { background: #0f172a; padding: 1px 5px; border-radius: 4px;
        color: #7dd3fc; font-size: 0.9em; }

    .card-header-row { display: flex; align-items: center;
        justify-content: space-between; margin-bottom: 1rem; }
    .card-header-row h2 { margin-bottom: 0; }
    .card-desc { color: #94a3b8; font-size: 0.9rem; margin-bottom: 1rem; line-height: 1.5; }
    .card-desc code { background: #0f172a; padding: 1px 5px; border-radius: 4px;
        color: #7dd3fc; font-size: 0.9em; }

    /* ── Zed card ─────────────────────────────────────────────────── */
    .zed-card { border-left: 4px solid #38bdf8; }
    .zed-card h2 { color: #38bdf8; }

    .snippet-block { background: #0f172a; border-radius: 6px;
        border: 1px solid #334155; margin-bottom: 1rem; overflow: hidden; }
    .snippet-block.snippet-secondary { border-color: #1e3a5f; }
    .snippet-header { display: flex; align-items: center; justify-content: space-between;
        padding: 0.5rem 1rem; background: #1e293b; border-bottom: 1px solid #334155; }
    .snippet-label { color: #64748b; font-size: 0.8rem; font-family: monospace; }
    .snippet-pre { margin: 0; padding: 1rem; overflow-x: auto;
        font-size: 0.82rem; line-height: 1.6; color: #e2e8f0; }
    .snippet-pre code { font-family: 'JetBrains Mono', 'Fira Code', monospace; }
    .snippet-footer { padding: 0.5rem 1rem; border-top: 1px solid #1e293b;
        display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;
        font-size: 0.82rem; }
    .snippet-footer code { background: #1e293b; padding: 1px 5px; border-radius: 4px;
        color: #fbbf24; }

    .zed-steps { margin: 0.75rem 1rem 0.75rem 2rem; color: #94a3b8;
        font-size: 0.9rem; line-height: 1.8; }
    .zed-steps kbd { background: #1e293b; border: 1px solid #475569;
        border-radius: 4px; padding: 1px 6px; font-size: 0.82rem; color: #e2e8f0; }
    .zed-steps em { color: #7dd3fc; font-style: normal; }
    .zed-note { margin-top: 0.75rem; padding: 0.6rem 0.9rem;
        background: #0c2d4a; border-radius: 6px; color: #7dd3fc;
        font-size: 0.85rem; }

    /* ── Create form ──────────────────────────────────────────────── */
    .create-form { padding: 0.25rem 0; }
    .form-row { display: flex; gap: 1rem; align-items: flex-end; flex-wrap: wrap; }
    .form-group { display: flex; flex-direction: column; flex: 1; min-width: 200px; }
    .form-group-btn { flex: 0 0 auto; min-width: unset; }
    .form-group label { color: #94a3b8; font-size: 0.82rem; font-weight: 600;
        margin-bottom: 0.35rem; text-transform: uppercase; letter-spacing: 0.05em; }
    .form-input { background: #0f172a; border: 1px solid #334155; color: #e2e8f0;
        padding: 0.55rem 0.75rem; border-radius: 6px; font-size: 0.9rem; width: 100%;
        transition: border-color 0.2s; }
    .form-input:focus { outline: none; border-color: #0ea5e9;
        box-shadow: 0 0 0 2px rgba(14,165,233,0.15); }
    .form-input::placeholder { color: #475569; }
    .required { color: #ef4444; }
    .optional { color: #475569; font-weight: 400; font-size: 0.78rem; }

    /* ── New key banner ───────────────────────────────────────────── */
    .new-key-banner { position: relative; background: #052e16;
        border: 1px solid #16a34a; border-radius: 8px;
        padding: 1rem 1.25rem; margin-bottom: 1rem; }
    .new-key-banner.hidden { display: none; }
    .banner-header { display: flex; align-items: center; gap: 0.5rem;
        color: #22c55e; font-size: 0.95rem; margin-bottom: 0.75rem; }
    .banner-icon { font-size: 1.1rem; }
    .key-display { display: flex; align-items: center; gap: 0.75rem;
        background: #0f172a; padding: 0.6rem 0.9rem; border-radius: 6px; }
    .key-value { font-family: 'JetBrains Mono', 'Fira Code', monospace;
        font-size: 0.9rem; color: #4ade80; word-break: break-all; flex: 1; }
    .copy-btn { flex-shrink: 0; }
    .banner-close { position: absolute; top: 0.75rem; right: 0.75rem;
        background: none; border: none; color: #4ade80; cursor: pointer;
        font-size: 1rem; line-height: 1; padding: 0.2rem; opacity: 0.7; }
    .banner-close:hover { opacity: 1; }

    /* ── Keys table ───────────────────────────────────────────────── */
    .table-wrapper { overflow-x: auto; }
    .key-name { font-weight: 600; color: #e2e8f0; font-size: 0.9rem; }
    .key-desc { display: block; color: #64748b; font-size: 0.78rem; margin-top: 2px; }
    .key-prefix { font-family: 'JetBrains Mono', 'Fira Code', monospace;
        background: #0f172a; padding: 2px 6px; border-radius: 4px;
        color: #7dd3fc; font-size: 0.82rem; }
    .ts-cell { color: #64748b; font-size: 0.82rem; white-space: nowrap; }
    .count-cell { color: #94a3b8; font-size: 0.85rem; text-align: right;
        padding-right: 1.5rem !important; }

    .empty-state { text-align: center; padding: 2.5rem; color: #475569; }
    .empty-icon { font-size: 2.5rem; display: block; margin-bottom: 0.5rem; }

    /* ── Info card ────────────────────────────────────────────────── */
    .info-card { border-left: 4px solid #475569; }
    .env-block { background: #0f172a; border-radius: 6px; padding: 0.6rem 0.9rem;
        margin: 0.5rem 0; display: flex; align-items: baseline; gap: 1rem;
        flex-wrap: wrap; border: 1px solid #1e293b; }
    .env-block code { font-family: 'JetBrains Mono', 'Fira Code', monospace;
        color: #4ade80; font-size: 0.85rem; flex-shrink: 0; }
    .env-note { color: #64748b; font-size: 0.82rem; }
    .env-note code { color: #fbbf24; background: #1e293b; padding: 1px 4px;
        border-radius: 3px; }

    .error-msg { color: #ef4444; padding: 0.5rem; font-size: 0.9rem; }
    </style>

    <script>
    // Called by htmx's hx-on::after-request on the create form.
    // htmx fires the HX-Trigger header as a JS CustomEvent.
    document.body.addEventListener('newKeyCreated', function(evt) {
        var detail = evt.detail;
        if (!detail || !detail.key) return;
        var banner = document.getElementById('new-key-banner');
        var keyEl  = document.getElementById('new-key-value');
        if (banner && keyEl) {
            keyEl.textContent = detail.key;
            banner.classList.remove('hidden');
            banner.scrollIntoView({ behavior: 'smooth', block: 'center' });
        }
    });

    function dismissBanner() {
        var banner = document.getElementById('new-key-banner');
        if (banner) banner.classList.add('hidden');
    }

    function copyKey() {
        var key = document.getElementById('new-key-value');
        if (!key) return;
        navigator.clipboard.writeText(key.textContent.trim()).then(function() {
            var btn = document.querySelector('.copy-btn');
            if (btn) {
                var orig = btn.textContent;
                btn.textContent = '✅ Copied!';
                setTimeout(function() { btn.textContent = orig; }, 2000);
            }
        });
    }

    function copySnippet() {
        var pre = document.getElementById('zed-snippet-text');
        if (!pre) return;
        navigator.clipboard.writeText(pre.innerText.trim()).then(function() {
            var btns = document.querySelectorAll('.snippet-header .btn');
            btns.forEach(function(btn) {
                var orig = btn.textContent;
                btn.textContent = '✅ Copied!';
                setTimeout(function() { btn.textContent = orig; }, 2000);
            });
        });
    }
    </script>"#
}
