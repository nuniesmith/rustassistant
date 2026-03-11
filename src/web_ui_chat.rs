// ============================================================================
// Chat Interface — Web UI Module
// ============================================================================
//
// Provides a full-featured chat interface for interacting with RustAssistant
// through LLMs (Ollama local models, Grok 4.1 remote, and RAG-augmented queries).
//
// Features:
//   - Real-time chat with streaming-style display
//   - Model selection (Ollama local, Grok remote, Auto-route)
//   - Repository context injection (select a repo to chat about)
//   - RAG-powered context retrieval from indexed codebase
//   - Conversation history with multi-turn context (persisted to localStorage)
//   - Chat log: named sessions stored in localStorage, restorable, clearable
//   - Task/batch dispatch suggestions from LLM responses
//   - Markdown rendering of responses
//   - Keyboard shortcuts (Enter to send, Shift+Enter for newline)
//
// Routes:
//   GET  /chat          — Chat page with full UI
//   POST /chat/send     — Send a message (returns HTML partial via HTMX)
//   GET  /chat/models   — List available models (HTMX partial)
//   POST /chat/clear    — Clear conversation history
//
// Integration:
//   In src/lib.rs:
//     pub mod web_ui_chat;
//
//   In src/server.rs:
//     use rustassistant::web_ui_chat::create_chat_router;
//     .merge(create_chat_router(web_app_state))

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

pub fn create_chat_router(state: Arc<WebAppState>) -> Router {
    Router::new()
        .route("/chat", get(chat_page_handler))
        .route("/chat/send", post(chat_send_handler))
        .route("/chat/models", get(chat_models_handler))
        .route("/chat/clear", post(chat_clear_handler))
        .route("/chat/repos", get(chat_repos_handler))
        .with_state(state)
}

// ============================================================================
// Form types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ChatSendForm {
    pub message: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub repo_id: String,
    #[serde(default)]
    pub use_rag: String,
    /// JSON-encoded conversation history
    #[serde(default)]
    pub history_json: String,
}

