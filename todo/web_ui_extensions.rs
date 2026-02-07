// src/web_ui_extensions.rs
//!
//! Extension routes for the Web UI:
//! - Ideas (quick thought capture with tagging)
//! - Documents (knowledge base)
//! - Scan progress (real-time indicators)
//! - Activity feed (scan events)
//! - Repository settings (scan interval editing)
//! - Enhanced health/stats endpoint
//!
//! These extend the existing `web_ui::create_router` by adding new routes.
//! Import and merge in server.rs.

use crate::db::documents::{
    count_documents, count_ideas, create_document, create_idea, delete_document, delete_idea,
    get_document, list_documents, list_ideas, list_tags, search_documents, search_tags,
    update_document, update_idea_status,
};
use crate::db::scan_events::{get_recent_events, get_repo_events, ScanEvent};
use crate::web_ui::{timezone_js, timezone_selector_html, WebAppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Form, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

// ============================================================================
// Form Structs
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct IdeaForm {
    pub content: String,
    pub tags: Option<String>,
    pub project: Option<String>,
    pub category: Option<String>,
    pub priority: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct DocumentForm {
    pub title: String,
    pub content: String,
    pub doc_type: Option<String>,
    pub tags: Option<String>,
    pub project: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RepoSettingsForm {
    pub scan_interval_minutes: i32,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub tag: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub doc_type: Option<String>,
}

// ============================================================================
// Ideas Page
// ============================================================================

pub async fn ideas_handler(
    State(state): State<Arc<WebAppState>>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    let pool = &state.db.pool;

    let ideas = list_ideas(
        pool,
        100,
        params.status.as_deref(),
        params.category.as_deref(),
        params.tag.as_deref(),
        None,
    )
    .await
    .unwrap_or_default();

    let tags = list_tags(pool, 50).await.unwrap_or_default();
    let total = count_ideas(pool).await.unwrap_or(0);

    // Build ideas cards HTML
    let ideas_html: String = if ideas.is_empty() {
        r#"<div class="empty-state">
            <p>No ideas yet. Capture your first thought above!</p>
        </div>"#.to_string()
    } else {
        ideas.iter().map(|idea| {
            let priority_badge = match idea.priority {
                1 => r#"<span class="badge badge-danger">Urgent</span>"#,
                2 => r#"<span class="badge badge-warning">High</span>"#,
                3 => r#"<span class="badge badge-info">Normal</span>"#,
                4 => r#"<span class="badge badge-muted">Low</span>"#,
                _ => r#"<span class="badge badge-muted">Someday</span>"#,
            };
            let status_badge = match idea.status.as_str() {
                "inbox" => r#"<span class="badge badge-warning">Inbox</span>"#,
                "active" => r#"<span class="badge badge-info">Active</span>"#,
                "in_progress" => r#"<span class="badge badge-primary">In Progress</span>"#,
                "done" => r#"<span class="badge badge-success">Done</span>"#,
                _ => r#"<span class="badge badge-muted">Archived</span>"#,
            };
            let category_str = idea.category.as_deref().unwrap_or("—");
            let tags_html = idea.tags.as_deref().map(|t| {
                t.split(',').map(|tag| format!(
                    r#"<span class="tag">{}</span>"#, tag.trim()
                )).collect::<Vec<_>>().join(" ")
            }).unwrap_or_default();

            format!(r#"
            <div class="card idea-card" data-status="{}">
                <div class="idea-header">
                    <div class="idea-badges">{} {} <span class="badge badge-muted">{}</span></div>
                    <div class="idea-actions">
                        <a href="/ideas/{}/status/active" class="btn-small btn-success" title="Activate">▶</a>
                        <a href="/ideas/{}/status/done" class="btn-small btn-primary" title="Done">✓</a>
                        <a href="/ideas/{}/status/archived" class="btn-small btn-muted" title="Archive">📦</a>
                        <a href="/ideas/{}/delete" class="btn-small btn-danger" title="Delete"
                           onclick="return confirm('Delete this idea?')">✕</a>
                    </div>
                </div>
                <div class="idea-content">{}</div>
                <div class="idea-tags">{}</div>
                <div class="idea-meta">
                    <span data-utc="{}">—</span>
                </div>
            </div>"#,
                idea.status,
                priority_badge, status_badge, category_str,
                idea.id, idea.id, idea.id, idea.id,
                idea.content,
                tags_html,
                format_timestamp(idea.created_at),
            )
        }).collect::<Vec<_>>().join("\n")
    };

    // Tag filter buttons
    let tag_filter_html: String = tags.iter().take(15).map(|t| {
        let active = params.tag.as_deref() == Some(&t.name);
        let cls = if active { "tag tag-active" } else { "tag" };
        format!(
            r#"<a href="/ideas?tag={}" class="{}">{} ({})</a>"#,
            t.name, cls, t.name, t.usage_count
        )
    }).collect::<Vec<_>>().join(" ");

    Html(format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Ideas - RustAssistant</title>
    {style}
    <style>
        .quick-capture {{ display: flex; gap: 0.5rem; margin-bottom: 1.5rem; }}
        .quick-capture input[type="text"] {{ flex: 1; padding: 0.75rem; border: 1px solid #334155; background: #1e293b;
            color: #e2e8f0; border-radius: 6px; font-size: 1rem; }}
        .quick-capture input[type="text"]:focus {{ outline: none; border-color: #0ea5e9; }}
        .quick-capture select, .quick-capture input[name="tags"] {{ padding: 0.75rem; border: 1px solid #334155;
            background: #1e293b; color: #e2e8f0; border-radius: 6px; }}
        .idea-card {{ margin-bottom: 0.75rem; padding: 1rem; }}
        .idea-header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; }}
        .idea-badges {{ display: flex; gap: 0.3rem; flex-wrap: wrap; }}
        .idea-actions {{ display: flex; gap: 0.25rem; }}
        .idea-content {{ font-size: 1rem; line-height: 1.5; margin-bottom: 0.5rem; white-space: pre-wrap; }}
        .idea-tags {{ display: flex; gap: 0.3rem; flex-wrap: wrap; margin-bottom: 0.3rem; }}
        .idea-meta {{ font-size: 0.8rem; color: #64748b; }}
        .tag {{ background: #1e3a5f; color: #7dd3fc; padding: 2px 8px; border-radius: 4px;
            font-size: 0.75rem; text-decoration: none; cursor: pointer; }}
        .tag-active {{ background: #0ea5e9; color: white; }}
        .tag:hover {{ background: #0284c7; color: white; }}
        .filter-bar {{ display: flex; gap: 0.5rem; flex-wrap: wrap; margin-bottom: 1rem; align-items: center; }}
        .filter-bar a.active {{ background: #0ea5e9; color: white; }}
        .stat-row {{ display: flex; gap: 1rem; margin-bottom: 1rem; }}
        .stat-box {{ background: #1e293b; padding: 0.5rem 1rem; border-radius: 6px; text-align: center; }}
        .stat-box .num {{ font-size: 1.5rem; font-weight: 700; color: #0ea5e9; }}
        .stat-box .label {{ font-size: 0.75rem; color: #94a3b8; }}
        .badge {{ padding: 2px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 600; }}
        .badge-danger {{ background: #ef4444; color: white; }}
        .badge-warning {{ background: #f59e0b; color: white; }}
        .badge-info {{ background: #0ea5e9; color: white; }}
        .badge-primary {{ background: #6366f1; color: white; }}
        .badge-success {{ background: #22c55e; color: white; }}
        .badge-muted {{ background: #475569; color: #cbd5e1; }}
        .empty-state {{ text-align: center; padding: 3rem; color: #64748b; }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos">Repos</a>
                <a href="/queue">Queue</a>
                <a href="/ideas" class="active">Ideas</a>
                <a href="/docs">Docs</a>
                <a href="/activity">Activity</a>
                {tz_selector}
            </nav>
        </header>

        <h2>💡 Ideas ({total} active)</h2>

        <!-- Quick capture form -->
        <form action="/ideas/add" method="post" class="quick-capture">
            <input type="text" name="content" placeholder="What's on your mind? Capture it here..." required autofocus>
            <input type="text" name="tags" placeholder="tags (comma-sep)" style="max-width: 200px;">
            <select name="category">
                <option value="">Category...</option>
                <option value="feature">Feature</option>
                <option value="bug">Bug</option>
                <option value="improvement">Improvement</option>
                <option value="research">Research</option>
                <option value="question">Question</option>
                <option value="random">Random</option>
            </select>
            <select name="priority">
                <option value="3">Normal</option>
                <option value="1">Urgent</option>
                <option value="2">High</option>
                <option value="4">Low</option>
                <option value="5">Someday</option>
            </select>
            <button type="submit" class="btn btn-primary">+ Capture</button>
        </form>

        <!-- Tag filters -->
        <div class="filter-bar">
            <strong>Filter:</strong>
            <a href="/ideas" class="tag {all_active}">All</a>
            {tag_filter_html}
        </div>

        <!-- Status filters -->
        <div class="filter-bar">
            <a href="/ideas" class="badge badge-muted">All</a>
            <a href="/ideas?status=inbox" class="badge badge-warning">Inbox</a>
            <a href="/ideas?status=active" class="badge badge-info">Active</a>
            <a href="/ideas?status=in_progress" class="badge badge-primary">In Progress</a>
            <a href="/ideas?status=done" class="badge badge-success">Done</a>
        </div>

        {ideas_html}
    </div>
    {tz_js}
</body>
</html>"#,
        style = common_style(),
        tz_selector = timezone_selector_html(),
        total = total,
        all_active = if params.tag.is_none() { "tag-active" } else { "" },
        tag_filter_html = tag_filter_html,
        ideas_html = ideas_html,
        tz_js = timezone_js(),
    ))
}

pub async fn add_idea_handler(
    State(state): State<Arc<WebAppState>>,
    Form(form): Form<IdeaForm>,
) -> impl IntoResponse {
    let pool = &state.db.pool;
    let priority = form.priority.unwrap_or(3);

    match create_idea(
        pool,
        &form.content,
        form.tags.as_deref(),
        form.project.as_deref(),
        form.category.as_deref(),
        priority,
    )
    .await
    {
        Ok(_) => {
            info!("Created idea: {}", &form.content[..form.content.len().min(50)]);
        }
        Err(e) => {
            error!("Failed to create idea: {}", e);
        }
    }

    axum::response::Redirect::to("/ideas")
}

pub async fn idea_status_handler(
    State(state): State<Arc<WebAppState>>,
    Path((id, status)): Path<(String, String)>,
) -> impl IntoResponse {
    let pool = &state.db.pool;
    let _ = update_idea_status(pool, &id, &status).await;
    axum::response::Redirect::to("/ideas")
}

pub async fn delete_idea_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let pool = &state.db.pool;
    let _ = delete_idea(pool, &id).await;
    axum::response::Redirect::to("/ideas")
}

// ============================================================================
// Documents Page
// ============================================================================

pub async fn docs_handler(
    State(state): State<Arc<WebAppState>>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    let pool = &state.db.pool;

    let documents = if let Some(ref q) = params.q {
        search_documents(pool, q, 50).await.unwrap_or_default()
    } else {
        list_documents(
            pool,
            50,
            params.doc_type.as_deref(),
            params.tag.as_deref(),
            None,
            params.status.as_deref(),
        )
        .await
        .unwrap_or_default()
    };

    let total = count_documents(pool).await.unwrap_or(0);
    let tags = list_tags(pool, 30).await.unwrap_or_default();

    let docs_html: String = if documents.is_empty() {
        r#"<div class="empty-state"><p>No documents yet. Add your first reference doc or research note!</p></div>"#.to_string()
    } else {
        documents.iter().map(|doc| {
            let type_badge = match doc.doc_type.as_str() {
                "research" => r#"<span class="badge badge-primary">Research</span>"#,
                "reference" => r#"<span class="badge badge-info">Reference</span>"#,
                "tutorial" => r#"<span class="badge badge-success">Tutorial</span>"#,
                "architecture" => r#"<span class="badge badge-warning">Architecture</span>"#,
                "decision" => r#"<span class="badge badge-danger">Decision</span>"#,
                "snippet" => r#"<span class="badge badge-muted">Snippet</span>"#,
                _ => r#"<span class="badge badge-muted">Other</span>"#,
            };
            let pin_icon = if doc.pinned == 1 { "📌 " } else { "" };
            let tags_html = doc.tags.as_deref().map(|t| {
                t.split(',').map(|tag| format!(
                    r#"<span class="tag">{}</span>"#, tag.trim()
                )).collect::<Vec<_>>().join(" ")
            }).unwrap_or_default();
            let preview = if doc.content.len() > 200 {
                format!("{}...", &doc.content[..200])
            } else {
                doc.content.clone()
            };

            format!(r#"
            <div class="card doc-card">
                <div class="doc-header">
                    <div>
                        <h3><a href="/docs/{id}">{pin}{title}</a></h3>
                        <div class="doc-badges">{type_badge} <span class="badge badge-muted">{words} words</span></div>
                    </div>
                    <div class="doc-actions">
                        <a href="/docs/{id}/edit" class="btn-small btn-primary">Edit</a>
                        <a href="/docs/{id}/delete" class="btn-small btn-danger"
                           onclick="return confirm('Delete this document?')">✕</a>
                    </div>
                </div>
                <div class="doc-preview">{preview}</div>
                <div class="doc-tags">{tags}</div>
                <div class="doc-meta"><span data-utc="{updated}">—</span></div>
            </div>"#,
                id = doc.id,
                pin = pin_icon,
                title = doc.title,
                type_badge = type_badge,
                words = doc.word_count,
                preview = html_escape(&preview),
                tags = tags_html,
                updated = format_timestamp(doc.updated_at),
            )
        }).collect::<Vec<_>>().join("\n")
    };

    // Doc type filter tabs
    let doc_types = ["research", "reference", "tutorial", "architecture", "decision", "snippet"];
    let type_tabs: String = doc_types.iter().map(|dt| {
        let active = params.doc_type.as_deref() == Some(dt);
        let cls = if active { "badge badge-primary" } else { "badge badge-muted" };
        format!(r#"<a href="/docs?doc_type={}" class="{}">{}</a>"#, dt, cls, dt)
    }).collect::<Vec<_>>().join(" ");

    Html(format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Documents - RustAssistant</title>
    {style}
    <style>
        .doc-card {{ margin-bottom: 0.75rem; padding: 1rem; }}
        .doc-header {{ display: flex; justify-content: space-between; align-items: flex-start; }}
        .doc-header h3 {{ margin: 0; font-size: 1.1rem; }}
        .doc-header h3 a {{ color: #e2e8f0; text-decoration: none; }}
        .doc-header h3 a:hover {{ color: #0ea5e9; }}
        .doc-badges {{ display: flex; gap: 0.3rem; margin-top: 0.25rem; }}
        .doc-actions {{ display: flex; gap: 0.25rem; }}
        .doc-preview {{ font-size: 0.9rem; color: #94a3b8; margin: 0.5rem 0; line-height: 1.4;
            max-height: 3em; overflow: hidden; }}
        .doc-tags {{ display: flex; gap: 0.3rem; flex-wrap: wrap; margin-bottom: 0.3rem; }}
        .doc-meta {{ font-size: 0.8rem; color: #64748b; }}
        .search-bar {{ display: flex; gap: 0.5rem; margin-bottom: 1rem; }}
        .search-bar input {{ flex: 1; padding: 0.75rem; border: 1px solid #334155; background: #1e293b;
            color: #e2e8f0; border-radius: 6px; font-size: 1rem; }}
        .search-bar input:focus {{ outline: none; border-color: #0ea5e9; }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos">Repos</a>
                <a href="/queue">Queue</a>
                <a href="/ideas">Ideas</a>
                <a href="/docs" class="active">Docs</a>
                <a href="/activity">Activity</a>
                {tz_selector}
            </nav>
        </header>

        <h2>📚 Documents ({total})</h2>

        <!-- Search -->
        <form action="/docs" method="get" class="search-bar">
            <input type="text" name="q" placeholder="Search documents..." value="{search_q}">
            <button type="submit" class="btn btn-primary">Search</button>
            <a href="/docs/new" class="btn btn-success">+ New Document</a>
        </form>

        <!-- Type filters -->
        <div class="filter-bar" style="margin-bottom: 1rem;">
            <a href="/docs" class="badge badge-muted">All</a>
            {type_tabs}
        </div>

        {docs_html}
    </div>
    {tz_js}
</body>
</html>"#,
        style = common_style(),
        tz_selector = timezone_selector_html(),
        total = total,
        search_q = params.q.as_deref().unwrap_or(""),
        type_tabs = type_tabs,
        docs_html = docs_html,
        tz_js = timezone_js(),
    ))
}

// Document creation form
pub async fn new_doc_form_handler() -> impl IntoResponse {
    Html(format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>New Document - RustAssistant</title>
    {style}
    <style>
        .form-group {{ margin-bottom: 1rem; }}
        .form-group label {{ display: block; margin-bottom: 0.3rem; color: #94a3b8; font-weight: 600; }}
        .form-group input, .form-group select, .form-group textarea {{ width: 100%; padding: 0.75rem;
            border: 1px solid #334155; background: #1e293b; color: #e2e8f0; border-radius: 6px; font-size: 1rem; }}
        .form-group textarea {{ min-height: 400px; font-family: 'Fira Code', monospace; line-height: 1.5; }}
        .form-group input:focus, .form-group textarea:focus {{ outline: none; border-color: #0ea5e9; }}
        .form-row {{ display: flex; gap: 1rem; }}
        .form-row .form-group {{ flex: 1; }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos">Repos</a>
                <a href="/queue">Queue</a>
                <a href="/ideas">Ideas</a>
                <a href="/docs" class="active">Docs</a>
                <a href="/activity">Activity</a>
            </nav>
        </header>

        <h2>📝 New Document</h2>

        <form action="/docs/create" method="post">
            <div class="form-group">
                <label>Title</label>
                <input type="text" name="title" placeholder="Document title" required autofocus>
            </div>

            <div class="form-row">
                <div class="form-group">
                    <label>Type</label>
                    <select name="doc_type">
                        <option value="reference">Reference</option>
                        <option value="research">Research</option>
                        <option value="tutorial">Tutorial</option>
                        <option value="architecture">Architecture</option>
                        <option value="decision">Decision</option>
                        <option value="snippet">Snippet</option>
                        <option value="runbook">Runbook</option>
                        <option value="template">Template</option>
                    </select>
                </div>
                <div class="form-group">
                    <label>Tags (comma-separated)</label>
                    <input type="text" name="tags" placeholder="rust, docker, deployment">
                </div>
                <div class="form-group">
                    <label>Project (optional)</label>
                    <input type="text" name="project" placeholder="rustassistant">
                </div>
            </div>

            <div class="form-group">
                <label>Source URL (optional)</label>
                <input type="text" name="source_url" placeholder="https://...">
            </div>

            <div class="form-group">
                <label>Content (Markdown)</label>
                <textarea name="content" placeholder="Write your document content here...&#10;&#10;Supports Markdown formatting." required></textarea>
            </div>

            <div style="display: flex; gap: 1rem;">
                <button type="submit" class="btn btn-success">Save Document</button>
                <a href="/docs" class="btn btn-muted">Cancel</a>
            </div>
        </form>
    </div>
</body>
</html>"#,
        style = common_style(),
    ))
}

pub async fn create_doc_handler(
    State(state): State<Arc<WebAppState>>,
    Form(form): Form<DocumentForm>,
) -> impl IntoResponse {
    let pool = &state.db.pool;
    let doc_type = form.doc_type.as_deref().unwrap_or("reference");

    match create_document(
        pool,
        &form.title,
        &form.content,
        doc_type,
        form.tags.as_deref(),
        form.project.as_deref(),
        form.source_url.as_deref(),
        "markdown",
    )
    .await
    {
        Ok(doc) => {
            info!("Created document: {}", doc.title);
            axum::response::Redirect::to(&format!("/docs/{}", doc.id))
        }
        Err(e) => {
            error!("Failed to create document: {}", e);
            axum::response::Redirect::to("/docs")
        }
    }
}

// View a single document
pub async fn view_doc_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let pool = &state.db.pool;

    match get_document(pool, &id).await {
        Ok(doc) => {
            let tags_html = doc.tags.as_deref().map(|t| {
                t.split(',').map(|tag| format!(
                    r#"<span class="tag">{}</span>"#, tag.trim()
                )).collect::<Vec<_>>().join(" ")
            }).unwrap_or_default();

            Html(format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title} - RustAssistant</title>
    {style}
    <style>
        .doc-content {{ background: #1e293b; padding: 2rem; border-radius: 8px; line-height: 1.7;
            white-space: pre-wrap; font-family: 'Fira Code', monospace; font-size: 0.95rem; }}
        .doc-meta-bar {{ display: flex; gap: 1rem; align-items: center; margin-bottom: 1rem;
            padding: 0.75rem 1rem; background: #1e293b; border-radius: 6px; flex-wrap: wrap; }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos">Repos</a>
                <a href="/ideas">Ideas</a>
                <a href="/docs" class="active">Docs</a>
                <a href="/activity">Activity</a>
                {tz_selector}
            </nav>
        </header>

        <h2>{pin}{title}</h2>

        <div class="doc-meta-bar">
            <span class="badge badge-info">{doc_type}</span>
            {tags_html}
            <span style="color: #64748b;">{word_count} words</span>
            <span style="color: #64748b;" data-utc="{updated}">—</span>
            <div style="margin-left: auto; display: flex; gap: 0.5rem;">
                <a href="/docs/{id}/edit" class="btn-small btn-primary">Edit</a>
                <a href="/docs" class="btn-small btn-muted">← Back</a>
            </div>
        </div>

        <div class="doc-content">{content}</div>
    </div>
    {tz_js}
</body>
</html>"#,
                style = common_style(),
                tz_selector = timezone_selector_html(),
                title = html_escape(&doc.title),
                pin = if doc.pinned == 1 { "📌 " } else { "" },
                doc_type = doc.doc_type,
                tags_html = tags_html,
                word_count = doc.word_count,
                id = doc.id,
                content = html_escape(&doc.content),
                updated = format_timestamp(doc.updated_at),
                tz_js = timezone_js(),
            ))
        }
        Err(_) => Html("<h1>Document not found</h1>".to_string()),
    }
}

pub async fn delete_doc_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let _ = delete_document(&state.db.pool, &id).await;
    axum::response::Redirect::to("/docs")
}

// ============================================================================
// Activity Feed
// ============================================================================

pub async fn activity_handler(
    State(state): State<Arc<WebAppState>>,
) -> impl IntoResponse {
    let pool = &state.db.pool;
    let events = get_recent_events(pool, 100, None).await.unwrap_or_default();

    let events_html: String = if events.is_empty() {
        r#"<div class="empty-state"><p>No activity yet. Events will appear here as scans run.</p></div>"#.to_string()
    } else {
        events.iter().map(|e| {
            let level_class = match e.level.as_str() {
                "error" => "event-error",
                "warn" => "event-warn",
                _ => "event-info",
            };
            let icon = match e.event_type.as_str() {
                "scan_start" => "🔍",
                "scan_complete" => "✅",
                "scan_error" => "❌",
                "repo_cloned" => "📥",
                "repo_updated" => "🔄",
                "todo_found" => "📝",
                "issue_found" => "⚠️",
                "llm_call" => "🤖",
                "cache_hit" => "💾",
                "system" => "⚙️",
                _ => "📋",
            };
            let details_html = e.details.as_deref().map(|d| {
                format!(r#"<div class="event-details">{}</div>"#, html_escape(d))
            }).unwrap_or_default();

            format!(r#"
            <div class="event-row {level_class}">
                <span class="event-icon">{icon}</span>
                <span class="event-type">{event_type}</span>
                <span class="event-msg">{message}</span>
                {details}
                <span class="event-time" data-utc="{time}">—</span>
            </div>"#,
                level_class = level_class,
                icon = icon,
                event_type = e.event_type,
                message = html_escape(&e.message),
                details = details_html,
                time = format_timestamp(e.created_at),
            )
        }).collect::<Vec<_>>().join("\n")
    };

    Html(format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Activity - RustAssistant</title>
    {style}
    <style>
        .event-row {{ display: flex; align-items: center; gap: 0.5rem; padding: 0.5rem 0.75rem;
            border-bottom: 1px solid #1e293b; font-size: 0.9rem; }}
        .event-row:hover {{ background: #1e293b; }}
        .event-icon {{ font-size: 1.1rem; }}
        .event-type {{ color: #64748b; min-width: 100px; font-size: 0.8rem; }}
        .event-msg {{ flex: 1; }}
        .event-time {{ color: #64748b; font-size: 0.8rem; min-width: 140px; text-align: right; }}
        .event-details {{ font-size: 0.8rem; color: #94a3b8; padding-left: 1.5rem; }}
        .event-error {{ border-left: 3px solid #ef4444; }}
        .event-warn {{ border-left: 3px solid #f59e0b; }}
        .event-info {{ border-left: 3px solid transparent; }}
        .auto-refresh {{ color: #64748b; font-size: 0.8rem; }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos">Repos</a>
                <a href="/queue">Queue</a>
                <a href="/ideas">Ideas</a>
                <a href="/docs">Docs</a>
                <a href="/activity" class="active">Activity</a>
                {tz_selector}
            </nav>
        </header>

        <div style="display: flex; justify-content: space-between; align-items: center;">
            <h2>📊 Activity Feed</h2>
            <span class="auto-refresh">Auto-refreshes every 10s</span>
        </div>

        <div id="activity-feed" hx-get="/activity/feed" hx-trigger="every 10s" hx-swap="innerHTML">
            {events_html}
        </div>
    </div>
    {tz_js}
    <script src="https://unpkg.com/htmx.org@2.0.0"></script>
</body>
</html>"#,
        style = common_style(),
        tz_selector = timezone_selector_html(),
        events_html = events_html,
        tz_js = timezone_js(),
    ))
}

/// HTMX partial: just the event rows (for auto-refresh)
pub async fn activity_feed_partial(
    State(state): State<Arc<WebAppState>>,
) -> impl IntoResponse {
    let pool = &state.db.pool;
    let events = get_recent_events(pool, 100, None).await.unwrap_or_default();

    let html: String = events.iter().map(|e| {
        let level_class = match e.level.as_str() {
            "error" => "event-error",
            "warn" => "event-warn",
            _ => "event-info",
        };
        let icon = match e.event_type.as_str() {
            "scan_start" => "🔍",
            "scan_complete" => "✅",
            "scan_error" => "❌",
            "repo_cloned" => "📥",
            "repo_updated" => "🔄",
            _ => "📋",
        };
        format!(r#"
        <div class="event-row {level_class}">
            <span class="event-icon">{icon}</span>
            <span class="event-type">{event_type}</span>
            <span class="event-msg">{message}</span>
            <span class="event-time" data-utc="{time}">—</span>
        </div>"#,
            level_class = level_class,
            icon = icon,
            event_type = e.event_type,
            message = html_escape(&e.message),
            time = format_timestamp(e.created_at),
        )
    }).collect::<Vec<_>>().join("\n");

    Html(html)
}

// ============================================================================
// Repository Settings (Scan Interval Edit)
// ============================================================================

pub async fn repo_settings_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let pool = &state.db.pool;

    // Get current repo
    let repo = sqlx::query_as::<_, crate::db::Repository>(
        "SELECT * FROM repositories WHERE id = ?1",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await;

    match repo {
        Ok(Some(repo)) => {
            Html(format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Settings: {name} - RustAssistant</title>
    {style}
    <style>
        .settings-form {{ max-width: 600px; }}
        .form-group {{ margin-bottom: 1.5rem; }}
        .form-group label {{ display: block; margin-bottom: 0.3rem; color: #94a3b8; font-weight: 600; }}
        .form-group input, .form-group select {{ width: 100%; padding: 0.75rem; border: 1px solid #334155;
            background: #1e293b; color: #e2e8f0; border-radius: 6px; font-size: 1rem; }}
        .form-group input:focus {{ outline: none; border-color: #0ea5e9; }}
        .form-group .help {{ font-size: 0.8rem; color: #64748b; margin-top: 0.3rem; }}
        .interval-presets {{ display: flex; gap: 0.5rem; margin-top: 0.5rem; }}
        .interval-presets button {{ padding: 0.3rem 0.75rem; border: 1px solid #334155; background: #1e293b;
            color: #94a3b8; border-radius: 4px; cursor: pointer; font-size: 0.85rem; }}
        .interval-presets button:hover {{ border-color: #0ea5e9; color: #0ea5e9; }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 RustAssistant</h1>
            <nav>
                <a href="/dashboard">Dashboard</a>
                <a href="/repos" class="active">Repos</a>
                <a href="/queue">Queue</a>
                <a href="/ideas">Ideas</a>
                <a href="/docs">Docs</a>
                <a href="/activity">Activity</a>
            </nav>
        </header>

        <h2>⚙️ Settings: {name}</h2>

        <form action="/repos/{id}/settings" method="post" class="settings-form">
            <div class="form-group">
                <label>Scan Interval (minutes)</label>
                <input type="number" name="scan_interval_minutes" value="{interval}"
                       min="5" max="1440" step="5" id="interval-input">
                <div class="help">How often to check for changes. Min: 5 min, Max: 1440 min (24h).</div>
                <div class="interval-presets">
                    <button type="button" onclick="document.getElementById('interval-input').value=15">15m</button>
                    <button type="button" onclick="document.getElementById('interval-input').value=30">30m</button>
                    <button type="button" onclick="document.getElementById('interval-input').value=60">1h</button>
                    <button type="button" onclick="document.getElementById('interval-input').value=120">2h</button>
                    <button type="button" onclick="document.getElementById('interval-input').value=360">6h</button>
                    <button type="button" onclick="document.getElementById('interval-input').value=720">12h</button>
                    <button type="button" onclick="document.getElementById('interval-input').value=1440">24h</button>
                </div>
            </div>

            <div class="form-group">
                <label>Repository Path</label>
                <input type="text" value="{path}" disabled>
            </div>

            <div class="form-group">
                <label>Auto-Scan</label>
                <span class="badge {scan_badge}">{scan_status}</span>
            </div>

            <div style="display: flex; gap: 1rem;">
                <button type="submit" class="btn btn-success">Save Settings</button>
                <a href="/repos" class="btn btn-muted">← Back to Repos</a>
            </div>
        </form>
    </div>
</body>
</html>"#,
                style = common_style(),
                name = repo.name,
                id = repo.id,
                interval = repo.scan_interval_minutes,
                path = repo.path,
                scan_badge = if repo.auto_scan_enabled == 1 { "badge-success" } else { "badge-muted" },
                scan_status = if repo.auto_scan_enabled == 1 { "Enabled" } else { "Disabled" },
            ))
        }
        _ => Html("<h1>Repository not found</h1>".to_string()),
    }
}

pub async fn update_repo_settings_handler(
    State(state): State<Arc<WebAppState>>,
    Path(id): Path<String>,
    Form(form): Form<RepoSettingsForm>,
) -> impl IntoResponse {
    let pool = &state.db.pool;
    let now = chrono::Utc::now().timestamp();

    // Clamp interval to valid range
    let interval = form.scan_interval_minutes.max(5).min(1440);

    let _ = sqlx::query(
        "UPDATE repositories SET scan_interval_minutes = ?1, updated_at = ?2 WHERE id = ?3",
    )
    .bind(interval)
    .bind(now)
    .bind(&id)
    .execute(pool)
    .await;

    info!("Updated scan interval for {} to {} minutes", id, interval);

    axum::response::Redirect::to("/repos")
}

// ============================================================================
// Scan Progress API (for HTMX polling)
// ============================================================================

#[derive(Serialize)]
struct RepoScanStatus {
    id: String,
    name: String,
    scan_status: String,
    scan_files_done: i32,
    scan_files_total: i32,
    scan_issues_found: i32,
    scan_duration_ms: Option<i64>,
    last_scan_error: Option<String>,
    percent: f32,
}

pub async fn scan_progress_api(
    State(state): State<Arc<WebAppState>>,
) -> impl IntoResponse {
    let pool = &state.db.pool;

    let repos: Vec<RepoScanStatus> = sqlx::query_as::<_, (String, String, String, i32, i32, i32, Option<i64>, Option<String>)>(
        r#"SELECT id, name, scan_status, scan_files_done, scan_files_total,
                  scan_issues_found, scan_duration_ms, last_scan_error
           FROM repositories WHERE auto_scan_enabled = 1
           ORDER BY name"#,
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, name, status, done, total, issues, duration, error)| {
        let percent = if total > 0 { (done as f32 / total as f32) * 100.0 } else { 0.0 };
        RepoScanStatus {
            id, name, scan_status: status, scan_files_done: done,
            scan_files_total: total, scan_issues_found: issues,
            scan_duration_ms: duration, last_scan_error: error, percent,
        }
    })
    .collect();

    Json(repos)
}

/// HTMX partial: scan progress bars for repos page
pub async fn scan_progress_partial(
    State(state): State<Arc<WebAppState>>,
) -> impl IntoResponse {
    let pool = &state.db.pool;

    let rows: Vec<(String, String, String, i32, i32, i32, Option<i64>, Option<String>)> =
        sqlx::query_as(
            r#"SELECT id, name, scan_status, scan_files_done, scan_files_total,
                      scan_issues_found, scan_duration_ms, last_scan_error
               FROM repositories WHERE auto_scan_enabled = 1
               ORDER BY name"#,
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    let html: String = rows.iter().map(|(id, name, status, done, total, issues, duration, error)| {
        let percent = if *total > 0 { (*done as f32 / *total as f32) * 100.0 } else { 0.0 };

        match status.as_str() {
            "scanning" | "analyzing" | "cloning" => format!(r#"
                <div class="progress-item">
                    <span class="progress-name">{name}</span>
                    <span class="progress-status scanning">⏳ {status}</span>
                    <div class="progress-bar">
                        <div class="progress-fill" style="width: {percent:.0}%"></div>
                    </div>
                    <span class="progress-text">{done}/{total} files ({percent:.0}%)</span>
                </div>"#,
                name = name, status = status, percent = percent, done = done, total = total,
            ),
            "error" => format!(r#"
                <div class="progress-item">
                    <span class="progress-name">{name}</span>
                    <span class="progress-status error">❌ Error</span>
                    <span class="progress-error">{err}</span>
                </div>"#,
                name = name, err = error.as_deref().unwrap_or("Unknown error"),
            ),
            _ => format!(r#"
                <div class="progress-item">
                    <span class="progress-name">{name}</span>
                    <span class="progress-status idle">✅ Idle</span>
                    <span class="progress-text">{issues} issues · {duration}s last scan</span>
                </div>"#,
                name = name, issues = issues,
                duration = duration.map(|d| format!("{:.1}", d as f64 / 1000.0)).unwrap_or_else(|| "—".to_string()),
            ),
        }
    }).collect::<Vec<_>>().join("\n");

    Html(html)
}

// ============================================================================
// Enhanced Health Endpoint
// ============================================================================

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub uptime_seconds: i64,
    pub scanner: ScannerHealth,
    pub database: DatabaseHealth,
    pub cache: CacheHealth,
}

#[derive(Serialize)]
pub struct ScannerHealth {
    pub repos_tracked: i64,
    pub repos_scanning: i64,
    pub total_queue_items: i64,
    pub last_scan_event: Option<String>,
}

#[derive(Serialize)]
pub struct DatabaseHealth {
    pub total_notes: i64,
    pub total_ideas: i64,
    pub total_documents: i64,
}

#[derive(Serialize)]
pub struct CacheHealth {
    pub redis_connected: bool,
}

// ============================================================================
// Router Extension
// ============================================================================

/// Create the extension router with all new routes.
/// Merge this with the main router in server.rs:
///
/// ```rust
/// let app = web_ui::create_router(state.clone())
///     .merge(web_ui_extensions::create_extension_router(state));
/// ```
pub fn create_extension_router(state: WebAppState) -> Router {
    let shared = Arc::new(state);

    Router::new()
        // Ideas
        .route("/ideas", get(ideas_handler))
        .route("/ideas/add", post(add_idea_handler))
        .route("/ideas/:id/status/:status", get(idea_status_handler))
        .route("/ideas/:id/delete", get(delete_idea_handler))
        // Documents
        .route("/docs", get(docs_handler))
        .route("/docs/new", get(new_doc_form_handler))
        .route("/docs/create", post(create_doc_handler))
        .route("/docs/:id", get(view_doc_handler))
        .route("/docs/:id/delete", get(delete_doc_handler))
        // Activity
        .route("/activity", get(activity_handler))
        .route("/activity/feed", get(activity_feed_partial))
        // Repo settings
        .route("/repos/:id/settings", get(repo_settings_handler))
        .route("/repos/:id/settings", post(update_repo_settings_handler))
        // Scan progress (HTMX + JSON)
        .route("/api/scan/progress", get(scan_progress_api))
        .route("/scan/progress", get(scan_progress_partial))
        // Tags API (for autocomplete)
        .route("/api/tags", get(tags_api_handler))
        .with_state(shared)
}

// Tags autocomplete API
async fn tags_api_handler(
    State(state): State<Arc<WebAppState>>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    let pool = &state.db.pool;
    let tags = if let Some(prefix) = params.q.as_deref() {
        search_tags(pool, prefix).await.unwrap_or_default()
    } else {
        list_tags(pool, 50).await.unwrap_or_default()
    };
    Json(tags)
}

// ============================================================================
// Helpers
// ============================================================================

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Common CSS shared across all extension pages
fn common_style() -> &'static str {
    r#"<style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0f172a; color: #e2e8f0; line-height: 1.6; }
        .container { max-width: 1200px; margin: 0 auto; padding: 1rem 2rem; }
        header { display: flex; justify-content: space-between; align-items: center;
            padding: 1rem 0; border-bottom: 1px solid #1e293b; margin-bottom: 1.5rem; }
        header h1 { font-size: 1.3rem; color: #0ea5e9; }
        nav { display: flex; gap: 0.5rem; flex-wrap: wrap; }
        nav a { color: #94a3b8; text-decoration: none; padding: 0.4rem 0.8rem;
            border-radius: 6px; font-size: 0.9rem; }
        nav a:hover { color: #e2e8f0; background: #1e293b; }
        nav a.active { color: #0ea5e9; background: #0c2d4a; font-weight: 600; }
        h2 { font-size: 1.4rem; margin-bottom: 1rem; color: #f1f5f9; }
        .card { background: #1e293b; border-radius: 8px; border: 1px solid #334155; }
        .btn, .btn-small { padding: 0.6rem 1.2rem; border-radius: 6px; border: none; cursor: pointer;
            font-size: 0.9rem; font-weight: 500; text-decoration: none; display: inline-block;
            transition: all 0.2s; }
        .btn-small { padding: 0.3rem 0.6rem; font-size: 0.8rem; }
        .btn-primary { background: #0ea5e9; color: white; }
        .btn-primary:hover { background: #0284c7; }
        .btn-success { background: #22c55e; color: white; }
        .btn-success:hover { background: #16a34a; }
        .btn-danger { background: #ef4444; color: white; }
        .btn-danger:hover { background: #dc2626; }
        .btn-muted { background: #475569; color: #cbd5e1; }
        .btn-muted:hover { background: #64748b; }
        .badge { padding: 2px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 600; }
        .badge-danger { background: #ef4444; color: white; }
        .badge-warning { background: #f59e0b; color: white; }
        .badge-info { background: #0ea5e9; color: white; }
        .badge-primary { background: #6366f1; color: white; }
        .badge-success { background: #22c55e; color: white; }
        .badge-muted { background: #475569; color: #cbd5e1; }
        .tag { background: #1e3a5f; color: #7dd3fc; padding: 2px 8px; border-radius: 4px;
            font-size: 0.75rem; text-decoration: none; }
        .tag:hover { background: #0284c7; color: white; }
        .filter-bar { display: flex; gap: 0.5rem; flex-wrap: wrap; align-items: center; }
        .empty-state { text-align: center; padding: 3rem; color: #64748b; }

        /* Progress bars */
        .progress-item { display: flex; align-items: center; gap: 0.75rem; padding: 0.5rem 0;
            border-bottom: 1px solid #1e293b; font-size: 0.9rem; }
        .progress-name { font-weight: 600; min-width: 120px; }
        .progress-status { font-size: 0.8rem; min-width: 80px; }
        .progress-status.scanning { color: #f59e0b; }
        .progress-status.idle { color: #22c55e; }
        .progress-status.error { color: #ef4444; }
        .progress-bar { flex: 1; height: 8px; background: #334155; border-radius: 4px; overflow: hidden; }
        .progress-fill { height: 100%; background: linear-gradient(90deg, #0ea5e9, #22c55e);
            border-radius: 4px; transition: width 0.3s ease; }
        .progress-text { color: #94a3b8; font-size: 0.8rem; min-width: 140px; text-align: right; }
        .progress-error { color: #ef4444; font-size: 0.8rem; }
    </style>"#
}
