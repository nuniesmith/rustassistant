// ============================================================================
// Required changes to existing src/web_ui.rs
// ============================================================================
// These are the modifications needed in the existing file.
// Apply these with str_replace or manual edits.

// 1. Make timezone functions public (so web_ui_extensions can use them)
//    Change:  fn timezone_js()
//    To:      pub fn timezone_js()
//
//    Change:  fn timezone_selector_html()
//    To:      pub fn timezone_selector_html()

// 2. Make WebAppState public (if not already)
//    It should already be pub based on the code, but verify:
//    pub struct WebAppState { pub db: Database }

// 3. Update nav in ALL existing handlers to add Ideas, Docs, Activity links.
//    In every handler (dashboard_handler, repos_handler, queue_handler, scanner_handler),
//    update the <nav> block:
//
//    FROM:
//      <nav>
//          <a href="/dashboard">Dashboard</a>
//          <a href="/repos">Repositories</a>
//          <a href="/queue">Queue</a>
//          <a href="/scanner">Auto-Scanner</a>
//          {tz_selector}
//      </nav>
//
//    TO:
//      <nav>
//          <a href="/dashboard">Dashboard</a>
//          <a href="/repos">Repos</a>
//          <a href="/queue">Queue</a>
//          <a href="/scanner">Scanner</a>
//          <a href="/ideas">Ideas</a>
//          <a href="/docs">Docs</a>
//          <a href="/activity">Activity</a>
//          {tz_selector}
//      </nav>

// 4. Add settings button to each repo card in repos_handler.
//    After the existing toggle-scan and delete buttons, add:
//      <a href="/repos/{id}/settings" class="btn-small btn-primary">⚙️</a>

// 5. Add scan progress section to repos_handler.
//    After the repo cards list, add:
//
//    <h3>📊 Scan Progress</h3>
//    <div id="scan-progress" hx-get="/scan/progress" hx-trigger="every 5s" hx-swap="innerHTML">
//        Loading scan status...
//    </div>
//    <script src="https://unpkg.com/htmx.org@2.0.0"></script>

// 6. Add ideas/docs counts to dashboard stats.
//    In dashboard_handler, add queries:
//
//    use crate::db::documents::{count_ideas, count_documents};
//    let idea_count = count_ideas(&state.db.pool).await.unwrap_or(0);
//    let doc_count = count_documents(&state.db.pool).await.unwrap_or(0);
//
//    Then add stat cards to the dashboard HTML for ideas and docs.
