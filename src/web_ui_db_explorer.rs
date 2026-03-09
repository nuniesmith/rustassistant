// ============================================================================
// Database Explorer — Web UI Module
// ============================================================================
//
// Provides an interactive SQLite database browser for the RustAssistant web UI.
// Browse tables, view rows with pagination, inspect schemas, and run ad-hoc
// queries — all from the browser.
//
// Routes:
//   GET /db                     — Table list with row counts and schema info
//   GET /db/table/:name         — Browse rows with pagination (?page=1&limit=50)
//   GET /db/query               — Query form + results (?sql=SELECT...)
//
// Integration:
//   In src/lib.rs:
//     pub mod web_ui_db_explorer;
//
//   In src/bin/server.rs:
//     use rustassistant::web_ui_db_explorer::create_db_explorer_router;
//     let db_explorer_router = create_db_explorer_router(Arc::new(web_state.clone()));
//     let app = Router::new()
//         .merge(web_router)
//         .merge(extension_router)
//         .merge(cache_viewer_router)
//         .merge(db_explorer_router)   // <-- add this
//         .merge(api_router);
//
//   Update nav() in web_ui_cache_viewer.rs & web_ui_extensions.rs to include:
//     ("DB Explorer", "/db"),

use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::Deserialize;
use sqlx::Column;
use std::sync::Arc;

use crate::web_ui::WebAppState;

// ============================================================================
// Router
// ============================================================================

