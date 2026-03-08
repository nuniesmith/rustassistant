//! Audit endpoint — Axum handlers for `GET /api/audit` and `POST /api/audit`
//!
//! # Routes
//!
//! | Method | Path              | Description                                      |
//! |--------|-------------------|--------------------------------------------------|
//! | GET    | `/api/audit`      | List recent audit reports stored in `docs/audit/`|
//! | POST   | `/api/audit`      | Trigger a new audit run for a given repo path    |
//! | GET    | `/api/audit/:id`  | Fetch a specific audit report by ID              |
//!
//! # Integration notes
//!
//! - Wired into `src/server.rs` via `audit_router()`.
//! - Delegates to `src/audit/runner.rs` (`AuditRunner`) for the actual work.
//! - Uses `src/audit/cache.rs` to skip re-auditing unchanged files.
//! - On completion, findings are appended to the target repo's `todo.md`
//!   via `TodoFile::append_item` (see `src/todo/todo_file.rs`).
//!
//! # TODO(scaffolder): implement

use axum::{
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::audit::types::{AuditRequest, AuditStatus};

// ============================================================================
// Router
// ============================================================================

/// Build the `/api/audit` sub-router.
///
/// Mount this inside `src/server.rs` with:
/// ```rust,ignore
/// app.nest("/api", audit_router(state))
/// ```
pub fn audit_router<S>(_state: Arc<S>) -> Router
where
    S: Send + Sync + 'static,
{
    // TODO(scaffolder): thread shared state (db pool, redis, grok client)
    // through AuditState and pass it to handlers via .with_state(...)
    Router::new()
        .route("/audit", get(handle_audit_get))
        .route("/audit", post(handle_audit_post))
        .route("/audit/:id", get(handle_audit_get_by_id))
}

// ============================================================================
// Handlers
// ============================================================================

/// `GET /api/audit`
///
/// Returns a list of recent audit reports from `docs/audit/`.
/// Reports are ordered by creation time, newest first.
///
/// # Response
///
/// ```json
/// {
///   "reports": [
///     {
///       "id": "20240101-120000-abc123",
///       "repo": "nuniesmith/rustassistant",
///       "created_at": "2024-01-01T12:00:00Z",
///       "status": "completed",
///       "findings_count": 12,
///       "report_path": "docs/audit/20240101-120000-abc123.md"
///     }
///   ],
///   "total": 1
/// }
/// ```
pub async fn handle_audit_get() -> impl IntoResponse {
    // TODO(scaffolder): implement
    // 1. Read `docs/audit/` directory listing
    // 2. Parse report metadata from each `.json` sidecar
    // 3. Sort by created_at descending, apply limit/offset query params
    // 4. Return paginated list
    (
        StatusCode::OK,
        Json(AuditListResponse {
            reports: vec![],
            total: 0,
        }),
    )
}

/// `POST /api/audit`
///
/// Triggers a new audit run for the specified repository path (local) or
/// GitHub URL (clones to a temp directory).
///
/// # Request body
///
/// ```json
/// {
///   "repo_path": "/path/to/local/repo",
///   "mode": "full",
///   "output_dir": "docs/audit",
///   "update_todo_md": true
/// }
/// ```
///
/// # Response (202 Accepted — audit runs asynchronously)
///
/// ```json
/// {
///   "audit_id": "20240101-120000-abc123",
///   "status": "running",
///   "message": "Audit started. Poll GET /api/audit/20240101-120000-abc123 for status."
/// }
/// ```
pub async fn handle_audit_post(Json(req): Json<AuditRequest>) -> impl IntoResponse {
    // TODO(scaffolder): implement
    // 1. Validate req.repo exists (or clone if GitHub slug)
    // 2. Check AuditCache — if all file hashes match, return cached report
    // 3. Spawn tokio task: AuditRunner::run(config).await
    // 4. Write results to docs/audit/<id>.{md,json}
    // 5. If req.append_to_todo, call TodoFile::append_item for each finding
    // 6. Return 202 with audit_id and polling URL

    let _ = req; // suppress unused warning until implemented

    (
        StatusCode::ACCEPTED,
        Json(AuditJobAccepted {
            audit_id: "not-yet-implemented".to_string(),
            status: AuditStatus::Running,
            message: "Audit endpoint not yet implemented — see src/audit/endpoint.rs".to_string(),
        }),
    )
}

/// `GET /api/audit/:id`
///
/// Fetch a specific audit report by its ID.
///
/// # Response (200 OK)
///
/// Returns the full `AuditReport` JSON for the given ID.
///
/// # Response (404 Not Found)
///
/// ```json
/// { "error": "Audit report 'xyz' not found" }
/// ```
pub async fn handle_audit_get_by_id(Path(audit_id): Path<String>) -> impl IntoResponse {
    // TODO(scaffolder): implement
    // 1. Look up docs/audit/<audit_id>.json
    // 2. Deserialise into AuditReport
    // 3. Return 200 with full report or 404 if not found

    let _ = audit_id; // suppress unused warning until implemented

    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "Audit report lookup not yet implemented — see src/audit/endpoint.rs"
                .to_string(),
        }),
    )
}

