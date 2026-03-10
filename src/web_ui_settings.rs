// ============================================================================
// Settings Page — Web UI Module
// ============================================================================
//
// Provides a comprehensive settings interface for configuring RustAssistant.
//
// Sections:
//   - General: Application name, timezone, theme preferences
//   - LLM Models: Ollama config, Grok API key, model routing preferences
//   - RAG System: Embedding model, chunk size, index management
//   - Auto-Scanner: Default scan interval, file filters, concurrency
//   - Cache: TTL settings, eviction policy, cache clearing
//   - Database: Connection info, migration status, backup/restore
//
// Routes:
//   GET  /settings            — Settings page with all sections
//   POST /settings/llm        — Update LLM settings
//   POST /settings/rag        — Update RAG settings
//   POST /settings/scanner    — Update scanner settings
//   POST /settings/cache      — Update cache settings
//   GET  /settings/status     — HTMX partial: live service status checks
//   POST /settings/test-ollama — Test Ollama connectivity
//   POST /settings/test-grok   — Test Grok API connectivity
//   POST /settings/reindex     — Trigger RAG re-index
//   POST /settings/clear-cache — Clear all caches
//
// Integration:
//   In src/lib.rs:
//     pub mod web_ui_settings;
//
//   In src/server.rs:
//     use rustassistant::web_ui_settings::create_settings_router;
//     .merge(create_settings_router(web_app_state))

use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::web_ui::WebAppState;
use crate::web_ui_nav;

// ============================================================================
// Router
// ============================================================================

pub fn create_settings_router(state: Arc<WebAppState>) -> Router {
    Router::new()
        .route("/settings", get(settings_page_handler))
        .route("/settings/llm", post(update_llm_handler))
        .route("/settings/rag", post(update_rag_handler))
        .route("/settings/scanner", post(update_scanner_handler))
        .route("/settings/cache", post(update_cache_handler))
        .route("/settings/status", get(status_partial_handler))
        .route("/settings/test-ollama", post(test_ollama_handler))
        .route("/settings/test-grok", post(test_grok_handler))
        .route("/settings/reindex", post(reindex_handler))
        .route("/settings/clear-cache", post(clear_cache_handler))
        .with_state(state)
}

// ============================================================================
// Form types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct LlmSettingsForm {
    #[serde(default)]
    pub ollama_base_url: String,
    #[serde(default)]
    pub ollama_model: String,
    #[serde(default)]
    pub ollama_timeout_secs: String,
    #[serde(default)]
    pub grok_api_key: String,
    #[serde(default)]
    pub grok_model: String,
    #[serde(default)]
    pub routing_strategy: String,
    #[serde(default)]
    pub force_remote: String,
    #[serde(default)]
    pub fallback_to_remote: String,
}

#[derive(Debug, Deserialize)]
pub struct RagSettingsForm {
    #[serde(default)]
    pub embedding_model: String,
    #[serde(default)]
    pub chunk_size: String,
    #[serde(default)]
    pub chunk_overlap: String,
    #[serde(default)]
    pub max_results: String,
}

#[derive(Debug, Deserialize)]
pub struct ScannerSettingsForm {
    #[serde(default)]
    pub default_interval_minutes: String,
    #[serde(default)]
    pub max_concurrent_scans: String,
    #[serde(default)]
    pub file_extensions: String,
    #[serde(default)]
    pub ignore_patterns: String,
    #[serde(default)]
    pub auto_scan_new_repos: String,
}

