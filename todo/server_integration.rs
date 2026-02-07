// ============================================================================
// Integration for src/bin/server.rs
// ============================================================================
// Add this to your server.rs to merge the new extension routes with the
// existing web_ui router.
//
// Replace your current router creation with:

use rustassistant::web_ui::{create_router, WebAppState};
use rustassistant::web_ui_extensions::create_extension_router;

// In your main() or server setup function:
async fn build_app(db: Database) -> Router {
    let state = WebAppState { db: db.clone() };

    // Merge base web UI routes with extension routes
    let app = create_router(state.clone())
        .merge(create_extension_router(state));

    app
}

// ============================================================================
// Updated nav: Add to existing web_ui.rs pages
// ============================================================================
// In your existing dashboard_handler, repos_handler, queue_handler, etc.
// update the <nav> section to include the new pages:
//
// <nav>
//     <a href="/dashboard">Dashboard</a>
//     <a href="/repos">Repos</a>
//     <a href="/queue">Queue</a>
//     <a href="/ideas">Ideas</a>
//     <a href="/docs">Docs</a>
//     <a href="/activity">Activity</a>
//     {tz_selector}
// </nav>
//
// Also add to existing repo cards in repos_handler:
//     <a href="/repos/{id}/settings" class="btn-small btn-primary">⚙️ Settings</a>
//
// And add scan progress section to repos page:
//     <div id="scan-progress" hx-get="/scan/progress" hx-trigger="every 5s" hx-swap="innerHTML">
//         Loading scan status...
//     </div>

// ============================================================================
// Dashboard enhancements
// ============================================================================
// Add these stats to your dashboard_handler by querying the new tables:
//
// let idea_count = count_ideas(&state.db.pool).await.unwrap_or(0);
// let doc_count = count_documents(&state.db.pool).await.unwrap_or(0);
// let recent_events = get_recent_events(&state.db.pool, 10, None).await.unwrap_or_default();
//
// Then add stat cards and activity feed to the dashboard HTML.

// ============================================================================
// Scanner integration
// ============================================================================
// In your auto_scanner module, use the scan_events functions to log activity:
//
// use crate::db::scan_events::{
//     mark_scan_started, update_scan_file_progress, mark_scan_complete, mark_scan_error
// };
//
// Before scanning:
//     mark_scan_started(&pool, &repo.id, total_files).await?;
//
// During file processing:
//     update_scan_file_progress(&pool, &repo.id, &file_path, i, total_files).await?;
//
// After scanning:
//     mark_scan_complete(&pool, &repo.id, files_scanned, issues_found, duration_ms).await?;
//
// On error:
//     mark_scan_error(&pool, &repo.id, &error_msg).await?;

// ============================================================================
// Repo Manager integration (replacing volume mounts)
// ============================================================================
// In your scanner, instead of reading repos from volume-mounted paths,
// use repo_manager to clone/update:
//
// use crate::repo_manager::{ensure_repo, repo_local_path};
//
// // When adding a repo via web UI, store git_url instead of local path:
// let git_url = format!("https://github.com/{}/{}.git", github_user, repo_name);
// add_repository(&pool, &git_url, &repo_name, Some(&git_url)).await?;
//
// // In scanner, ensure repo is cloned/updated before scanning:
// let sync = ensure_repo(&pool, &repo.id, &repo.git_url, &repo.name).await?;
// let scan_path = sync.local_path; // Use this for scanning
//
// // If the repo was freshly cloned, scan everything.
// // If updated, optionally use get_changed_files() to scan only changed files.