// ============================================================================
// Response types local to this module
// ============================================================================

/// Summary entry for a single audit report (used in list response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReportSummary {
    /// Unique audit ID (timestamp + short hash)
    pub id: String,
    /// Repository that was audited
    pub repo: String,
    /// ISO-8601 creation timestamp
    pub created_at: String,
    /// Current status
    pub status: AuditStatus,
    /// Total number of findings
    pub findings_count: usize,
    /// Relative path to the Markdown report
    pub report_path: String,
}

/// Response body for `GET /api/audit`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditListResponse {
    pub reports: Vec<AuditReportSummary>,
    pub total: usize,
}

/// Response body for `POST /api/audit` (202 Accepted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditJobAccepted {
    /// Unique audit run ID for polling
    pub audit_id: String,
    /// Current status (will be `running`)
    pub status: AuditStatus,
    /// Human-readable message with polling instructions
    pub message: String,
}

/// Generic error response
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ErrorResponse {
    pub error: String,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handle_audit_get_returns_200() {
        let response = handle_audit_get().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_handle_audit_post_returns_202() {
        let req = AuditRequest {
            repo: "/tmp/test-repo".to_string(),
            ..AuditRequest::default()
        };
        let response = handle_audit_post(Json(req)).await.into_response();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn test_handle_audit_get_by_id_returns_404() {
        let response = handle_audit_get_by_id(Path("nonexistent-id".to_string()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_audit_list_response_serialises() {
        let resp = AuditListResponse {
            reports: vec![AuditReportSummary {
                id: "20240101-abc123".to_string(),
                repo: "nuniesmith/rustassistant".to_string(),
                created_at: "2024-01-01T12:00:00Z".to_string(),
                status: AuditStatus::Completed,
                findings_count: 5,
                report_path: "docs/audit/20240101-abc123.md".to_string(),
            }],
            total: 1,
        };

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("nuniesmith/rustassistant"));
        assert!(json.contains("20240101-abc123"));
        assert!(json.contains("\"total\":1"));
    }

    #[test]
    fn test_audit_job_accepted_serialises() {
        let resp = AuditJobAccepted {
            audit_id: "run-001".to_string(),
            status: AuditStatus::Running,
            message: "Audit started".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("run-001"));
        assert!(json.contains("running"));
    }

    #[test]
    fn test_audit_report_summary_deserialises() {
        let json = r#"{
            "id": "abc",
            "repo": "org/repo",
            "created_at": "2024-01-01T00:00:00Z",
            "status": "completed",
            "findings_count": 3,
            "report_path": "docs/audit/abc.md"
        }"#;

        let summary: AuditReportSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.id, "abc");
        assert_eq!(summary.findings_count, 3);
    }
}