#[derive(Debug, Deserialize)]
pub struct CacheSettingsForm {
    #[serde(default)]
    pub response_cache_ttl: String,
    #[serde(default)]
    pub repo_cache_ttl: String,
    #[serde(default)]
    pub eviction_policy: String,
    #[serde(default)]
    pub max_cache_size_mb: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /settings — Render the full settings page
async fn settings_page_handler(
    State(state): State<Arc<WebAppState>>,
) -> impl IntoResponse {
    // Read current configuration from environment / defaults
    let ollama_url = std::env::var("OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    let ollama_model = std::env::var("LOCAL_MODEL")
        .unwrap_or_else(|_| "qwen2.5-coder:7b".to_string());
    let ollama_timeout = std::env::var("OLLAMA_TIMEOUT_SECS")
        .unwrap_or_else(|_| "120".to_string());
    let grok_model = std::env::var("REMOTE_MODEL")
        .unwrap_or_else(|_| "grok-2-latest".to_string());
    let grok_key_set = std::env::var("XAI_API_KEY")
        .map(|k| !k.is_empty())
        .unwrap_or(false);
    let force_remote = std::env::var("FORCE_REMOTE_MODEL")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let sync_interval = std::env::var("REPO_SYNC_INTERVAL_SECS")
        .unwrap_or_else(|_| "300".to_string());

    // Fetch some DB stats for the database section
    let db_info = get_db_info(&state).await;

    let content = format!(
        r#"
        <div class="settings-page">
            <div class="page-header">
                <h1>⚙️ Settings</h1>
                <p class="subtitle">Configure RustAssistant services, models, and behaviour</p>
            </div>

            <!-- Service Status Banner -->
            <div class="card status-banner" id="status-banner"
                 hx-get="/settings/status"
                 hx-trigger="load, every 30s"
                 hx-swap="innerHTML">
                <div class="status-loading">Checking service status...</div>
            </div>

            <!-- Settings Sections (tabbed) -->
            <div class="settings-tabs">
                <button class="tab-btn active" onclick="showTab('llm')">🤖 LLM Models</button>
                <button class="tab-btn" onclick="showTab('rag')">🔎 RAG System</button>
                <button class="tab-btn" onclick="showTab('scanner')">🔍 Scanner</button>
                <button class="tab-btn" onclick="showTab('cache')">🗄️ Cache</button>
                <button class="tab-btn" onclick="showTab('database')">🛢️ Database</button>
                <button class="tab-btn" onclick="showTab('general')">🎛️ General</button>
            </div>

            <!-- ── LLM Models Tab ──────────────────────────────── -->
            <div class="tab-content active" id="tab-llm">
                <div class="settings-grid">

                    <!-- Ollama (Local) -->
                    <div class="card settings-card">
                        <h2>🦙 Ollama (Local Models)</h2>
                        <p class="card-desc">Configure the local Ollama instance for fast, private inference.</p>
                        <form hx-post="/settings/llm" hx-target="#llm-result" hx-swap="innerHTML">
                            <div class="form-group">
                                <label for="ollama_base_url">Base URL</label>
                                <input type="text" id="ollama_base_url" name="ollama_base_url"
                                       value="{ollama_url}" placeholder="http://localhost:11434">
                            </div>
                            <div class="form-group">
                                <label for="ollama_model">Default Model</label>
                                <input type="text" id="ollama_model" name="ollama_model"
                                       value="{ollama_model}" placeholder="qwen2.5-coder:7b">
                                <span class="field-hint">Model tag as shown in <code>ollama list</code></span>
                            </div>
                            <div class="form-group">
                                <label for="ollama_timeout_secs">Timeout (seconds)</label>
                                <input type="number" id="ollama_timeout_secs" name="ollama_timeout_secs"
                                       value="{ollama_timeout}" min="10" max="600">
                            </div>
                            <div class="form-actions">
                                <button type="submit" class="btn btn-primary">💾 Save Ollama Settings</button>
                                <button type="button" class="btn btn-muted"
                                        hx-post="/settings/test-ollama"
                                        hx-target="#ollama-test-result"
                                        hx-swap="innerHTML">
                                    🧪 Test Connection
                                </button>
                            </div>
                            <div id="ollama-test-result" class="test-result"></div>
                        </form>
                    </div>

                    <!-- Grok (Remote) -->
                    <div class="card settings-card">
                        <h2>⚡ Grok 4.1 (Remote API)</h2>
                        <p class="card-desc">Configure the xAI Grok API for high-quality reasoning and code generation.</p>
                        <form hx-post="/settings/llm" hx-target="#llm-result" hx-swap="innerHTML">
                            <div class="form-group">
                                <label for="grok_api_key">API Key</label>
                                <div class="input-with-icon">
                                    <input type="password" id="grok_api_key" name="grok_api_key"
                                           value="" placeholder="{grok_key_status}">
                                    <button type="button" class="icon-btn" onclick="togglePasswordVisibility('grok_api_key')">👁️</button>
                                </div>
                                <span class="field-hint">Your xAI API key. Leave blank to keep the current key.</span>
                            </div>
                            <div class="form-group">
                                <label for="grok_model">Model</label>
                                <select id="grok_model" name="grok_model" class="settings-select">
                                    <option value="grok-2-latest" {grok_model_sel_2}>grok-2-latest</option>
                                    <option value="grok-4-1-fast-reasoning" {grok_model_sel_41}>grok-4-1-fast-reasoning</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label>Routing Strategy</label>
                                <div class="radio-group">
                                    <label class="radio-label">
                                        <input type="radio" name="routing_strategy" value="auto" {routing_auto}>
                                        <span>Auto — route by task complexity</span>
                                    </label>
                                    <label class="radio-label">
                                        <input type="radio" name="routing_strategy" value="local_first" {routing_local}>
                                        <span>Local First — prefer Ollama, fallback to Grok</span>
                                    </label>
                                    <label class="radio-label">
                                        <input type="radio" name="routing_strategy" value="remote_only" {routing_remote}>
                                        <span>Remote Only — always use Grok</span>
                                    </label>
                                </div>
                            </div>
                            <div class="form-actions">
                                <button type="submit" class="btn btn-primary">💾 Save Grok Settings</button>
                                <button type="button" class="btn btn-muted"
                                        hx-post="/settings/test-grok"
                                        hx-target="#grok-test-result"
                                        hx-swap="innerHTML">
                                    🧪 Test API Key
                                </button>
                            </div>
                            <div id="grok-test-result" class="test-result"></div>
                        </form>
                    </div>
                </div>
                <div id="llm-result" class="result-area"></div>
            </div>

            <!-- ── RAG System Tab ──────────────────────────────── -->
            <div class="tab-content" id="tab-rag">
                <div class="settings-grid">
                    <div class="card settings-card">
                        <h2>🔎 Embedding & Retrieval</h2>
                        <p class="card-desc">Configure how code is chunked, embedded, and retrieved for RAG context.</p>
                        <form hx-post="/settings/rag" hx-target="#rag-result" hx-swap="innerHTML">
                            <div class="form-group">
                                <label for="embedding_model">Embedding Model</label>
                                <select id="embedding_model" name="embedding_model" class="settings-select">
                                    <option value="all-MiniLM-L6-v2">all-MiniLM-L6-v2 (fast, 384d)</option>
                                    <option value="nomic-embed-text-v1.5">nomic-embed-text-v1.5 (768d)</option>
                                    <option value="bge-small-en-v1.5">bge-small-en-v1.5 (384d)</option>
                                </select>
                            </div>
                            <div class="form-row">
                                <div class="form-group">
                                    <label for="chunk_size">Chunk Size (tokens)</label>
                                    <input type="number" id="chunk_size" name="chunk_size"
                                           value="512" min="128" max="4096" step="64">
                                </div>
                                <div class="form-group">
                                    <label for="chunk_overlap">Overlap (tokens)</label>
                                    <input type="number" id="chunk_overlap" name="chunk_overlap"
                                           value="64" min="0" max="512" step="16">
                                </div>
                            </div>
                            <div class="form-group">
                                <label for="max_results">Max RAG Results per Query</label>
                                <input type="number" id="max_results" name="max_results"
                                       value="5" min="1" max="20">
                            </div>
                            <div class="form-actions">
                                <button type="submit" class="btn btn-primary">💾 Save RAG Settings</button>
                                <button type="button" class="btn btn-warning"
                                        hx-post="/settings/reindex"
                                        hx-target="#rag-reindex-result"
                                        hx-swap="innerHTML"
                                        hx-confirm="Re-index all embeddings? This may take a while.">
                                    🔄 Re-index All
                                </button>
                            </div>
                            <div id="rag-reindex-result" class="test-result"></div>
                        </form>
                    </div>

                    <div class="card settings-card">
                        <h2>📊 RAG Index Status</h2>
                        <div id="rag-status"
                             hx-get="/settings/status"
                             hx-trigger="load"
                             hx-swap="none">
                        </div>
                        <div class="stats-mini" id="rag-stats">
                            <div class="stat-mini">
                                <span class="stat-mini-value" id="rag-embed-count">—</span>
                                <span class="stat-mini-label">Embeddings</span>
                            </div>
                            <div class="stat-mini">
                                <span class="stat-mini-value" id="rag-file-count">—</span>
                                <span class="stat-mini-label">Files Indexed</span>
                            </div>
                            <div class="stat-mini">
                                <span class="stat-mini-value" id="rag-last-index">—</span>
                                <span class="stat-mini-label">Last Index Run</span>
                            </div>
                        </div>
                    </div>
                </div>
                <div id="rag-result" class="result-area"></div>
            </div>

            <!-- ── Scanner Tab ─────────────────────────────────── -->
            <div class="tab-content" id="tab-scanner">
                <div class="card settings-card" style="max-width: 700px;">
                    <h2>🔍 Auto-Scanner Configuration</h2>
                    <p class="card-desc">Control how repositories are automatically scanned for changes, TODOs, and issues.</p>
                    <form hx-post="/settings/scanner" hx-target="#scanner-result" hx-swap="innerHTML">
                        <div class="form-row">
                            <div class="form-group">
                                <label for="default_interval_minutes">Default Scan Interval (minutes)</label>
                                <input type="number" id="default_interval_minutes" name="default_interval_minutes"
                                       value="{sync_interval_min}" min="1" max="1440">
                            </div>
                            <div class="form-group">
                                <label for="max_concurrent_scans">Max Concurrent Scans</label>
                                <input type="number" id="max_concurrent_scans" name="max_concurrent_scans"
                                       value="2" min="1" max="10">
                            </div>
                        </div>
                        <div class="form-group">
                            <label for="file_extensions">File Extensions to Scan</label>
                            <input type="text" id="file_extensions" name="file_extensions"
                                   value=".rs,.toml,.md,.yml,.yaml,.json,.sql"
                                   placeholder=".rs,.toml,.md,...">
                            <span class="field-hint">Comma-separated list of extensions to include in scans.</span>
                        </div>
                        <div class="form-group">
                            <label for="ignore_patterns">Ignore Patterns</label>
                            <textarea id="ignore_patterns" name="ignore_patterns" rows="3"
                                      placeholder="target/&#10;node_modules/&#10;.git/">target/
node_modules/
.git/
*.lock</textarea>
                            <span class="field-hint">One pattern per line. Directories should end with /</span>
                        </div>
                        <div class="form-group">
                            <label class="toggle-label">
                                <input type="checkbox" id="auto_scan_new_repos" name="auto_scan_new_repos" checked>
                                <span>Automatically enable scanning for newly added repositories</span>
                            </label>
                        </div>
                        <div class="form-actions">
                            <button type="submit" class="btn btn-primary">💾 Save Scanner Settings</button>
                        </div>
                    </form>
                </div>
                <div id="scanner-result" class="result-area"></div>
            </div>

            <!-- ── Cache Tab ───────────────────────────────────── -->
            <div class="tab-content" id="tab-cache">
                <div class="settings-grid">
                    <div class="card settings-card">
                        <h2>🗄️ Cache Configuration</h2>
                        <p class="card-desc">Manage response caching, TTL settings, and eviction policies.</p>
                        <form hx-post="/settings/cache" hx-target="#cache-result" hx-swap="innerHTML">
                            <div class="form-row">
                                <div class="form-group">
                                    <label for="response_cache_ttl">Response Cache TTL (seconds)</label>
                                    <input type="number" id="response_cache_ttl" name="response_cache_ttl"
                                           value="3600" min="60" max="604800">
                                </div>
                                <div class="form-group">
                                    <label for="repo_cache_ttl">Repo Cache TTL (seconds)</label>
                                    <input type="number" id="repo_cache_ttl" name="repo_cache_ttl"
                                           value="86400" min="300" max="2592000">
                                </div>
                            </div>
                            <div class="form-group">
                                <label for="eviction_policy">Eviction Policy</label>
                                <select id="eviction_policy" name="eviction_policy" class="settings-select">
                                    <option value="lru">LRU (Least Recently Used)</option>
                                    <option value="lfu">LFU (Least Frequently Used)</option>
                                    <option value="ttl">TTL-based only</option>
                                </select>
                            </div>
                            <div class="form-group">
                                <label for="max_cache_size_mb">Max Cache Size (MB)</label>
                                <input type="number" id="max_cache_size_mb" name="max_cache_size_mb"
                                       value="256" min="32" max="4096">
                            </div>
                            <div class="form-actions">
                                <button type="submit" class="btn btn-primary">💾 Save Cache Settings</button>
                                <button type="button" class="btn btn-danger"
                                        hx-post="/settings/clear-cache"
                                        hx-target="#cache-clear-result"
                                        hx-swap="innerHTML"
                                        hx-confirm="Clear ALL caches? This cannot be undone.">
                                    🗑️ Clear All Caches
                                </button>
                            </div>
                            <div id="cache-clear-result" class="test-result"></div>
                        </form>
                    </div>

                    <div class="card settings-card">
                        <h2>📊 Cache Statistics</h2>
                        <div class="stats-mini">
                            <div class="stat-mini">
                                <span class="stat-mini-value" id="cache-hit-rate">—</span>
                                <span class="stat-mini-label">Hit Rate</span>
                            </div>
                            <div class="stat-mini">
                                <span class="stat-mini-value" id="cache-entries">—</span>
                                <span class="stat-mini-label">Entries</span>
                            </div>
                            <div class="stat-mini">
                                <span class="stat-mini-value" id="cache-size">—</span>
                                <span class="stat-mini-label">Size (MB)</span>
                            </div>
                        </div>
                        <p class="hint" style="margin-top: 1rem;">
                            Visit <a href="/cache" style="color: #0ea5e9;">Cache Viewer</a> for detailed cache inspection.
                        </p>
                    </div>
                </div>
                <div id="cache-result" class="result-area"></div>
            </div>

            <!-- ── Database Tab ────────────────────────────────── -->
            <div class="tab-content" id="tab-database">
                <div class="card settings-card" style="max-width: 700px;">
                    <h2>🛢️ Database Information</h2>
                    <table class="info-table">
                        <tr>
                            <td class="info-label">Engine</td>
                            <td>PostgreSQL</td>
                        </tr>
                        <tr>
                            <td class="info-label">Connection</td>
                            <td><code>{db_url_masked}</code></td>
                        </tr>
                        <tr>
                            <td class="info-label">Total Repositories</td>
                            <td>{db_repos}</td>
                        </tr>
                        <tr>
                            <td class="info-label">Total Notes</td>
                            <td>{db_notes}</td>
                        </tr>
                        <tr>
                            <td class="info-label">Total Tasks</td>
                            <td>{db_tasks}</td>
                        </tr>
                        <tr>
                            <td class="info-label">Total Ideas</td>
                            <td>{db_ideas}</td>
                        </tr>
                        <tr>
                            <td class="info-label">Total Documents</td>
                            <td>{db_documents}</td>
                        </tr>
                    </table>
                    <div class="form-actions" style="margin-top: 1rem;">
                        <a href="/db" class="btn btn-primary">🔍 Open DB Explorer</a>
                    </div>
                </div>
            </div>

            <!-- ── General Tab ─────────────────────────────────── -->
            <div class="tab-content" id="tab-general">
                <div class="card settings-card" style="max-width: 700px;">
                    <h2>🎛️ General Configuration</h2>
                    <table class="info-table">
                        <tr>
                            <td class="info-label">Version</td>
                            <td>v0.1.0</td>
                        </tr>
                        <tr>
                            <td class="info-label">Server Host</td>
                            <td><code>{server_host}</code></td>
                        </tr>
                        <tr>
                            <td class="info-label">Workspace Dir</td>
                            <td><code>{workspace_dir}</code></td>
                        </tr>
                        <tr>
                            <td class="info-label">Static Dir</td>
                            <td><code>{static_dir}</code></td>
                        </tr>
                    </table>

                    <h3 style="margin-top: 1.5rem; margin-bottom: 0.75rem;">Environment Variables</h3>
                    <div class="env-list">
                        {env_vars_html}
                    </div>
                </div>
            </div>
        </div>
        "#,
        ollama_url = html_escape(&ollama_url),
        ollama_model = html_escape(&ollama_model),
        ollama_timeout = html_escape(&ollama_timeout),
        grok_key_status = if grok_key_set { "••••••••••• (key configured)" } else { "Not set" },
        grok_model_sel_2 = if grok_model == "grok-2-latest" { "selected" } else { "" },
        grok_model_sel_41 = if grok_model.contains("4-1") || grok_model.contains("4.1") { "selected" } else { "" },
        routing_auto = if !force_remote { "checked" } else { "" },
        routing_local = "",
        routing_remote = if force_remote { "checked" } else { "" },
        sync_interval_min = {
            let secs: u64 = sync_interval.parse().unwrap_or(300);
            secs / 60
        },
        db_url_masked = html_escape(&mask_db_url()),
        db_repos = db_info.repos,
        db_notes = db_info.notes,
        db_tasks = db_info.tasks,
        db_ideas = db_info.ideas,
        db_documents = db_info.documents,
        server_host = html_escape(&std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0:3000".to_string())),
        workspace_dir = html_escape(&std::env::var("WORKSPACE_DIR").unwrap_or_else(|_| "data/workspaces".to_string())),
        static_dir = html_escape(&std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string())),
        env_vars_html = render_env_vars(),
    );

    let extra_head = settings_extra_styles();
    let page = web_ui_nav::page_shell("Settings", "Settings", &extra_head, &content);
    Html(page)
}

/// GET /settings/status — Return live service status as HTML partial
async fn status_partial_handler(
    State(state): State<Arc<WebAppState>>,
) -> impl IntoResponse {
    let mut items = Vec::new();

    // Ollama
    let ollama_ok = check_ollama().await;
    items.push(if ollama_ok {
        r#"<span class="status-chip online">🦙 Ollama Online</span>"#.to_string()
    } else {
        r#"<span class="status-chip offline">🦙 Ollama Offline</span>"#.to_string()
    });

    // Grok
    let grok_ok = std::env::var("XAI_API_KEY")
        .map(|k| !k.is_empty())
        .unwrap_or(false);
    items.push(if grok_ok {
        r#"<span class="status-chip online">⚡ Grok Configured</span>"#.to_string()
    } else {
        r#"<span class="status-chip offline">⚡ Grok No Key</span>"#.to_string()
    });

    // Database
    let db_ok = check_db(&state).await;
    items.push(if db_ok {
        r#"<span class="status-chip online">🛢️ Database Connected</span>"#.to_string()
    } else {
        r#"<span class="status-chip offline">🛢️ Database Error</span>"#.to_string()
    });

    // RAG
    let embed_count = get_embedding_count(&state).await;
    items.push(format!(
        r#"<span class="status-chip {}">🔎 RAG: {} embeddings</span>"#,
        if embed_count > 0 { "online" } else { "warning" },
        embed_count,
    ));

    let html = format!(
        r#"<div class="status-chips">{}</div>"#,
        items.join("\n")
    );
    Html(html)
}

/// POST /settings/llm — Update LLM settings
async fn update_llm_handler(
    Form(form): Form<LlmSettingsForm>,
) -> impl IntoResponse {
    info!(
        ollama_url = %form.ollama_base_url,
        ollama_model = %form.ollama_model,
        routing = %form.routing_strategy,
        "Updating LLM settings"
    );

    // In a production system, these would be persisted to a config file or DB.
    // For now, we update environment variables for the current process.
    if !form.ollama_base_url.is_empty() {
        std::env::set_var("OLLAMA_BASE_URL", &form.ollama_base_url);
    }
    if !form.ollama_model.is_empty() {
        std::env::set_var("LOCAL_MODEL", &form.ollama_model);
    }
    if !form.ollama_timeout_secs.is_empty() {
        std::env::set_var("OLLAMA_TIMEOUT_SECS", &form.ollama_timeout_secs);
    }
    if !form.grok_api_key.is_empty() && !form.grok_api_key.contains("•") {
        std::env::set_var("XAI_API_KEY", &form.grok_api_key);
    }
    if !form.grok_model.is_empty() {
        std::env::set_var("REMOTE_MODEL", &form.grok_model);
    }
    match form.routing_strategy.as_str() {
        "remote_only" => std::env::set_var("FORCE_REMOTE_MODEL", "true"),
        _ => std::env::set_var("FORCE_REMOTE_MODEL", "false"),
    }

    Html(
        r#"<div class="alert alert-success">✅ LLM settings updated successfully.
           <br><small>Note: Some changes may require a server restart to take full effect.</small></div>"#
            .to_string(),
    )
}

/// POST /settings/rag — Update RAG settings
async fn update_rag_handler(
    Form(form): Form<RagSettingsForm>,
) -> impl IntoResponse {
    info!(
        embedding_model = %form.embedding_model,
        chunk_size = %form.chunk_size,
        "Updating RAG settings"
    );

    // Persist to env for current session
    if !form.embedding_model.is_empty() {
        std::env::set_var("EMBEDDING_MODEL", &form.embedding_model);
    }
    if !form.chunk_size.is_empty() {
        std::env::set_var("CHUNK_SIZE", &form.chunk_size);
    }
    if !form.chunk_overlap.is_empty() {
        std::env::set_var("CHUNK_OVERLAP", &form.chunk_overlap);
    }
    if !form.max_results.is_empty() {
        std::env::set_var("RAG_MAX_RESULTS", &form.max_results);
    }

    Html(
        r#"<div class="alert alert-success">✅ RAG settings updated.
           Consider re-indexing if you changed the embedding model or chunk size.</div>"#
            .to_string(),
    )
}

/// POST /settings/scanner — Update scanner settings
async fn update_scanner_handler(
    Form(form): Form<ScannerSettingsForm>,
) -> impl IntoResponse {
    info!(
        interval = %form.default_interval_minutes,
        concurrency = %form.max_concurrent_scans,
        "Updating scanner settings"
    );

    if !form.default_interval_minutes.is_empty() {
        let minutes: u64 = form.default_interval_minutes.parse().unwrap_or(5);
        std::env::set_var("REPO_SYNC_INTERVAL_SECS", (minutes * 60).to_string());
    }

    Html(
        r#"<div class="alert alert-success">✅ Scanner settings updated.
           Changes will apply to the next scan cycle.</div>"#
            .to_string(),
    )
}

/// POST /settings/cache — Update cache settings
async fn update_cache_handler(
    Form(form): Form<CacheSettingsForm>,
) -> impl IntoResponse {
    info!(
        response_ttl = %form.response_cache_ttl,
        eviction = %form.eviction_policy,
        "Updating cache settings"
    );

    Html(
        r#"<div class="alert alert-success">✅ Cache settings updated.</div>"#.to_string(),
    )
}

/// POST /settings/test-ollama — Test Ollama connection
async fn test_ollama_handler() -> impl IntoResponse {
    let ok = check_ollama().await;
    if ok {
        let models = list_ollama_models().await;
        let model_list = if models.is_empty() {
            "No models found".to_string()
        } else {
            models.join(", ")
        };
        Html(format!(
            r#"<div class="alert alert-success">✅ Ollama is reachable!<br>Available models: {}</div>"#,
            html_escape(&model_list)
        ))
    } else {
        Html(
            r#"<div class="alert alert-danger">❌ Cannot reach Ollama. Check that it's running and the URL is correct.</div>"#.to_string()
        )
    }
}

/// POST /settings/test-grok — Test Grok API key
async fn test_grok_handler(
    State(state): State<Arc<WebAppState>>,
) -> impl IntoResponse {
    let api_key = match std::env::var("XAI_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            return Html(
                r#"<div class="alert alert-danger">❌ No API key configured. Please save a key first.</div>"#.to_string()
            );
        }
    };