pub fn create_db_explorer_router(state: Arc<WebAppState>) -> Router {
    Router::new()
        .route("/db", get(db_overview_handler))
        .route("/db/table/{name}", get(db_table_handler))
        .route("/db/query", get(db_query_handler))
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

fn page_shell(title: &str, nav_active: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title} — RustAssistant</title>
<script src="https://unpkg.com/htmx.org@1.9.10"></script>
<style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
        background: #0f172a; color: #e2e8f0; line-height: 1.6; }}
    .container {{ max-width: 1400px; margin: 0 auto; padding: 1rem 2rem; }}
    nav {{ display: flex; gap: 0.5rem; flex-wrap: wrap; align-items: center;
        padding: 1rem 0; border-bottom: 1px solid #1e293b; margin-bottom: 1.5rem; }}
    nav a {{ color: #94a3b8; text-decoration: none; padding: 0.4rem 0.8rem;
        border-radius: 6px; font-size: 0.9rem; }}
    nav a:hover {{ color: #e2e8f0; background: #1e293b; }}
    nav a.active {{ color: #0ea5e9; background: #0c2d4a; font-weight: 600; }}
    h2 {{ font-size: 1.4rem; margin-bottom: 1rem; color: #f1f5f9; }}
    h3 {{ font-size: 1.1rem; margin-bottom: 0.75rem; color: #94a3b8; }}
    .card {{ background: #1e293b; border-radius: 8px; border: 1px solid #334155;
        padding: 1.25rem; margin-bottom: 1rem; }}
    .btn {{ padding: 0.5rem 1rem; border-radius: 6px; border: none; cursor: pointer;
        font-size: 0.85rem; font-weight: 500; text-decoration: none; display: inline-block;
        transition: all 0.2s; }}
    .btn-primary {{ background: #0ea5e9; color: white; }}
    .btn-primary:hover {{ background: #0284c7; }}
    .btn-sm {{ padding: 0.3rem 0.6rem; font-size: 0.8rem; }}

    /* Tables */
    table {{ width: 100%; border-collapse: collapse; font-size: 0.85rem; }}
    th {{ text-align: left; padding: 0.6rem 0.75rem; background: #0f172a;
        border-bottom: 2px solid #334155; font-weight: 600; color: #94a3b8;
        font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.05em;
        position: sticky; top: 0; }}
    td {{ padding: 0.5rem 0.75rem; border-bottom: 1px solid #1e293b;
        max-width: 400px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
    tr:hover {{ background: #1e293b; }}
    td.null {{ color: #475569; font-style: italic; }}

    /* Query editor */
    textarea {{ width: 100%; min-height: 120px; background: #0f172a; color: #e2e8f0;
        border: 1px solid #334155; border-radius: 6px; padding: 0.75rem;
        font-family: 'JetBrains Mono', 'Fira Code', monospace; font-size: 0.9rem;
        resize: vertical; }}
    textarea:focus {{ outline: none; border-color: #0ea5e9; }}

    /* Stats grid */
    .stats-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
        gap: 1rem; margin-bottom: 1.5rem; }}
    .stat {{ background: #1e293b; border-radius: 8px; border: 1px solid #334155;
        padding: 1rem; text-align: center; }}
    .stat-value {{ font-size: 1.8rem; font-weight: 700; color: #0ea5e9; }}
    .stat-label {{ font-size: 0.8rem; color: #94a3b8; margin-top: 0.25rem; }}

    /* Table list */
    .table-item {{ display: flex; justify-content: space-between; align-items: center;
        padding: 0.75rem 1rem; border-bottom: 1px solid #1e293b; }}
    .table-item:last-child {{ border-bottom: none; }}
    .table-item:hover {{ background: #0f172a; }}
    .table-name {{ font-weight: 600; color: #e2e8f0; }}
    .table-name a {{ color: #0ea5e9; text-decoration: none; }}
    .table-name a:hover {{ text-decoration: underline; }}
    .table-meta {{ display: flex; gap: 1rem; font-size: 0.8rem; color: #64748b; }}
    .table-badge {{ background: #0c2d4a; color: #38bdf8; padding: 0.15rem 0.5rem;
        border-radius: 4px; font-size: 0.75rem; }}
    .view-badge {{ background: #1a2e05; color: #84cc16; }}

    /* Pagination */
    .pagination {{ display: flex; gap: 0.5rem; align-items: center; margin-top: 1rem;
        justify-content: center; }}
    .pagination a {{ color: #94a3b8; text-decoration: none; padding: 0.4rem 0.8rem;
        border-radius: 4px; border: 1px solid #334155; }}
    .pagination a:hover {{ background: #1e293b; color: #e2e8f0; }}
    .pagination .current {{ background: #0ea5e9; color: white; padding: 0.4rem 0.8rem;
        border-radius: 4px; }}

    /* Schema */
    .schema-col {{ display: grid; grid-template-columns: 2fr 1.5fr 0.5fr 0.5fr 1fr;
        gap: 0.5rem; padding: 0.4rem 0; font-size: 0.85rem; border-bottom: 1px solid #1e293b; }}
    .schema-col.header {{ color: #64748b; font-weight: 600; font-size: 0.75rem;
        text-transform: uppercase; letter-spacing: 0.05em; border-bottom: 2px solid #334155; }}
    .pk {{ color: #fbbf24; }}
    .notnull {{ color: #f87171; font-size: 0.75rem; }}

    /* Breadcrumbs */
    .breadcrumb {{ font-size: 0.85rem; margin-bottom: 1rem; color: #64748b; }}
    .breadcrumb a {{ color: #0ea5e9; text-decoration: none; }}
    .breadcrumb a:hover {{ text-decoration: underline; }}
</style>
</head>
<body>
<div class="container">
    {nav}
    {content}
</div>
</body>
</html>"#,
        title = title,
        nav = nav(nav_active),
        content = content,
    )
}

// ============================================================================
// GET /db — Table Overview
// ============================================================================

pub async fn db_overview_handler(State(state): State<Arc<WebAppState>>) -> impl IntoResponse {
    let pool = &state.db.pool;

    // Get all tables and views
    let tables: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, type FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx_%' ORDER BY type, name",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Get row counts for each table
    let mut table_rows = Vec::new();
    for (name, obj_type) in &tables {
        let count: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM \"{}\"",
            name.replace('"', "\"\"")
        ))
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        // Get column count
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(&format!(
            "PRAGMA table_info(\"{}\")",
            name.replace('"', "\"\"")
        ))
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        table_rows.push((name.clone(), obj_type.clone(), count, cols.len()));
    }

    // Stats
    let total_tables = table_rows.iter().filter(|t| t.1 == "table").count();
    let total_views = table_rows.iter().filter(|t| t.1 == "view").count();
    let total_rows: i64 = table_rows.iter().map(|t| t.2).sum();

    // DB file size
    let db_size: String = {
        let page_count = sqlx::query_scalar::<_, i64>("PRAGMA page_count")
            .fetch_one(pool)
            .await
            .ok();
        let page_size = sqlx::query_scalar::<_, i64>("PRAGMA page_size")
            .fetch_one(pool)
            .await
            .ok();
        match (page_count, page_size) {
            (Some(pages), Some(size)) => {
                let bytes = pages * size;
                if bytes > 1_048_576 {
                    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
                } else {
                    format!("{:.0} KB", bytes as f64 / 1024.0)
                }
            }
            _ => "—".to_string(),
        }
    };

    let stats_html = format!(
        r#"<div class="stats-grid">
            <div class="stat">
                <div class="stat-value">{}</div>
                <div class="stat-label">Tables</div>
            </div>
            <div class="stat">
                <div class="stat-value">{}</div>
                <div class="stat-label">Views</div>
            </div>
            <div class="stat">
                <div class="stat-value">{}</div>
                <div class="stat-label">Total Rows</div>
            </div>
            <div class="stat">
                <div class="stat-value">{}</div>
                <div class="stat-label">DB Size</div>
            </div>
        </div>"#,
        total_tables, total_views, total_rows, db_size
    );

    let table_list: String = table_rows
        .iter()
        .map(|(name, obj_type, count, cols)| {
            let badge = if obj_type == "view" {
                r#"<span class="table-badge view-badge">VIEW</span>"#
            } else {
                r#"<span class="table-badge">TABLE</span>"#
            };
            format!(
                r#"<div class="table-item">
                    <div>
                        <span class="table-name"><a href="/db/table/{name}">{name}</a></span>
                        {badge}
                    </div>
                    <div class="table-meta">
                        <span>{cols} columns</span>
                        <span>{count} rows</span>
                    </div>
                </div>"#,
                name = html_escape(name),
                badge = badge,
                cols = cols,
                count = count,
            )
        })
        .collect();

    let content = format!(
        r#"<h2>🗄️ Database Explorer</h2>
        {stats}
        <div class="card">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem;">
                <h3 style="margin: 0;">Tables & Views</h3>
                <a href="/db/query" class="btn btn-primary btn-sm">🔍 Run Query</a>
            </div>
            {tables}
        </div>"#,
        stats = stats_html,
        tables = table_list,
    );

    Html(page_shell("DB Explorer", "DB Explorer", &content))
}

// ============================================================================
// GET /db/table/:name — Browse Table Rows
// ============================================================================

#[derive(Deserialize)]
pub struct TableQuery {
    page: Option<i64>,
    limit: Option<i64>,
    sort: Option<String>,
    dir: Option<String>,
}

pub async fn db_table_handler(
    State(state): State<Arc<WebAppState>>,
    Path(table_name): Path<String>,
    Query(params): Query<TableQuery>,
) -> impl IntoResponse {
    let pool = &state.db.pool;
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(50).min(500);
    let offset = (page - 1) * limit;

    let safe_name = table_name.replace('"', "\"\"");

    // Get schema
    let columns: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as(&format!("PRAGMA table_info(\"{}\")", safe_name))
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    if columns.is_empty() {
        return Html(page_shell(
            "Not Found",
            "DB Explorer",
            &format!(
                r#"<div class="breadcrumb"><a href="/db">DB Explorer</a> / {}</div>
                <div class="card"><p>Table not found: {}</p></div>"#,
                html_escape(&table_name),
                html_escape(&table_name)
            ),
        ));
    }

    // Get total count
    let total: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM \"{}\"", safe_name))
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    // Determine sort column
    let sort_col = params
        .sort
        .as_deref()
        .filter(|s| columns.iter().any(|c| c.1 == **s))
        .unwrap_or_else(|| &columns[0].1);
    let sort_dir = params.dir.as_deref().unwrap_or("ASC");
    let sort_dir_safe = if sort_dir.eq_ignore_ascii_case("DESC") {
        "DESC"
    } else {
        "ASC"
    };
    let next_dir = if sort_dir_safe == "ASC" {
        "DESC"
    } else {
        "ASC"
    };

    // Fetch rows
    let query = format!(
        "SELECT * FROM \"{}\" ORDER BY \"{}\" {} LIMIT {} OFFSET {}",
        safe_name,
        sort_col.replace('"', "\"\""),
        sort_dir_safe,
        limit,
        offset,
    );

    let rows: Vec<sqlx::postgres::PgRow> = sqlx::query(&query)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    // Build schema card
    let schema_html: String = columns
        .iter()
        .map(|(_, col_name, col_type, notnull, default, pk)| {
            let pk_icon = if *pk > 0 { "🔑" } else { "" };
            let nn = if *notnull > 0 {
                r#"<span class="notnull">NOT NULL</span>"#
            } else {
                ""
            };
            let def = default
                .as_deref()
                .map(|d| format!("<span style='color:#64748b;'>= {}</span>", html_escape(d)))
                .unwrap_or_default();
            format!(
                r#"<div class="schema-col">
                    <span>{pk} {name}</span>
                    <span style="color:#38bdf8;">{col_type}</span>
                    <span>{nn}</span>
                    <span></span>
                    <span>{def}</span>
                </div>"#,
                pk = pk_icon,
                name = html_escape(col_name),
                col_type = html_escape(col_type),
                nn = nn,
                def = def,
            )
        })
        .collect();

    // Build data table headers with sort links
    let headers: String = columns
        .iter()
        .map(|(_, col_name, _, _, _, _)| {
            let arrow = if col_name == sort_col {
                if sort_dir_safe == "ASC" {
                    " ▲"
                } else {
                    " ▼"
                }
            } else {
                ""
            };
            format!(
                r#"<th><a href="/db/table/{table}?sort={col}&dir={dir}&limit={limit}" style="color:inherit;text-decoration:none;">{col}{arrow}</a></th>"#,
                table = html_escape(&table_name),
                col = html_escape(col_name),
                dir = if col_name == sort_col { next_dir } else { "ASC" },
                limit = limit,
                arrow = arrow,
            )
        })
        .collect();

    // Build data rows
    use sqlx::Row;
    let data_rows: String = rows
        .iter()
        .map(|row| {
            let cells: String = columns
                .iter()
                .enumerate()
                .map(|(i, (_, _col_name, _col_type, _, _, _))| {
                    // Try to get value as string
                    let val: Option<String> = row
                        .try_get::<Option<String>, _>(i)
                        .ok()
                        .flatten()
                        .or_else(|| {
                            row.try_get::<Option<i64>, _>(i)
                                .ok()
                                .flatten()
                                .map(|v| v.to_string())
                        })
                        .or_else(|| {
                            row.try_get::<Option<f64>, _>(i)
                                .ok()
                                .flatten()
                                .map(|v| format!("{:.4}", v))
                        })
                        .or_else(|| {
                            row.try_get::<Option<bool>, _>(i)
                                .ok()
                                .flatten()
                                .map(|v| v.to_string())
                        });

                    match val {
                        Some(v) => {
                            let display = if v.len() > 120 {
                                format!("{}…", html_escape(&v[..120]))
                            } else {
                                html_escape(&v)
                            };
                            format!("<td title=\"{}\">{}</td>", html_escape(&v), display)
                        }
                        None => r#"<td class="null">NULL</td>"#.to_string(),
                    }
                })
                .collect();
            format!("<tr>{}</tr>", cells)
        })
        .collect();

    // Pagination
    let total_pages = (total as f64 / limit as f64).ceil() as i64;
    let pagination = if total_pages > 1 {
        let mut pag = String::from(r#"<div class="pagination">"#);
        if page > 1 {
            pag.push_str(&format!(
                r#"<a href="/db/table/{table}?page={prev}&limit={limit}&sort={sort}&dir={dir}">← Prev</a>"#,
                table = html_escape(&table_name),
                prev = page - 1,
                limit = limit,
                sort = html_escape(sort_col),
                dir = sort_dir_safe,
            ));
        }
        // Show page numbers (max 7 around current)
        let start = (page - 3).max(1);
        let end = (page + 3).min(total_pages);
        for p in start..=end {
            if p == page {
                pag.push_str(&format!(r#"<span class="current">{}</span>"#, p));
            } else {
                pag.push_str(&format!(
                    r#"<a href="/db/table/{table}?page={p}&limit={limit}&sort={sort}&dir={dir}">{p}</a>"#,
                    table = html_escape(&table_name),
                    p = p,
                    limit = limit,
                    sort = html_escape(sort_col),
                    dir = sort_dir_safe,
                ));
            }
        }
        if page < total_pages {
            pag.push_str(&format!(
                r#"<a href="/db/table/{table}?page={next}&limit={limit}&sort={sort}&dir={dir}">Next →</a>"#,
                table = html_escape(&table_name),
                next = page + 1,
                limit = limit,
                sort = html_escape(sort_col),
                dir = sort_dir_safe,
            ));
        }
        pag.push_str("</div>");
        pag
    } else {
        String::new()
    };

    let showing = format!(
        "Showing {}-{} of {}",
        offset + 1,
        (offset + limit).min(total),
        total
    );

    let content = format!(
        r#"<div class="breadcrumb"><a href="/db">DB Explorer</a> / <strong>{table}</strong></div>
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
            <h2 style="margin:0;">📋 {table}</h2>
            <div style="display: flex; gap: 0.5rem;">
                <a href="/db/query$1sql=SELECT+*+FROM+%22{table}%22+LIMIT+100" class="btn btn-primary btn-sm">🔍 Query</a>
            </div>
        </div>

        <details class="card" style="cursor:pointer;">
            <summary style="font-weight:600; padding: 0.5rem 0;">Schema ({col_count} columns)</summary>
            <div style="margin-top: 0.75rem;">
                <div class="schema-col header">
                    <span>Column</span><span>Type</span><span>Null</span><span></span><span>Default</span>
                </div>
                {schema}
            </div>
        </details>

        <div class="card" style="overflow-x: auto;">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem;">
                <span style="color: #64748b; font-size: 0.85rem;">{showing}</span>
            </div>
            <table>
                <thead><tr>{headers}</tr></thead>
                <tbody>{data_rows}</tbody>
            </table>
            {pagination}
        </div>"#,
        table = html_escape(&table_name),
        col_count = columns.len(),
        schema = schema_html,
        headers = headers,
        data_rows = data_rows,
        showing = showing,
        pagination = pagination,
    );

    Html(page_shell(
        &format!("{} — DB Explorer", table_name),
        "DB Explorer",
        &content,
    ))
}

// ============================================================================
// GET /db/query — Ad-Hoc Query Runner
// ============================================================================

#[derive(Deserialize)]
pub struct QueryParams {
    sql: Option<String>,
}

pub async fn db_query_handler(
    State(state): State<Arc<WebAppState>>,
    Query(params): Query<QueryParams>,
) -> impl IntoResponse {
    let pool = &state.db.pool;
    let sql = params.sql.as_deref().unwrap_or("").trim();

    let mut results_html = String::new();

    if !sql.is_empty() {
        // Safety: only allow SELECT, PRAGMA, EXPLAIN, and WITH (read-only)
        let sql_upper = sql.to_uppercase();
        let first_keyword = sql_upper.split_whitespace().next().unwrap_or("");
        let allowed = matches!(first_keyword, "SELECT" | "PRAGMA" | "EXPLAIN" | "WITH");

        if !allowed {
            results_html = r#"<div class="card" style="border-color: #ef4444;">
                    <p style="color: #f87171;">⛔ Only SELECT, PRAGMA, EXPLAIN, and WITH queries are allowed.</p>
                    <p style="color: #64748b; font-size: 0.85rem; margin-top: 0.5rem;">
                        This is a read-only explorer. Use the CLI for write operations.</p>
                </div>"#
                .to_string();
        } else {
            match sqlx::query(sql).fetch_all(pool).await {
                Ok(rows) => {
                    if rows.is_empty() {
                        results_html = r#"<div class="card"><p style="color:#64748b;">Query returned 0 rows.</p></div>"#.to_string();
                    } else {
                        use sqlx::Row;
                        // Get column info from first row
                        let col_count = rows[0].len();
                        let col_names: Vec<String> = (0..col_count)
                            .map(|i| rows[0].column(i).name().to_string())
                            .collect();

                        let headers: String = col_names
                            .iter()
                            .map(|n| format!("<th>{}</th>", html_escape(n)))
                            .collect();

                        let data: String = rows
                            .iter()
                            .take(1000) // Hard cap for safety
                            .map(|row| {
                                let cells: String = (0..col_count)
                                    .map(|i| {
                                        let val: Option<String> = row
                                            .try_get::<Option<String>, _>(i)
                                            .ok()
                                            .flatten()
                                            .or_else(|| {
                                                row.try_get::<Option<i64>, _>(i)
                                                    .ok()
                                                    .flatten()
                                                    .map(|v| v.to_string())
                                            })
                                            .or_else(|| {
                                                row.try_get::<Option<f64>, _>(i)
                                                    .ok()
                                                    .flatten()
                                                    .map(|v| format!("{:.4}", v))
                                            });
                                        match val {
                                            Some(v) => {
                                                let d = if v.len() > 200 {
                                                    format!("{}…", html_escape(&v[..200]))
                                                } else {
                                                    html_escape(&v)
                                                };
                                                format!("<td>{}</td>", d)
                                            }
                                            None => r#"<td class="null">NULL</td>"#.to_string(),
                                        }
                                    })
                                    .collect();
                                format!("<tr>{}</tr>", cells)
                            })
                            .collect();

                        let row_count = rows.len();
                        let capped = if row_count > 1000 {
                            " (showing first 1000)"
                        } else {
                            ""
                        };

                        results_html = format!(
                            r#"<div class="card" style="overflow-x: auto;">
                                <div style="margin-bottom: 0.75rem; color: #22c55e; font-size: 0.85rem;">
                                    ✅ {row_count} row{s} returned{capped}
                                </div>
                                <table>
                                    <thead><tr>{headers}</tr></thead>
                                    <tbody>{data}</tbody>
                                </table>
                            </div>"#,
                            row_count = row_count,
                            s = if row_count == 1 { "" } else { "s" },
                            capped = capped,
                            headers = headers,
                            data = data,
                        );
                    }
                }
                Err(e) => {
                    results_html = format!(
                        r#"<div class="card" style="border-color: #ef4444;">
                            <p style="color: #f87171;">❌ Query error:</p>
                            <pre style="color: #fca5a5; margin-top: 0.5rem; font-size: 0.85rem; white-space: pre-wrap;">{}</pre>
                        </div>"#,
                        html_escape(&e.to_string())
                    );
                }
            }
        }
    }

    // Common quick queries
    let quick_queries = [
        ("Tables & sizes", "SELECT name, type FROM sqlite_master WHERE type IN ('table','view') ORDER BY type, name"),
        ("Repositories", "SELECT id, name, auto_scan, scan_status, scan_files_processed, scan_files_total FROM repositories"),
        ("Recent scan events", "SELECT * FROM scan_events ORDER BY created_at DESC LIMIT 20"),
        ("Tasks", "SELECT id, COALESCE(title, content, 'Untitled') as title, priority, status, COALESCE(source, source_type, 'unknown') as source, COALESCE(repo_id, source_repo) as repo_id, substr(COALESCE(description, context, ''),1,80) as description_preview FROM tasks ORDER BY priority ASC, created_at DESC LIMIT 50"),
        ("LLM cost log", "SELECT model, SUM(cost) as total_cost, COUNT(*) as calls, SUM(tokens_used) as total_tokens FROM llm_usage GROUP BY model"),
        ("Notes", "SELECT id, substr(content,1,100) as preview, status, created_at FROM notes ORDER BY created_at DESC LIMIT 20"),
    ];

    let quick_html: String = quick_queries
        .iter()
        .map(|(label, q)| {
            format!(
                r#"<a href="/db/query?sql={}" class="btn btn-sm" style="background:#334155;color:#e2e8f0;margin:0.25rem;">{}</a>"#,
                urlencoding_simple(q),
                label,
            )
        })
        .collect();

    let content = format!(
        r#"<div class="breadcrumb"><a href="/db">DB Explorer</a> / <strong>Query</strong></div>
        <h2 style="margin-bottom: 1rem;">🔍 SQL Query</h2>

        <div class="card">
            <form method="GET" action="/db/query">
                <textarea name="sql" placeholder="SELECT * FROM repositories LIMIT 10;">{sql_val}</textarea>
                <div style="display: flex; justify-content: space-between; align-items: center; margin-top: 0.75rem;">
                    <button type="submit" class="btn btn-primary">▶ Run Query</button>
                    <span style="color: #64748b; font-size: 0.8rem;">Read-only: SELECT, PRAGMA, EXPLAIN, WITH</span>
                </div>
            </form>
        </div>

        <div style="margin-bottom: 1rem;">
            <h3 style="margin-bottom: 0.5rem;">Quick Queries</h3>
            <div style="display: flex; flex-wrap: wrap; gap: 0.25rem;">
                {quick}
            </div>
        </div>

        {results}"#,
        sql_val = html_escape(sql),
        quick = quick_html,
        results = results_html,
    );

    Html(page_shell(
        "SQL Query — DB Explorer",
        "DB Explorer",
        &content,
    ))
}

// Simple URL encoding for query strings
fn urlencoding_simple(s: &str) -> String {
    s.replace('%', "%25")
        .replace('+', "%2B")
        .replace(' ', "+")
        .replace('"', "%22")
        .replace('\'', "%27")
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace('#', "%23")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('*', "%2A")
        .replace('\n', "%0A")
}