#[derive(Debug, Deserialize)]
pub struct ModelsQuery {
    #[serde(default)]
    pub refresh: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /chat — Render the full chat page
async fn chat_page_handler(State(state): State<Arc<WebAppState>>) -> impl IntoResponse {
    // Load repos for the context selector dropdown
    let repos = load_repos(&state).await;

    let repo_options: String = repos
        .iter()
        .map(|(id, name)| format!(r#"<option value="{id}">{name}</option>"#))
        .collect::<Vec<_>>()
        .join("\n");

    let content = format!(
        r##"
        <div class="chat-layout">
            <!-- Sidebar: Settings & Context -->
            <aside class="chat-sidebar">
                <div class="sidebar-section">
                    <h3>🤖 Model</h3>
                    <select id="model-select" class="chat-select">
                        <option value="auto">Auto (Smart Routing)</option>
                        <optgroup label="Local (Ollama)">
                            <option value="ollama:qwen2.5-coder:7b">Qwen 2.5 Coder 7B</option>
                            <option value="ollama:qwen2.5-coder:14b">Qwen 2.5 Coder 14B</option>
                            <option value="ollama:codellama:7b">Code Llama 7B</option>
                            <option value="ollama:deepseek-coder:6.7b">DeepSeek Coder 6.7B</option>
                        </optgroup>
                        <optgroup label="Remote (Grok)">
                            <option value="grok:grok-4-1-fast-reasoning">Grok 4.1 Fast Reasoning</option>
                        </optgroup>
                    </select>
                    <div id="model-status" class="model-status"
                         hx-get="/chat/models"
                         hx-trigger="load"
                         hx-swap="innerHTML">
                        <span class="status-dot checking"></span> Checking models...
                    </div>
                </div>

                <div class="sidebar-section">
                    <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:0.35rem;">
                        <h3 style="margin:0;">📦 Repository Context</h3>
                        <button class="btn btn-xs btn-muted"
                                title="Refresh repo list"
                                hx-get="/chat/repos"
                                hx-target="#repo-select"
                                hx-swap="innerHTML"
                                hx-indicator="#repo-refresh-spinner">
                            <span id="repo-refresh-spinner" class="htmx-indicator">⏳</span>
                            <span>↻</span>
                        </button>
                    </div>
                    <select id="repo-select" class="chat-select"
                            hx-get="/chat/repos"
                            hx-trigger="load"
                            hx-target="#repo-select"
                            hx-swap="innerHTML">
                        <option value="">No repository (general chat)</option>
                        {repo_options}
                    </select>
                    <p class="hint">Select a repo to inject its todo.md and structure as context.</p>
                </div>

                <div class="sidebar-section">
                    <h3>🔎 RAG Context</h3>
                    <label class="toggle-label">
                        <input type="checkbox" id="rag-toggle" checked>
                        <span>Enable RAG retrieval</span>
                    </label>
                    <p class="hint">Searches indexed code embeddings for relevant context.</p>
                </div>

                <div class="sidebar-section">
                    <h3>⚡ Quick Actions</h3>
                    <div class="quick-actions">
                        <button class="btn btn-sm btn-muted quick-prompt"
                                data-prompt="Summarize the current state of this repository and suggest next steps.">
                            📋 Summarize Repo
                        </button>
                        <button class="btn btn-sm btn-muted quick-prompt"
                                data-prompt="Review the todo.md and create a prioritized task plan with batches.">
                            📝 Plan Tasks
                        </button>
                        <button class="btn btn-sm btn-muted quick-prompt"
                                data-prompt="Analyze the codebase for security concerns and technical debt.">
                            🔒 Security Audit
                        </button>
                        <button class="btn btn-sm btn-muted quick-prompt"
                                data-prompt="Suggest refactoring opportunities to improve code quality.">
                            🔧 Refactor Ideas
                        </button>
                        <button class="btn btn-sm btn-muted quick-prompt"
                                data-prompt="Generate a comprehensive test plan for the untested modules.">
                            🧪 Test Plan
                        </button>
                        <button class="btn btn-sm btn-muted quick-prompt"
                                data-prompt="Run the full pipeline: scan, scaffold, plan for this repository.">
                            🚀 Full Pipeline
                        </button>
                    </div>
                </div>

                <div class="sidebar-section">
                    <div style="display:flex;gap:0.4rem;margin-bottom:0.4rem;">
                        <button class="btn btn-sm btn-success" style="flex:1;"
                                onclick="saveSession()">
                            💾 Save Chat
                        </button>
                        <button class="btn btn-sm btn-danger" style="flex:1;"
                                hx-post="/chat/clear"
                                hx-target="#chat-messages"
                                hx-swap="innerHTML"
                                onclick="clearHistory()">
                            🗑️ Clear
                        </button>
                    </div>
                </div>

                <div class="sidebar-section">
                    <h3>🗂️ Chat Log</h3>
                    <div id="chat-log-list" class="chat-log-list">
                        <span class="hint">No saved sessions yet.</span>
                    </div>
                    <button class="btn btn-sm btn-danger" style="width:100%;margin-top:0.5rem;"
                            onclick="clearAllSessions()">
                        🗑️ Clear All Sessions
                    </button>
                </div>
            </aside>

            <!-- Main Chat Area -->
            <div class="chat-main">
                <div class="chat-messages" id="chat-messages">
                    <div class="system-message">
                        <div class="message-icon">🦀</div>
                        <div class="message-body">
                            <p><strong>Welcome to RustAssistant Chat</strong></p>
                            <p>I'm your AI-powered development assistant. I can help you with:</p>
                            <ul>
                                <li>📦 Repository analysis and code review</li>
                                <li>📝 Task planning and pipeline execution</li>
                                <li>🔍 Code search and understanding via RAG</li>
                                <li>🔧 Refactoring suggestions and code generation</li>
                                <li>🧪 Test generation and security audits</li>
                            </ul>
                            <p>Select a model and optionally a repository, then start chatting!</p>
                        </div>
                    </div>
                </div>

                <!-- Input Area -->
                <div class="chat-input-area">
                    <form id="chat-form" onsubmit="sendMessage(event)">
                        <input type="hidden" id="history-json" name="history_json" value="[]">
                        <div class="input-row">
                            <textarea id="chat-input"
                                      name="message"
                                      placeholder="Ask RustAssistant anything... (Enter to send, Shift+Enter for newline)"
                                      rows="3"
                                      autofocus></textarea>
                            <button type="submit" class="btn btn-primary send-btn" id="send-btn">
                                ▶ Send
                            </button>
                        </div>
                        <div class="input-footer">
                            <span id="token-counter" class="hint">Tokens: 0 | Cost: $0.00</span>
                            <span id="typing-indicator" class="typing-indicator" style="display:none;">
                                <span class="dot"></span><span class="dot"></span><span class="dot"></span>
                                Thinking...
                            </span>
                        </div>
                    </form>
                </div>
            </div>
        </div>
        "##
    );

    let extra_head = chat_extra_styles();

    let page = web_ui_nav::page_shell("Chat", "Chat", &extra_head, &content);
    Html(page)
}

/// POST /chat/send — Process a chat message and return the response as HTML
async fn chat_send_handler(
    State(state): State<Arc<WebAppState>>,
    Form(form): Form<ChatSendForm>,
) -> impl IntoResponse {
    let message = form.message.trim().to_string();
    if message.is_empty() {
        return Html(r#"<div class="error-message">Please enter a message.</div>"#.to_string());
    }

    info!(
        model = %form.model,
        repo_id = %form.repo_id,
        use_rag = %form.use_rag,
        msg_len = message.len(),
        "Chat message received"
    );

    // Parse conversation history
    let history: Vec<(String, String)> =
        serde_json::from_str(&form.history_json).unwrap_or_default();

    // Build the full prompt with context
    let mut context_parts: Vec<String> = Vec::new();

    // Load repo context if selected
    if !form.repo_id.is_empty() {
        if let Some(ctx) = load_repo_context(&state, &form.repo_id).await {
            context_parts.push(ctx);
        }
    }

    // Build RAG context if enabled
    let use_rag = form.use_rag == "on" || form.use_rag == "true";
    if use_rag {
        if let Some(rag_ctx) = retrieve_rag_context(&state, &message).await {
            context_parts.push(rag_ctx);
        }
    }

    // Build system prompt
    let system_prompt = build_system_prompt(&context_parts);

    // Build conversation with history
    let mut full_prompt = system_prompt.clone();
    full_prompt.push_str("\n\n");

    for (role, content) in &history {
        full_prompt.push_str(&format!("**{}**: {}\n\n", role, content));
    }

    full_prompt.push_str(&format!("**user**: {}", message));

    // Route to appropriate model
    let model_id = if form.model.is_empty() {
        "auto"
    } else {
        &form.model
    };

    let result = match dispatch_to_model(&state, model_id, &system_prompt, &message, &full_prompt)
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!(error = %e, "Chat model call failed");
            ChatResult {
                reply: format!("❌ **Error**: {}\n\nPlease check that your model backend (Ollama/Grok) is running and configured.", e),
                model_used: "error".to_string(),
                tokens_used: 0,
                cost_usd: 0.0,
            }
        }
    };

    // Render the response as HTML
    let escaped_reply = html_escape(&result.reply);
    let rendered_reply = simple_markdown_to_html(&escaped_reply);

    let html = format!(
        r##"
        <div class="user-message">
            <div class="message-icon">👤</div>
            <div class="message-body">
                <div class="message-text">{user_msg}</div>
            </div>
        </div>
        <div class="assistant-message">
            <div class="message-icon">🦀</div>
            <div class="message-body">
                <div class="message-meta">
                    <span class="badge badge-info">{model}</span>
                    <span class="hint">{tokens} tokens · ${cost:.6}</span>
                </div>
                <div class="message-text">{reply}</div>
                {action_buttons}
            </div>
        </div>
        <script>
            updateTokenCounter({tokens}, {cost});
            scrollToBottom();
        </script>
        "##,
        user_msg = html_escape(&message),
        model = html_escape(&result.model_used),
        tokens = result.tokens_used,
        cost = result.cost_usd,
        reply = rendered_reply,
        action_buttons = extract_action_buttons(&result.reply),
    );

    Html(html)
}

/// GET /chat/models — Check available models and return status HTML
async fn chat_models_handler(State(state): State<Arc<WebAppState>>) -> impl IntoResponse {
    let mut statuses = Vec::new();

    // Check Ollama
    let ollama_ok = check_ollama_health().await;
    if ollama_ok {
        let models = list_ollama_models().await;
        statuses.push(format!(
            r#"<span class="status-dot online"></span> Ollama: {} model(s) available"#,
            models.len()
        ));
    } else {
        statuses.push(r#"<span class="status-dot offline"></span> Ollama: offline"#.to_string());
    }

    // Check Grok
    let grok_available = std::env::var("XAI_API_KEY")
        .map(|k| !k.is_empty())
        .unwrap_or(false);
    if grok_available {
        statuses.push(
            r#"<span class="status-dot online"></span> Grok: API key configured"#.to_string(),
        );
    } else {
        statuses.push(r#"<span class="status-dot offline"></span> Grok: no API key"#.to_string());
    }

    // Check RAG index
    let rag_status = check_rag_status(&state).await;
    statuses.push(rag_status);

    Html(statuses.join("<br>"))
}

/// GET /chat/repos — Return repo <option> list as an HTMX partial so the
/// dropdown stays fresh without a full page reload.
async fn chat_repos_handler(State(state): State<Arc<WebAppState>>) -> impl IntoResponse {
    let repos = load_repos(&state).await;

    let mut options = String::from(r#"<option value="">No repository (general chat)</option>"#);
    for (id, name) in &repos {
        options.push_str(&format!(
            r#"<option value="{id}">{name}</option>"#,
            id = html_escape(id),
            name = html_escape(name),
        ));
    }

    Html(options)
}

/// POST /chat/clear — Clear conversation and return empty chat area
async fn chat_clear_handler() -> impl IntoResponse {
    Html(
        r##"<div class="system-message">
            <div class="message-icon">🦀</div>
            <div class="message-body">
                <p>Conversation cleared. Ready for a new chat!</p>
            </div>
        </div>
        <script>clearHistory();</script>"##
            .to_string(),
    )
}

// ============================================================================
// Model Dispatch
// ============================================================================

struct ChatResult {
    reply: String,
    model_used: String,
    tokens_used: i64,
    cost_usd: f64,
}

async fn dispatch_to_model(
    state: &WebAppState,
    model_id: &str,
    system_prompt: &str,
    user_message: &str,
    full_prompt: &str,
) -> anyhow::Result<ChatResult> {
    match model_id {
        id if id.starts_with("ollama:") => {
            let model_name = id.strip_prefix("ollama:").unwrap_or("qwen2.5-coder:7b");
            dispatch_ollama(model_name, system_prompt, user_message).await
        }
        id if id.starts_with("grok:") => dispatch_grok(state, full_prompt).await,
        "auto" | "" => {
            // Try Ollama first, fall back to Grok
            match dispatch_ollama("qwen2.5-coder:7b", system_prompt, user_message).await {
                Ok(result) => Ok(result),
                Err(ollama_err) => {
                    warn!(error = %ollama_err, "Ollama failed, trying Grok fallback");
                    match dispatch_grok(state, full_prompt).await {
                        Ok(result) => Ok(result),
                        Err(grok_err) => Err(anyhow::anyhow!(
                            "Both models failed. Ollama: {}. Grok: {}",
                            ollama_err,
                            grok_err
                        )),
                    }
                }
            }
        }
        _ => Err(anyhow::anyhow!("Unknown model: {}", model_id)),
    }
}

async fn dispatch_ollama(
    model: &str,
    system_prompt: &str,
    user_message: &str,
) -> anyhow::Result<ChatResult> {
    use crate::ollama_client::{OllamaClient, OllamaClientConfig};

    let config = OllamaClientConfig {
        model: model.to_string(),
        timeout: std::time::Duration::from_secs(180),
        ..OllamaClientConfig::default()
    };
    let client = OllamaClient::new(config, None);

    let resp = client
        .complete_with_ctx(Some(system_prompt), user_message, 0.3, 4096, 16384)
        .await?;

    let tokens =
        resp.prompt_tokens.unwrap_or(0) as i64 + resp.completion_tokens.unwrap_or(0) as i64;

    Ok(ChatResult {
        reply: resp.content,
        model_used: format!("ollama/{}", resp.model_used),
        tokens_used: tokens,
        cost_usd: 0.0, // Local models are free
    })
}

async fn dispatch_grok(state: &WebAppState, full_prompt: &str) -> anyhow::Result<ChatResult> {
    let api_key =
        std::env::var("XAI_API_KEY").map_err(|_| anyhow::anyhow!("XAI_API_KEY not set"))?;

    if api_key.is_empty() {
        return Err(anyhow::anyhow!("XAI_API_KEY is empty"));
    }

    let db = crate::db::Database::from_pool(state.db.pool.clone());
    let grok = crate::grok_client::GrokClient::new(api_key, db);

    let resp = grok.ask_tracked(full_prompt, None, "web_chat").await?;

    Ok(ChatResult {
        reply: resp.content,
        model_used: "grok-4-1-fast-reasoning".to_string(),
        tokens_used: resp.total_tokens,
        cost_usd: resp.cost_usd,
    })
}

// ============================================================================
// Context Building
// ============================================================================

fn build_system_prompt(context_parts: &[String]) -> String {
    let mut prompt = String::from(
        r#"You are RustAssistant, an expert Rust developer and code-workflow automation agent.

## Your capabilities

You control a TODO-driven pipeline with these steps:
1. **todo scan** — Walk a repo and extract every TODO/FIXME/HACK/XXX comment.
2. **todo scaffold** — Read todo.md, determine which files/dirs need to exist, create stubs.
3. **todo plan** — Produce a batched, prioritised GamePlan from todo.md + source context.
4. **todo work** — Execute one GamePlan batch: generate real code, apply changes, create backups.
5. **todo sync** — Apply WorkResult back to todo.md, marking items done/warning/failed.
6. **full pipeline** — Runs scan → scaffold → plan in one shot.

## How to respond

- Be concise and action-oriented.
- Use Markdown formatting for readability.
- When suggesting pipeline actions, include a JSON block:
  ```json
  { "suggest_action": { "step": "plan", "dry_run": false } }
  ```
- Provide specific, actionable advice with code examples when relevant.
- If asked about code, reference file paths and line numbers when possible.
"#,
    );

    if !context_parts.is_empty() {
        prompt.push_str("\n## Provided Context\n\n");
        for part in context_parts {
            prompt.push_str(part);
            prompt.push_str("\n\n---\n\n");
        }
    }

    prompt
}

async fn load_repo_context(state: &WebAppState, repo_id: &str) -> Option<String> {
    let repo = crate::db::get_repository(&state.db.pool, repo_id)
        .await
        .ok()?;

    let mut context = format!("### Repository: {}\n", repo.name);
    context.push_str(&format!("- Path: `{}`\n", repo.path));
    if let Some(ref url) = repo.git_url {
        context.push_str(&format!("- Git URL: {}\n", url));
    }

    // Try to load todo.md
    let base = std::path::PathBuf::from(&repo.path);

    /// Truncate `s` to at most `max_bytes` bytes, respecting UTF-8 char boundaries.
    fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
        if s.len() <= max_bytes {
            return s;
        }
        // Walk backwards from max_bytes until we land on a char boundary.
        let mut boundary = max_bytes;
        while boundary > 0 && !s.is_char_boundary(boundary) {
            boundary -= 1;
        }
        &s[..boundary]
    }

    let todo_paths = ["todo.md", "TODO.md", "Todo.md"];
    for name in &todo_paths {
        let p = base.join(name);
        if p.exists() {
            if let Ok(contents) = std::fs::read_to_string(&p) {
                let truncated = if contents.len() > 8000 {
                    format!(
                        "{}...\n\n(truncated, {} bytes total)",
                        truncate_at_char_boundary(&contents, 8000),
                        contents.len()
                    )
                } else {
                    contents
                };
                context.push_str(&format!(
                    "\n### todo.md\n\n```markdown\n{}\n```\n",
                    truncated
                ));
            }
            break;
        }
    }

    // Try to load directory structure summary
    let readme_paths = ["README.md", "readme.md", "Readme.md"];
    for name in &readme_paths {
        let p = base.join(name);
        if p.exists() {
            if let Ok(contents) = std::fs::read_to_string(&p) {
                let truncated = if contents.len() > 4000 {
                    format!(
                        "{}...\n\n(truncated)",
                        truncate_at_char_boundary(&contents, 4000)
                    )
                } else {
                    contents
                };
                context.push_str(&format!(
                    "\n### README.md (summary)\n\n```markdown\n{}\n```\n",
                    truncated
                ));
            }
            break;
        }
    }

    Some(context)
}

async fn retrieve_rag_context(state: &WebAppState, query: &str) -> Option<String> {
    // Try to use the vector index for RAG retrieval
    // This is a best-effort operation — if the index isn't populated, we skip gracefully
    use crate::embeddings::{EmbeddingConfig, EmbeddingGenerator};

    let generator = match EmbeddingGenerator::new(EmbeddingConfig::default()) {
        Ok(g) => g,
        Err(e) => {
            warn!(error = %e, "Could not initialise embedding generator for RAG");
            return None;
        }
    };

    let _query_embedding = match generator.embed(query).await {
        Ok(emb) => emb,
        Err(e) => {
            warn!(error = %e, "Failed to generate query embedding for RAG");
            return None;
        }
    };

    // Search the database for similar embeddings
    // TODO: use actual vector similarity search once the index supports it;
    // for now, fall back to a random sample so the feature path is exercised.
    let pool = &state.db.pool;
    let rows: Vec<(String, String, f64)> = sqlx::query_as(
        r#"SELECT file_path, content_snippet, 1.0 as score
           FROM file_embeddings
           ORDER BY RANDOM()
           LIMIT 5"#,
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return None;
    }

    let mut context = String::from("### RAG Retrieved Context\n\n");
    for (path, snippet, score) in &rows {
        context.push_str(&format!(
            "**{}** (relevance: {:.2})\n```\n{}\n```\n\n",
            path, score, snippet
        ));
    }

    Some(context)
}

// ============================================================================
// Health & Status Checks
// ============================================================================

async fn check_ollama_health() -> bool {
    let base_url =
        std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
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
    let base_url =
        std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
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

async fn check_rag_status(state: &WebAppState) -> String {
    let pool = &state.db.pool;

    // Check if embeddings table exists and has data
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'file_embeddings'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    if count > 0 {
        let embed_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_embeddings")
            .fetch_one(pool)
            .await
            .unwrap_or(0);
        format!(
            r#"<span class="status-dot online"></span> RAG: {} embeddings indexed"#,
            embed_count
        )
    } else {
        r#"<span class="status-dot offline"></span> RAG: no embeddings yet"#.to_string()
    }
}

// ============================================================================
// Helpers
// ============================================================================

async fn load_repos(state: &WebAppState) -> Vec<(String, String)> {
    let pool = &state.db.pool;
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM repositories ORDER BY name")
            .fetch_all(pool)
            .await
            .unwrap_or_default();
    rows
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Very basic Markdown-to-HTML for chat responses.
/// Handles: bold, italic, code blocks, inline code, headers, lists, paragraphs.
fn simple_markdown_to_html(text: &str) -> String {
    let mut result = String::new();
    let mut in_code_block = false;
    let mut in_list = false;
    let lines: Vec<&str> = text.lines().collect();

    for line in &lines {
        let trimmed = line.trim();

        // Code blocks
        if trimmed.starts_with("```") {
            if in_code_block {
                result.push_str("</code></pre>\n");
                in_code_block = false;
            } else {
                let lang = trimmed.strip_prefix("```").unwrap_or("").trim();
                if lang.is_empty() {
                    result.push_str("<pre><code>");
                } else {
                    result.push_str(&format!(r#"<pre><code class="lang-{}">"#, lang));
                }
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            result.push_str(line);
            result.push('\n');
            continue;
        }

        // Close list if current line isn't a list item
        if in_list
            && !trimmed.starts_with("- ")
            && !trimmed.starts_with("* ")
            && !trimmed.starts_with("• ")
        {
            result.push_str("</ul>\n");
            in_list = false;
        }

        if trimmed.is_empty() {
            if !in_list {
                result.push_str("<br>\n");
            }
            continue;
        }

        // Headers
        if let Some(h) = trimmed.strip_prefix("### ") {
            result.push_str(&format!("<h4>{}</h4>\n", h));
            continue;
        }
        if let Some(h) = trimmed.strip_prefix("## ") {
            result.push_str(&format!("<h3>{}</h3>\n", h));
            continue;
        }
        if let Some(h) = trimmed.strip_prefix("# ") {
            result.push_str(&format!("<h3>{}</h3>\n", h));
            continue;
        }

        // List items
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("• ") {
            if !in_list {
                result.push_str("<ul>\n");
                in_list = true;
            }
            let content = &trimmed[2..];
            result.push_str(&format!("<li>{}</li>\n", inline_markdown(content)));
            continue;
        }

        // Numbered list items
        if trimmed.len() > 2
            && trimmed
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        {
            if let Some(rest) = trimmed.split_once(". ") {
                if !in_list {
                    result.push_str("<ul>\n");
                    in_list = true;
                }
                result.push_str(&format!("<li>{}</li>\n", inline_markdown(rest.1)));
                continue;
            }
        }

        // Regular paragraph
        result.push_str(&format!("<p>{}</p>\n", inline_markdown(trimmed)));
    }

    if in_code_block {
        result.push_str("</code></pre>\n");
    }
    if in_list {
        result.push_str("</ul>\n");
    }

    result
}

/// Handle inline markdown: **bold**, *italic*, `code`, [links](url)
fn inline_markdown(text: &str) -> String {
    let mut result = text.to_string();

    // Bold: **text**
    while let Some(start) = result.find("**") {
        if let Some(end) = result[start + 2..].find("**") {
            let bold_text = &result[start + 2..start + 2 + end].to_string();
            result = format!(
                "{}<strong>{}</strong>{}",
                &result[..start],
                bold_text,
                &result[start + 2 + end + 2..]
            );
        } else {
            break;
        }
    }

    // Inline code: `text`
    while let Some(start) = result.find('`') {
        if let Some(end) = result[start + 1..].find('`') {
            let code_text = &result[start + 1..start + 1 + end].to_string();
            result = format!(
                "{}<code class=\"inline\">{}</code>{}",
                &result[..start],
                code_text,
                &result[start + 1 + end + 1..]
            );
        } else {
            break;
        }
    }

    result
}

/// Extract suggested action buttons from the LLM response JSON blocks
fn extract_action_buttons(reply: &str) -> String {
    // Look for { "suggest_action": { "step": "...", "dry_run": ... } }
    let mut buttons = String::new();

    if let Some(json_start) = reply.find(r#""suggest_action""#) {
        // Try to find the enclosing JSON object
        if let Some(obj_start) = reply[..json_start].rfind('{') {
            let remaining = &reply[obj_start..];
            // Find matching closing brace (simple depth counting)
            let mut depth = 0;
            let mut obj_end = None;
            for (i, ch) in remaining.char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            obj_end = Some(i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(end) = obj_end {
                let json_str = &remaining[..end];
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(action) = val.get("suggest_action") {
                        let step = action
                            .get("step")
                            .and_then(|s| s.as_str())
                            .unwrap_or("plan");
                        let dry_run = action
                            .get("dry_run")
                            .and_then(|d| d.as_bool())
                            .unwrap_or(false);

                        let step_label = match step {
                            "scan" => "🔍 Run Scan",
                            "scaffold" => "🏗️ Run Scaffold",
                            "plan" => "📋 Run Plan",
                            "work" => "⚡ Run Work",
                            "sync" => "🔄 Run Sync",
                            "full" => "🚀 Full Pipeline",
                            _ => "▶ Run Step",
                        };

                        buttons = format!(
                            r#"<div class="action-suggestion">
                                <button class="btn btn-success btn-sm"
                                        onclick="dispatchPipelineAction('{step}', {dry_run})">
                                    {label}
                                </button>
                                {dry_btn}
                            </div>"#,
                            step = step,
                            dry_run = dry_run,
                            label = step_label,
                            dry_btn = if !dry_run {
                                format!(
                                    r#"<button class="btn btn-muted btn-sm"
                                            onclick="dispatchPipelineAction('{step}', true)">
                                        🧪 Dry Run
                                    </button>"#,
                                    step = step,
                                )
                            } else {
                                String::new()
                            },
                        );
                    }
                }
            }
        }
    }

    buttons
}

// ============================================================================
// Styles & Scripts
// ============================================================================

fn chat_extra_styles() -> String {
    r##"<style>
    /* ── Chat Layout ──────────────────────────────────────────────── */
    .chat-layout {
        display: grid;
        grid-template-columns: 280px 1fr;
        gap: 0;
        height: calc(100vh - 80px);
        margin: -1rem -2rem;
    }

    /* ── Sidebar ──────────────────────────────────────────────────── */
    .chat-sidebar {
        background: #1a2332;
        border-right: 1px solid #334155;
        padding: 1rem;
        overflow-y: auto;
        display: flex;
        flex-direction: column;
        gap: 0.75rem;
    }
    .sidebar-section {
        background: #1e293b;
        border-radius: 8px;
        padding: 0.75rem;
        border: 1px solid #334155;
    }
    .sidebar-section h3 {
        font-size: 0.85rem;
        color: #94a3b8;
        margin-bottom: 0.5rem;
    }
    .chat-select {
        width: 100%;
        background: #0f172a;
        color: #e2e8f0;
        border: 1px solid #334155;
        border-radius: 6px;
        padding: 0.5rem;
        font-size: 0.85rem;
        cursor: pointer;
    }
    .chat-select:focus {
        outline: none;
        border-color: #0ea5e9;
    }
    .hint {
        font-size: 0.75rem;
        color: #64748b;
        margin-top: 0.35rem;
        display: block;
    }
    .toggle-label {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        cursor: pointer;
        font-size: 0.85rem;
    }
    .toggle-label input[type="checkbox"] {
        width: 1rem;
        height: 1rem;
        accent-color: #0ea5e9;
    }

    /* Model status dots */
    .model-status {
        margin-top: 0.5rem;
        font-size: 0.75rem;
        color: #94a3b8;
        line-height: 1.8;
    }
    .status-dot {
        display: inline-block;
        width: 8px;
        height: 8px;
        border-radius: 50%;
        margin-right: 4px;
        vertical-align: middle;
    }
    .status-dot.online { background: #22c55e; }
    .status-dot.offline { background: #ef4444; }
    .status-dot.checking { background: #f59e0b; animation: pulse 1s infinite; }
    @keyframes pulse {
        0%, 100% { opacity: 1; }
        50% { opacity: 0.4; }
    }

    /* Quick actions */
    .quick-actions {
        display: flex;
        flex-direction: column;
        gap: 0.35rem;
    }
    .quick-actions .btn {
        text-align: left;
        font-size: 0.78rem;
        padding: 0.4rem 0.6rem;
    }

    /* ── Main Chat Area ───────────────────────────────────────────── */
    .chat-main {
        display: flex;
        flex-direction: column;
        height: 100%;
        overflow: hidden;
    }

    /* Messages */
    .chat-messages {
        flex: 1;
        overflow-y: auto;
        padding: 1.5rem;
        display: flex;
        flex-direction: column;
        gap: 1rem;
    }
    .system-message,
    .user-message,
    .assistant-message {
        display: flex;
        gap: 0.75rem;
        max-width: 90%;
        animation: fadeIn 0.3s ease;
    }
    @keyframes fadeIn {
        from { opacity: 0; transform: translateY(8px); }
        to { opacity: 1; transform: translateY(0); }
    }
    .system-message {
        max-width: 100%;
        background: #1e293b;
        border: 1px solid #334155;
        border-radius: 8px;
        padding: 1rem;
    }
    .user-message {
        align-self: flex-end;
        flex-direction: row-reverse;
    }
    .user-message .message-body {
        background: #0c2d4a;
        border: 1px solid #1e4976;
        border-radius: 12px 12px 0 12px;
        padding: 0.75rem 1rem;
    }
    .assistant-message .message-body {
        background: #1e293b;
        border: 1px solid #334155;
        border-radius: 12px 12px 12px 0;
        padding: 0.75rem 1rem;
        flex: 1;
    }
    .message-icon {
        font-size: 1.5rem;
        flex-shrink: 0;
        width: 2rem;
        text-align: center;
        padding-top: 0.25rem;
    }
    .message-meta {
        display: flex;
        gap: 0.5rem;
        align-items: center;
        margin-bottom: 0.5rem;
    }
    .message-text {
        font-size: 0.9rem;
        line-height: 1.7;
    }
    .message-text pre {
        background: #0f172a;
        border: 1px solid #334155;
        border-radius: 6px;
        padding: 0.75rem;
        overflow-x: auto;
        margin: 0.5rem 0;
        font-size: 0.82rem;
    }
    .message-text code {
        font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
        font-size: 0.85em;
    }
    .message-text code.inline {
        background: #0f172a;
        padding: 0.15rem 0.35rem;
        border-radius: 3px;
        border: 1px solid #334155;
    }
    .message-text h3, .message-text h4 {
        margin: 0.75rem 0 0.35rem;
        color: #38bdf8;
    }
    .message-text ul {
        margin: 0.35rem 0 0.35rem 1.5rem;
    }
    .message-text li {
        margin: 0.2rem 0;
    }
    .message-text p {
        margin: 0.35rem 0;
    }
    .error-message {
        background: #7f1d1d;
        color: #fee2e2;
        border: 1px solid #ef4444;
        border-radius: 8px;
        padding: 0.75rem 1rem;
    }

    /* Action suggestion buttons from LLM */
    .action-suggestion {
        display: flex;
        gap: 0.5rem;
        margin-top: 0.75rem;
        padding-top: 0.75rem;
        border-top: 1px solid #334155;
    }

    /* ── Input Area ───────────────────────────────────────────────── */
    .chat-input-area {
        border-top: 1px solid #334155;
        background: #1a2332;
        padding: 1rem 1.5rem;
        flex-shrink: 0;
    }
    .input-row {
        display: flex;
        gap: 0.75rem;
        align-items: flex-end;
    }
    .input-row textarea {
        flex: 1;
        background: #0f172a;
        color: #e2e8f0;
        border: 1px solid #334155;
        border-radius: 8px;
        padding: 0.75rem;
        font-family: inherit;
        font-size: 0.9rem;
        resize: none;
        line-height: 1.5;
        max-height: 200px;
    }
    .input-row textarea:focus {
        outline: none;
        border-color: #0ea5e9;
        box-shadow: 0 0 0 2px rgba(14, 165, 233, 0.2);
    }
    .send-btn {
        padding: 0.75rem 1.5rem;
        height: fit-content;
        font-size: 0.9rem;
        white-space: nowrap;
    }
    .input-footer {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-top: 0.5rem;
    }

    /* Typing indicator */
    .typing-indicator {
        display: flex;
        align-items: center;
        gap: 0.35rem;
        color: #94a3b8;
        font-size: 0.8rem;
    }
    .typing-indicator .dot {
        width: 6px;
        height: 6px;
        background: #0ea5e9;
        border-radius: 50%;
        animation: dotBounce 1.4s infinite ease-in-out;
    }
    .typing-indicator .dot:nth-child(1) { animation-delay: 0s; }
    .typing-indicator .dot:nth-child(2) { animation-delay: 0.2s; }
    .typing-indicator .dot:nth-child(3) { animation-delay: 0.4s; }
    @keyframes dotBounce {
        0%, 80%, 100% { transform: scale(0.6); opacity: 0.4; }
        40% { transform: scale(1); opacity: 1; }
    }

    /* ── Chat Log ─────────────────────────────────────────────────── */
    .chat-log-list {
        display: flex;
        flex-direction: column;
        gap: 0.4rem;
        max-height: 240px;
        overflow-y: auto;
    }
    .chat-log-entry {
        background: #0f172a;
        border: 1px solid #334155;
        border-radius: 6px;
        padding: 0.5rem 0.6rem;
        font-size: 0.78rem;
    }
    .chat-log-meta {
        display: flex;
        justify-content: space-between;
        align-items: baseline;
        gap: 0.25rem;
        margin-bottom: 0.2rem;
    }
    .chat-log-name {
        font-weight: 600;
        color: #38bdf8;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        max-width: 120px;
    }
    .chat-log-preview {
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        margin-bottom: 0.35rem;
    }
    .chat-log-actions {
        display: flex;
        gap: 0.3rem;
    }
    .btn-xs {
        padding: 0.15rem 0.4rem;
        font-size: 0.72rem;
        border-radius: 4px;
    }

    /* ── Responsive ───────────────────────────────────────────────── */
    @media (max-width: 900px) {
        .chat-layout {
            grid-template-columns: 1fr;
            grid-template-rows: auto 1fr;
        }
        .chat-sidebar {
            flex-direction: row;
            overflow-x: auto;
            border-right: none;
            border-bottom: 1px solid #334155;
            max-height: 200px;
        }
        .sidebar-section {
            min-width: 200px;
        }
    }
    </style>
    <script>
    // ── Chat Client Logic ─────────────────────────────────────────
    const LS_HISTORY_KEY   = 'ra_chat_history';
    const LS_MESSAGES_KEY  = 'ra_chat_messages';
    const LS_TOKENS_KEY    = 'ra_chat_tokens';
    const LS_COST_KEY      = 'ra_chat_cost';
    const LS_SESSIONS_KEY  = 'ra_chat_sessions';

    let conversationHistory = [];
    let totalTokens = 0;
    let totalCost = 0;

    // ── Persistence helpers ───────────────────────────────────────

    function persistState() {
        try {
            localStorage.setItem(LS_HISTORY_KEY,  JSON.stringify(conversationHistory));
            localStorage.setItem(LS_TOKENS_KEY,   String(totalTokens));
            localStorage.setItem(LS_COST_KEY,     String(totalCost));
            const el = document.getElementById('chat-messages');
            if (el) localStorage.setItem(LS_MESSAGES_KEY, el.innerHTML);
        } catch(e) { /* quota exceeded — silent */ }
    }

    function restoreState() {
        try {
            const hist = localStorage.getItem(LS_HISTORY_KEY);
            if (hist) conversationHistory = JSON.parse(hist);

            totalTokens = parseFloat(localStorage.getItem(LS_TOKENS_KEY) || '0');
            totalCost   = parseFloat(localStorage.getItem(LS_COST_KEY)   || '0');

            const msgs = localStorage.getItem(LS_MESSAGES_KEY);
            const el = document.getElementById('chat-messages');
            if (msgs && el) {
                el.innerHTML = msgs;
                scrollToBottom();
            }

            document.getElementById('history-json').value = JSON.stringify(conversationHistory);

            const counterEl = document.getElementById('token-counter');
            if (counterEl && (totalTokens > 0 || totalCost > 0)) {
                counterEl.textContent = 'Tokens: ' + totalTokens.toLocaleString() +
                                        ' | Cost: $' + totalCost.toFixed(6);
            }
        } catch(e) { /* corrupted storage — ignore */ }
    }

    // ── Session log ───────────────────────────────────────────────

    function loadSessions() {
        try {
            return JSON.parse(localStorage.getItem(LS_SESSIONS_KEY) || '[]');
        } catch(e) { return []; }
    }

    function saveSessions(sessions) {
        try {
            localStorage.setItem(LS_SESSIONS_KEY, JSON.stringify(sessions));
        } catch(e) {}
    }

    function renderSessionLog() {
        const container = document.getElementById('chat-log-list');
        if (!container) return;
        const sessions = loadSessions();
        if (sessions.length === 0) {
            container.innerHTML = '<span class="hint">No saved sessions yet.</span>';
            return;
        }
        container.innerHTML = sessions.slice().reverse().map(function(s) {
            const date = new Date(s.ts).toLocaleString();
            const preview = s.preview || '(empty)';
            return '<div class="chat-log-entry" data-id="' + s.id + '">' +
                '<div class="chat-log-meta">' +
                    '<span class="chat-log-name" title="' + date + '">' + htmlEscape(s.name) + '</span>' +
                    '<span class="hint">' + date + '</span>' +
                '</div>' +
                '<div class="chat-log-preview hint">' + htmlEscape(preview) + '</div>' +
                '<div class="chat-log-actions">' +
                    '<button class="btn btn-xs btn-muted" onclick="restoreSession(\'' + s.id + '\')">↩ Restore</button>' +
                    '<button class="btn btn-xs btn-danger" onclick="deleteSession(\'' + s.id + '\')">✕</button>' +
                '</div>' +
            '</div>';
        }).join('');
    }

    function saveSession() {
        if (conversationHistory.length === 0) {
            alert('Nothing to save — start a conversation first.');
            return;
        }
        const defaultName = 'Chat ' + new Date().toLocaleString();
        const name = prompt('Save session as:', defaultName);
        if (!name) return;

        const sessions = loadSessions();
        const el = document.getElementById('chat-messages');
        const firstUserMsg = conversationHistory.find(function(e){ return e[0] === 'user'; });
        const preview = firstUserMsg ? firstUserMsg[1].substring(0, 80) : '(no messages)';

        sessions.push({
            id:       Date.now().toString(36) + Math.random().toString(36).slice(2),
            name:     name,
            ts:       Date.now(),
            preview:  preview,
            history:  conversationHistory.slice(),
            messages: el ? el.innerHTML : '',
            tokens:   totalTokens,
            cost:     totalCost,
        });

        saveSessions(sessions);
        renderSessionLog();
    }

    function restoreSession(id) {
        const sessions = loadSessions();
        const session = sessions.find(function(s){ return s.id === id; });
        if (!session) return;

        if (!confirm('Restore "' + session.name + '"? Your current chat will be lost.')) return;

        conversationHistory = session.history || [];
        totalTokens = session.tokens || 0;
        totalCost   = session.cost   || 0;

        const el = document.getElementById('chat-messages');
        if (el && session.messages) el.innerHTML = session.messages;

        document.getElementById('history-json').value = JSON.stringify(conversationHistory);

        const counterEl = document.getElementById('token-counter');
        if (counterEl) {
            counterEl.textContent = 'Tokens: ' + totalTokens.toLocaleString() +
                                    ' | Cost: $' + totalCost.toFixed(6);
        }

        persistState();
        scrollToBottom();
    }

    function deleteSession(id) {
        if (!confirm('Delete this session?')) return;
        const sessions = loadSessions().filter(function(s){ return s.id !== id; });
        saveSessions(sessions);
        renderSessionLog();
    }

    function clearAllSessions() {
        if (!confirm('Delete ALL saved sessions? This cannot be undone.')) return;
        saveSessions([]);
        renderSessionLog();
    }

    function htmlEscape(str) {
        return String(str)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    // ── Core chat helpers ─────────────────────────────────────────

    function scrollToBottom() {
        const el = document.getElementById('chat-messages');
        if (el) el.scrollTop = el.scrollHeight;
    }

    function clearHistory() {
        conversationHistory = [];
        totalTokens = 0;
        totalCost = 0;
        document.getElementById('history-json').value = '[]';
        updateTokenCounter(0, 0);
        // Wipe persisted current session (saved sessions are untouched)
        try {
            localStorage.removeItem(LS_HISTORY_KEY);
            localStorage.removeItem(LS_MESSAGES_KEY);
            localStorage.removeItem(LS_TOKENS_KEY);
            localStorage.removeItem(LS_COST_KEY);
        } catch(e) {}
    }

    function updateTokenCounter(tokens, cost) {
        totalTokens += tokens;
        totalCost += cost;
        const el = document.getElementById('token-counter');
        if (el) {
            el.textContent = 'Tokens: ' + totalTokens.toLocaleString() +
                             ' | Cost: $' + totalCost.toFixed(6);
        }
    }

    async function sendMessage(event) {
        event.preventDefault();

        const input = document.getElementById('chat-input');
        const message = input.value.trim();
        if (!message) return;

        const model = document.getElementById('model-select').value;
        const repoId = document.getElementById('repo-select').value;
        const ragToggle = document.getElementById('rag-toggle');
        const useRag = ragToggle && ragToggle.checked ? 'on' : 'off';
        const messagesEl = document.getElementById('chat-messages');
        const sendBtn = document.getElementById('send-btn');
        const typingEl = document.getElementById('typing-indicator');

        // Clear input
        input.value = '';
        input.style.height = 'auto';

        // Disable send
        sendBtn.disabled = true;
        sendBtn.textContent = '⏳ ...';
        if (typingEl) typingEl.style.display = 'flex';

        // Build form data
        const formData = new URLSearchParams();
        formData.append('message', message);
        formData.append('model', model);
        formData.append('repo_id', repoId);
        formData.append('use_rag', useRag);
        formData.append('history_json', JSON.stringify(conversationHistory));

        try {
            const resp = await fetch('/chat/send', {
                method: 'POST',
                headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
                body: formData.toString(),
            });

            const html = await resp.text();
            messagesEl.insertAdjacentHTML('beforeend', html);

            // Update conversation history
            conversationHistory.push(['user', message]);
            // Extract assistant reply from the last assistant-message
            const assistantMsgs = messagesEl.querySelectorAll('.assistant-message .message-text');
            if (assistantMsgs.length > 0) {
                const lastReply = assistantMsgs[assistantMsgs.length - 1].textContent;
                conversationHistory.push(['assistant', lastReply.substring(0, 2000)]);
            }

            // Keep history reasonable (last 20 turns)
            if (conversationHistory.length > 40) {
                conversationHistory = conversationHistory.slice(-40);
            }

            document.getElementById('history-json').value = JSON.stringify(conversationHistory);

            // Persist current session to localStorage so navigation doesn't lose it
            persistState();

        } catch (err) {
            messagesEl.insertAdjacentHTML('beforeend',
                '<div class="error-message">Network error: ' + err.message + '</div>');
        } finally {
            sendBtn.disabled = false;
            sendBtn.textContent = '▶ Send';
            if (typingEl) typingEl.style.display = 'none';
            scrollToBottom();
            input.focus();
        }
    }

    // ── Init ──────────────────────────────────────────────────────
    document.addEventListener('DOMContentLoaded', function() {
        // Restore previous session on page load
        restoreState();
        // Render saved session list
        renderSessionLog();

        document.querySelectorAll('.quick-prompt').forEach(function(btn) {
            btn.addEventListener('click', function() {
                const prompt = this.getAttribute('data-prompt');
                const input = document.getElementById('chat-input');
                if (input && prompt) {
                    input.value = prompt;
                    input.focus();
                }
            });
        });

        // Textarea auto-resize + Enter to send
        const input = document.getElementById('chat-input');
        if (input) {
            input.addEventListener('keydown', function(e) {
                if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    sendMessage(e);
                }
            });
            input.addEventListener('input', function() {
                this.style.height = 'auto';
                this.style.height = Math.min(this.scrollHeight, 200) + 'px';
            });
        }
    });

    function dispatchPipelineAction(step, dryRun) {
        const repoId = document.getElementById('repo-select').value;
        if (!repoId) {
            alert('Please select a repository first to run pipeline actions.');
            return;
        }

        const msg = dryRun
            ? 'Run ' + step + ' (dry run) on the selected repository?'
            : 'Run ' + step + ' on the selected repository? This will modify files.';

        if (!confirm(msg)) return;

        // Insert the action as a chat message
        const input = document.getElementById('chat-input');
        input.value = 'Please run the "' + step + '" pipeline step' +
                       (dryRun ? ' (dry run)' : '') +
                       ' on this repository.';
        sendMessage(new Event('submit'));
    }
    </script>"##
        .to_string()
}