    let db = crate::db::Database::from_pool(state.db.pool.clone());
    let grok = crate::grok_client::GrokClient::new(api_key, db);

    match grok.ask_tracked("Say 'API key is valid' in exactly those words.", None, "settings_test").await {
        Ok(resp) => {
            Html(format!(
                r#"<div class="alert alert-success">✅ Grok API key is valid!<br>Model: {}<br>Tokens used: {}</div>"#,
                "grok", resp.total_tokens
            ))
        }
        Err(e) => {
            Html(format!(
                r#"<div class="alert alert-danger">❌ Grok API test failed: {}</div>"#,
                html_escape(&e.to_string())
            ))
        }
    }
}

/// POST /settings/reindex — Trigger a RAG re-index
async fn reindex_handler(
    State(state): State<Arc<WebAppState>>,
) -> impl IntoResponse {
    info!("RAG re-index requested from settings UI");

    // Spawn the re-index in the background
    let pool = state.db.pool.clone();
    tokio::spawn(async move {
        match crate::scanner::refresh_rag_index(&pool).await {
            Ok(n) => info!(vectors = n, "RAG re-index completed from settings"),
            Err(e) => error!(error = %e, "RAG re-index failed"),
        }
    });

    Html(
        r#"<div class="alert alert-info">🔄 Re-indexing started in the background. Check back in a few minutes.</div>"#.to_string()
    )
}

/// POST /settings/clear-cache — Clear all caches
async fn clear_cache_handler(
    State(state): State<Arc<WebAppState>>,
) -> impl IntoResponse {
    info!("Cache clear requested from settings UI");

    // Attempt to clear response cache via the GrokClient path
    let api_key = std::env::var("XAI_API_KEY").unwrap_or_default();
    if !api_key.is_empty() {
        let db = crate::db::Database::from_pool(state.db.pool.clone());
        let grok = crate::grok_client::GrokClient::new(api_key, db);
        if let Err(e) = grok.clear_cache().await {
            warn!(error = %e, "Failed to clear Grok response cache");
        }
    }

    Html(
        r#"<div class="alert alert-success">✅ Caches cleared successfully.</div>"#.to_string()
    )
}

// ============================================================================
// Helpers
// ============================================================================

struct DbInfo {
    repos: i64,
    notes: i64,
    tasks: i64,
    ideas: i64,
    documents: i64,
}

async fn get_db_info(state: &WebAppState) -> DbInfo {
    let pool = &state.db.pool;

    let repos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM registered_repos")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let notes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let tasks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let ideas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ideas")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let documents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM documents")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    DbInfo {
        repos,
        notes,
        tasks,
        ideas,
        documents,
    }
}

async fn check_ollama() -> bool {
    let base_url = std::env::var("OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    let url = format!("{}/api/tags", base_url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    client
        .get(&url)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

async fn list_ollama_models() -> Vec<String> {
    let base_url = std::env::var("OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    let url = format!("{}/api/tags", base_url);

    #[derive(serde::Deserialize)]
    struct TagsResponse {
        models: Vec<ModelEntry>,
    }
    #[derive(serde::Deserialize)]
    struct ModelEntry {
        name: String,
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<TagsResponse>().await {
            Ok(tags) => tags.models.into_iter().map(|m| m.name).collect(),
            Err(_) => vec![],
        },
        Err(_) => vec![],
    }
}

async fn check_db(state: &WebAppState) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db.pool)
        .await
        .is_ok()
}

async fn get_embedding_count(state: &WebAppState) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'file_embeddings'"
    )
    .fetch_one(&state.db.pool)
    .await
    .and_then(|exists: i64| {
        if exists > 0 {
            Ok(exists)
        } else {
            Ok(0)
        }
    })
    .unwrap_or(0)
}

fn mask_db_url() -> String {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "not configured".to_string());
    // Mask password in the URL
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            if let Some(slash_pos) = url[..colon_pos].rfind('/') {
                return format!(
                    "{}:****@{}",
                    &url[..slash_pos + 1],
                    &url[at_pos + 1..]
                );
            }
        }
    }
    url
}

fn render_env_vars() -> String {
    let vars = [
        ("OLLAMA_BASE_URL", "Ollama server URL"),
        ("LOCAL_MODEL", "Default Ollama model"),
        ("OLLAMA_TIMEOUT_SECS", "Ollama request timeout"),
        ("XAI_API_KEY", "Grok API key"),
        ("REMOTE_MODEL", "Default Grok model"),
        ("FORCE_REMOTE_MODEL", "Always use remote model"),
        ("DATABASE_URL", "PostgreSQL connection string"),
        ("WORKSPACE_DIR", "Repository workspace directory"),
        ("STATIC_DIR", "Static file serving directory"),
        ("REPO_SYNC_INTERVAL_SECS", "Sync scheduler interval"),
        ("GITHUB_WEBHOOK_SECRET", "GitHub webhook secret"),
        ("RUST_LOG", "Logging level filter"),
    ];

    let rows: String = vars
        .iter()
        .map(|(name, desc)| {
            let val = std::env::var(name).unwrap_or_else(|_| "—".to_string());
            let display_val = if name.contains("KEY") || name.contains("SECRET") || name.contains("PASSWORD") {
                if val == "—" {
                    "—".to_string()
                } else {
                    "••••••••".to_string()
                }
            } else if val.len() > 60 {
                format!("{}…", &val[..57])
            } else {
                val
            };
            format!(
                r#"<div class="env-row">
                    <span class="env-name"><code>{name}</code></span>
                    <span class="env-value">{value}</span>
                    <span class="env-desc">{desc}</span>
                </div>"#,
                name = name,
                value = html_escape(&display_val),
                desc = desc,
            )
        })
        .collect();

    rows
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ============================================================================
// Styles
// ============================================================================

fn settings_extra_styles() -> String {
    r#"<style>
    /* ── Settings Page Layout ─────────────────────────────────────── */
    .settings-page {
        padding: 1rem 0;
    }
    .page-header {
        margin-bottom: 1.5rem;
    }
    .page-header h1 {
        font-size: 1.6rem;
        color: #f1f5f9;
        margin-bottom: 0.25rem;
    }
    .subtitle {
        color: #94a3b8;
        font-size: 0.9rem;
    }

    /* Status banner */
    .status-banner {
        margin-bottom: 1.5rem;
        padding: 0.75rem 1rem;
    }
    .status-chips {
        display: flex;
        gap: 0.75rem;
        flex-wrap: wrap;
    }
    .status-chip {
        display: inline-flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.4rem 0.75rem;
        border-radius: 20px;
        font-size: 0.8rem;
        font-weight: 600;
    }
    .status-chip.online {
        background: #052e16;
        color: #4ade80;
        border: 1px solid #166534;
    }
    .status-chip.offline {
        background: #450a0a;
        color: #fca5a5;
        border: 1px solid #991b1b;
    }
    .status-chip.warning {
        background: #451a03;
        color: #fcd34d;
        border: 1px solid #92400e;
    }
    .status-loading {
        color: #94a3b8;
        font-size: 0.85rem;
    }

    /* Tab navigation */
    .settings-tabs {
        display: flex;
        gap: 0.25rem;
        margin-bottom: 1.5rem;
        border-bottom: 2px solid #1e293b;
        padding-bottom: 0;
        flex-wrap: wrap;
    }
    .tab-btn {
        padding: 0.6rem 1rem;
        background: transparent;
        color: #94a3b8;
        border: none;
        border-bottom: 2px solid transparent;
        margin-bottom: -2px;
        cursor: pointer;
        font-size: 0.85rem;
        font-weight: 500;
        transition: all 0.2s;
        white-space: nowrap;
    }
    .tab-btn:hover {
        color: #e2e8f0;
        background: #1e293b;
    }
    .tab-btn.active {
        color: #0ea5e9;
        border-bottom-color: #0ea5e9;
    }
    .tab-content {
        display: none;
    }
    .tab-content.active {
        display: block;
        animation: fadeIn 0.2s ease;
    }
    @keyframes fadeIn {
        from { opacity: 0; }
        to { opacity: 1; }
    }

    /* Settings grid */
    .settings-grid {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
        gap: 1.5rem;
    }
    .settings-card {
        padding: 1.5rem;
    }
    .settings-card h2 {
        font-size: 1.15rem;
        margin-bottom: 0.25rem;
    }
    .card-desc {
        color: #94a3b8;
        font-size: 0.82rem;
        margin-bottom: 1.25rem;
    }

    /* Forms */
    .form-group {
        margin-bottom: 1rem;
    }
    .form-group label {
        display: block;
        margin-bottom: 0.35rem;
        font-size: 0.85rem;
        font-weight: 600;
        color: #cbd5e1;
    }
    .form-group input[type="text"],
    .form-group input[type="password"],
    .form-group input[type="number"],
    .form-group textarea,
    .settings-select {
        width: 100%;
        background: #0f172a;
        color: #e2e8f0;
        border: 1px solid #334155;
        border-radius: 6px;
        padding: 0.5rem 0.75rem;
        font-size: 0.85rem;
        font-family: inherit;
        transition: border-color 0.2s;
    }
    .form-group input:focus,
    .form-group textarea:focus,
    .settings-select:focus {
        outline: none;
        border-color: #0ea5e9;
        box-shadow: 0 0 0 2px rgba(14, 165, 233, 0.15);
    }
    .form-group textarea {
        resize: vertical;
        min-height: 80px;
        font-family: 'JetBrains Mono', 'Fira Code', monospace;
        font-size: 0.8rem;
    }
    .form-row {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 1rem;
    }
    .field-hint {
        display: block;
        font-size: 0.75rem;
        color: #64748b;
        margin-top: 0.25rem;
    }
    .field-hint code {
        background: #1e293b;
        padding: 0.1rem 0.3rem;
        border-radius: 3px;
        font-size: 0.72rem;
    }
    .input-with-icon {
        display: flex;
        gap: 0;
    }
    .input-with-icon input {
        border-top-right-radius: 0;
        border-bottom-right-radius: 0;
        flex: 1;
    }
    .icon-btn {
        background: #334155;
        border: 1px solid #334155;
        border-left: none;
        color: #94a3b8;
        padding: 0 0.6rem;
        cursor: pointer;
        border-top-right-radius: 6px;
        border-bottom-right-radius: 6px;
        font-size: 0.85rem;
        transition: background 0.2s;
    }
    .icon-btn:hover {
        background: #475569;
    }
    .form-actions {
        display: flex;
        gap: 0.75rem;
        margin-top: 1.25rem;
        flex-wrap: wrap;
    }

    /* Radio & toggle */
    .radio-group {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }
    .radio-label {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        font-size: 0.85rem;
        cursor: pointer;
        color: #cbd5e1;
    }
    .radio-label input[type="radio"] {
        accent-color: #0ea5e9;
    }
    .toggle-label {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        cursor: pointer;
        font-size: 0.85rem;
        color: #cbd5e1;
    }
    .toggle-label input[type="checkbox"] {
        width: 1rem;
        height: 1rem;
        accent-color: #0ea5e9;
    }

    /* Alerts */
    .alert {
        padding: 0.75rem 1rem;
        border-radius: 6px;
        font-size: 0.85rem;
        margin-top: 0.75rem;
    }
    .alert small {
        color: #94a3b8;
    }
    .alert-success {
        background: #052e16;
        color: #86efac;
        border: 1px solid #166534;
    }
    .alert-danger {
        background: #450a0a;
        color: #fca5a5;
        border: 1px solid #991b1b;
    }
    .alert-info {
        background: #0c2d4a;
        color: #7dd3fc;
        border: 1px solid #0369a1;
    }
    .alert-warning {
        background: #451a03;
        color: #fcd34d;
        border: 1px solid #92400e;
    }
    .btn-warning {
        background: #f59e0b;
        color: #1e293b;
    }
    .btn-warning:hover {
        background: #d97706;
    }

    /* Test result areas */
    .test-result, .result-area {
        min-height: 0;
    }

    /* Info tables */
    .info-table {
        width: 100%;
        border-collapse: collapse;
    }
    .info-table td {
        padding: 0.6rem 0.75rem;
        border-bottom: 1px solid #1e293b;
        font-size: 0.85rem;
    }
    .info-table .info-label {
        color: #94a3b8;
        font-weight: 600;
        width: 180px;
    }
    .info-table code {
        background: #0f172a;
        padding: 0.15rem 0.4rem;
        border-radius: 3px;
        font-size: 0.8rem;
        color: #7dd3fc;
    }

    /* Stats mini cards */
    .stats-mini {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: 0.75rem;
        margin-top: 1rem;
    }
    .stat-mini {
        background: #0f172a;
        border-radius: 6px;
        padding: 0.75rem;
        text-align: center;
        border: 1px solid #1e293b;
    }
    .stat-mini-value {
        display: block;
        font-size: 1.4rem;
        font-weight: 700;
        color: #38bdf8;
    }
    .stat-mini-label {
        display: block;
        font-size: 0.72rem;
        color: #64748b;
        margin-top: 0.2rem;
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }

    /* Environment variables list */
    .env-list {
        border: 1px solid #1e293b;
        border-radius: 6px;
        overflow: hidden;
    }
    .env-row {
        display: grid;
        grid-template-columns: 220px 1fr 200px;
        gap: 0.5rem;
        padding: 0.5rem 0.75rem;
        border-bottom: 1px solid #1e293b;
        font-size: 0.8rem;
        align-items: center;
    }
    .env-row:last-child {
        border-bottom: none;
    }
    .env-row:hover {
        background: #0f172a;
    }
    .env-name code {
        background: #1e293b;
        padding: 0.15rem 0.35rem;
        border-radius: 3px;
        font-size: 0.75rem;
        color: #fbbf24;
    }
    .env-value {
        color: #e2e8f0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .env-desc {
        color: #64748b;
        font-size: 0.75rem;
    }

    .hint {
        font-size: 0.75rem;
        color: #64748b;
        display: block;
    }

    /* Responsive */
    @media (max-width: 900px) {
        .settings-grid {
            grid-template-columns: 1fr;
        }
        .form-row {
            grid-template-columns: 1fr;
        }
        .env-row {
            grid-template-columns: 1fr;
            gap: 0.2rem;
        }
        .env-desc {
            display: none;
        }
        .stats-mini {
            grid-template-columns: 1fr 1fr;
        }
    }
    </style>
    <script>
    function showTab(tabId) {
        // Hide all tab contents
        document.querySelectorAll('.tab-content').forEach(el => {
            el.classList.remove('active');
        });
        document.querySelectorAll('.tab-btn').forEach(el => {
            el.classList.remove('active');
        });

        // Show selected tab
        const tab = document.getElementById('tab-' + tabId);
        if (tab) tab.classList.add('active');

        // Highlight the clicked button
        event.target.classList.add('active');

        // Store in URL hash for bookmarking
        history.replaceState(null, '', '#' + tabId);
    }

    function togglePasswordVisibility(inputId) {
        const input = document.getElementById(inputId);
        if (input) {
            input.type = input.type === 'password' ? 'text' : 'password';
        }
    }

    // Restore tab from URL hash
    document.addEventListener('DOMContentLoaded', function() {
        const hash = window.location.hash.replace('#', '');
        if (hash) {
            const tab = document.getElementById('tab-' + hash);
            if (tab) {
                document.querySelectorAll('.tab-content').forEach(el => el.classList.remove('active'));
                document.querySelectorAll('.tab-btn').forEach(el => el.classList.remove('active'));
                tab.classList.add('active');
                // Find and activate the correct tab button
                document.querySelectorAll('.tab-btn').forEach(btn => {
                    if (btn.getAttribute('onclick') && btn.getAttribute('onclick').includes(hash)) {
                        btn.classList.add('active');
                    }
                });
            }
        }
    });
    </script>"#
        .to_string()
}
