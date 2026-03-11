//! Integration Tests for RAG API
//!
//! Comprehensive end-to-end tests for the RAG API system including:
//! - Document upload and management
//! - Search functionality (semantic, keyword, hybrid)
//! - Background indexing jobs
//! - Authentication and rate limiting
//! - Webhook delivery
//! - Cache behavior

use reqwest::StatusCode;
use rustassistant::api::{
    ApiConfig, ApiResponse, SearchRequest, SearchType, UploadDocumentRequest,
};
use serde_json::Value;
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// Test Setup
// ============================================================================

/// Setup test database and API
///
/// Requires a running Postgres instance. Set TEST_DATABASE_URL or DATABASE_URL
/// in the environment before running integration tests:
///
///   TEST_DATABASE_URL=postgresql://rustassistant:changeme@localhost:5432/rustassistant_test
async fn setup_test_env() -> (PgPool, String) {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgresql://rustassistant:changeme@localhost:5432/rustassistant_test".to_string()
        });

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test Postgres database. Set TEST_DATABASE_URL.");

    // Run migrations to ensure schema is up to date
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Generate test API key
    let api_key = "test_api_key_12345".to_string();

    (pool, api_key)
}

/// Create test server with a permissive rate limit (1 000 req / 60 s).
///
/// Most tests only make a handful of requests; this limit is high enough that
/// normal test traffic never triggers a 429.  Tests that specifically need to
/// exercise rate-limiting should call `create_test_server_strict` instead.
async fn create_test_server(pool: PgPool, api_key: String) -> String {
    create_test_server_with_rate_limit(pool, api_key, 1000, 60).await
}

/// Create test server with a tight rate limit (20 req / 60 s) for use by
/// `test_rate_limiting` only.
async fn create_test_server_strict(pool: PgPool, api_key: String) -> String {
    create_test_server_with_rate_limit(pool, api_key, 20, 60).await
}

/// Internal helper — spins up an Axum server with the given rate-limit
/// parameters and returns its base URL.
async fn create_test_server_with_rate_limit(
    pool: PgPool,
    api_key: String,
    max_requests: u32,
    window_seconds: u64,
) -> String {
    use std::net::TcpListener;
    use tokio::net::TcpListener as TokioTcpListener;

    // allow_anonymous_read so unauthenticated GETs to /health etc. return 200.
    let config = ApiConfig::development()
        .with_api_key(api_key)
        .with_rate_limit(max_requests, window_seconds)
        .allow_anonymous_read();
    let api_router = config.build_router(pool).await;

    // Nest under /api to match every test URL: "{base_url}/api/..."
    use axum::Router;
    let app = Router::new().nest("/api", api_router);

    // Find available port using a std listener, then drop it so tokio can bind
    let std_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
    let addr = std_listener.local_addr().unwrap();
    let url = format!("http://{}", addr);
    drop(std_listener);

    let listener = TokioTcpListener::bind(addr)
        .await
        .expect("Failed to bind tokio listener");

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .unwrap();
    });

    // Give server time to start
    sleep(Duration::from_millis(100)).await;

    url
}

// ============================================================================
// Document Management Tests
// ============================================================================

#[tokio::test]
async fn test_upload_document() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key.clone()).await;

    let client = reqwest::Client::new();

    let upload_req = UploadDocumentRequest {
        title: "Test Document".to_string(),
        content: "This is a test document about Rust programming.".to_string(),
        doc_type: "markdown".to_string(),
        tags: vec!["rust".to_string(), "test".to_string()],
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let response = client
        .post(format!("{}/api/documents", base_url))
        .header("X-API-Key", &api_key)
        .json(&upload_req)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::CREATED);

    let body: ApiResponse<Value> = response.json().await.expect("Failed to parse response");
    assert!(body.success);
    assert!(body.data.is_some());

    let data = body.data.unwrap();
    assert!(data["id"].as_str().is_some());
    assert_eq!(data["title"], "Test Document");
}

#[tokio::test]
async fn test_list_documents() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key.clone()).await;

    let client = reqwest::Client::new();

    // Upload multiple documents
    for i in 1..=3 {
        let upload_req = UploadDocumentRequest {
            title: format!("Document {}", i),
            content: format!("Content {}", i),
            doc_type: "markdown".to_string(),
            tags: vec![],
            repo_id: None,
            source_type: None,
            source_url: None,
        };

        client
            .post(format!("{}/api/documents", base_url))
            .header("X-API-Key", &api_key)
            .json(&upload_req)
            .send()
            .await
            .expect("Failed to upload document");
    }

    // List documents
    let response = client
        .get(format!("{}/api/documents?limit=10", base_url))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<Value> = response.json().await.expect("Failed to parse response");
    assert!(body.success);
}

#[tokio::test]
async fn test_get_document_by_id() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key.clone()).await;

    let client = reqwest::Client::new();

    // Upload document
    let upload_req = UploadDocumentRequest {
        title: "Test Document".to_string(),
        content: "Test content".to_string(),
        doc_type: "markdown".to_string(),
        tags: vec![],
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let upload_response = client
        .post(format!("{}/api/documents", base_url))
        .header("X-API-Key", &api_key)
        .json(&upload_req)
        .send()
        .await
        .expect("Failed to upload");

    assert_eq!(
        upload_response.status(),
        StatusCode::CREATED,
        "Upload must succeed before we can test GET by id"
    );

    let upload_body: ApiResponse<Value> = upload_response.json().await.unwrap();
    assert!(
        upload_body.success,
        "Upload response must be successful: {:?}",
        upload_body.error
    );
    let doc_id = upload_body
        .data
        .as_ref()
        .and_then(|d| d["id"].as_str())
        .expect("Upload response must contain an id field")
        .to_string();

    // Get document — include the API key so auth/middleware handling is
    // identical to the upload request and no edge-case in anonymous-read
    // path can cause a 404.
    let response = client
        .get(format!("{}/api/documents/{}", base_url, doc_id))
        .header("X-API-Key", &api_key)
        .send()
        .await
        .expect("Failed to get document");

    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<Value> = response.json().await.unwrap();
    assert!(body.success);
    assert_eq!(body.data.as_ref().unwrap()["title"], "Test Document");
}

#[tokio::test]
async fn test_delete_document() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key.clone()).await;

    let client = reqwest::Client::new();

    // Upload document
    let upload_req = UploadDocumentRequest {
        title: "To Delete".to_string(),
        content: "Delete me".to_string(),
        doc_type: "markdown".to_string(),
        tags: vec![],
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let upload_response = client
        .post(format!("{}/api/documents", base_url))
        .header("X-API-Key", &api_key)
        .json(&upload_req)
        .send()
        .await
        .expect("Failed to upload");

    let upload_body: ApiResponse<Value> = upload_response.json().await.unwrap();
    let doc_id = upload_body.data.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Delete document
    let response = client
        .delete(format!("{}/api/documents/{}", base_url, doc_id))
        .header("X-API-Key", &api_key)
        .send()
        .await
        .expect("Failed to delete");

    assert_eq!(response.status(), StatusCode::OK);

    // Verify deleted
    let get_response = client
        .get(format!("{}/api/documents/{}", base_url, doc_id))
        .send()
        .await
        .expect("Failed to get");

    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// Search Tests
// ============================================================================

#[tokio::test]
async fn test_search_documents() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key.clone()).await;

    let client = reqwest::Client::new();

    // Upload documents
    let docs = vec![
        ("Rust Programming", "Learn Rust programming language basics"),
        ("Python Guide", "Introduction to Python programming"),
        ("JavaScript Tutorial", "Modern JavaScript development"),
    ];

    for (title, content) in docs {
        let upload_req = UploadDocumentRequest {
            title: title.to_string(),
            content: content.to_string(),
            doc_type: "markdown".to_string(),
            tags: vec![],
            repo_id: None,
            source_type: None,
            source_url: None,
        };

        client
            .post(format!("{}/api/documents", base_url))
            .header("X-API-Key", &api_key)
            .json(&upload_req)
            .send()
            .await
            .expect("Failed to upload");
    }

    // Wait for indexing
    sleep(Duration::from_secs(2)).await;

    // Search
    let search_req = SearchRequest {
        query: "rust programming".to_string(),
        limit: 10,
        search_type: SearchType::Hybrid,
        filters: Default::default(),
    };

    let response = client
        .post(format!("{}/api/search", base_url))
        .header("X-API-Key", &api_key)
        .json(&search_req)
        .send()
        .await
        .expect("Failed to search");

    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<Value> = response.json().await.unwrap();
    assert!(body.success);
}

// ============================================================================
// Authentication Tests
// ============================================================================

#[tokio::test]
async fn test_auth_missing_key() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key).await;

    let client = reqwest::Client::new();

    // Try to upload without API key
    let upload_req = UploadDocumentRequest {
        title: "Test".to_string(),
        content: "Test".to_string(),
        doc_type: "markdown".to_string(),
        tags: vec![],
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let response = client
        .post(format!("{}/api/documents", base_url))
        .json(&upload_req)
        .send()
        .await
        .expect("Failed to send");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_invalid_key() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key).await;

    let client = reqwest::Client::new();

    let upload_req = UploadDocumentRequest {
        title: "Test".to_string(),
        content: "Test".to_string(),
        doc_type: "markdown".to_string(),
        tags: vec![],
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let response = client
        .post(format!("{}/api/documents", base_url))
        .header("X-API-Key", "wrong_key")
        .json(&upload_req)
        .send()
        .await
        .expect("Failed to send");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_auth_anonymous_read() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key).await;

    let client = reqwest::Client::new();

    // Anonymous GET should work
    let response = client
        .get(format!("{}/api/health", base_url))
        .send()
        .await
        .expect("Failed to send");

    assert_eq!(response.status(), StatusCode::OK);
}

// ============================================================================
// Rate Limiting Tests
// ============================================================================

#[tokio::test]
async fn test_rate_limiting() {
    let (pool, api_key) = setup_test_env().await;
    // Use a tight rate-limit server so 150 rapid requests will be throttled.
    let base_url = create_test_server_strict(pool, api_key.clone()).await;

    let client = reqwest::Client::new();

    // Make many rapid requests
    let mut responses = vec![];
    for _ in 0..150 {
        let response = client
            .get(format!("{}/api/health", base_url))
            .header("X-API-Key", &api_key)
            .send()
            .await
            .expect("Failed to send");

        responses.push(response.status());
    }

    // Should have some rate limited responses
    let rate_limited = responses
        .iter()
        .filter(|s| **s == StatusCode::TOO_MANY_REQUESTS)
        .count();

    assert!(
        rate_limited > 0,
        "Expected some requests to be rate limited"
    );
}

// ============================================================================
// Indexing Job Tests
// ============================================================================

#[tokio::test]
async fn test_index_job_lifecycle() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key.clone()).await;

    let client = reqwest::Client::new();

    // Upload document
    let upload_req = UploadDocumentRequest {
        title: "Test Doc".to_string(),
        content: "Test content for indexing".to_string(),
        doc_type: "markdown".to_string(),
        tags: vec![],
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let upload_response = client
        .post(format!("{}/api/documents", base_url))
        .header("X-API-Key", &api_key)
        .json(&upload_req)
        .send()
        .await
        .expect("Failed to upload");

    let upload_body: ApiResponse<Value> = upload_response.json().await.unwrap();
    let doc_id = upload_body.data.as_ref().unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Trigger indexing
    let index_req = serde_json::json!({
        "document_id": doc_id,
        "force_reindex": false
    });

    let index_response = client
        .post(format!("{}/api/index", base_url))
        .header("X-API-Key", &api_key)
        .json(&index_req)
        .send()
        .await
        .expect("Failed to index");

    assert_eq!(index_response.status(), StatusCode::OK);

    let index_body: ApiResponse<Value> = index_response.json().await.unwrap();
    let job_id = index_body.data.as_ref().unwrap()["job_id"]
        .as_str()
        .unwrap();

    // Wait for job to complete
    sleep(Duration::from_secs(2)).await;

    // Check job status
    let status_response = client
        .get(format!("{}/api/index/jobs/{}", base_url, job_id))
        .send()
        .await
        .expect("Failed to get job status");

    assert_eq!(status_response.status(), StatusCode::OK);

    let status_body: ApiResponse<Value> = status_response.json().await.unwrap();
    assert!(status_body.success);
}

// ============================================================================
// Health Check Tests
// ============================================================================

#[tokio::test]
async fn test_health_check() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key).await;

    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/api/health", base_url))
        .send()
        .await
        .expect("Failed to send");

    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<Value> = response.json().await.unwrap();
    assert!(body.success);
    assert_eq!(body.data.as_ref().unwrap()["status"], "healthy");
}

#[tokio::test]
async fn test_stats_endpoint() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key).await;

    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/api/stats", base_url))
        .send()
        .await
        .expect("Failed to send");

    assert_eq!(response.status(), StatusCode::OK);

    let body: ApiResponse<Value> = response.json().await.unwrap();
    assert!(body.success);

    let data = body.data.as_ref().unwrap();
    assert!(data["documents"].is_object());
    assert!(data["chunks"].is_object());
    assert!(data["indexing"].is_object());
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_invalid_document_id() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key).await;

    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/api/documents/99999", base_url))
        .send()
        .await
        .expect("Failed to send");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_invalid_json() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key.clone()).await;

    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/api/documents", base_url))
        .header("X-API-Key", &api_key)
        .header("Content-Type", "application/json")
        .body("invalid json")
        .send()
        .await
        .expect("Failed to send");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ============================================================================
// Pagination Tests
// ============================================================================

#[tokio::test]
async fn test_pagination() {
    let (pool, api_key) = setup_test_env().await;
    let base_url = create_test_server(pool, api_key.clone()).await;

    let client = reqwest::Client::new();

    // Use a run-unique tag so this test's documents can be counted in isolation
    // without a global DELETE that would race against other concurrent tests
    // (e.g. test_get_document_by_id uploads a document and immediately GETs it;
    // a DELETE FROM documents here would delete that document out from under it,
    // causing a spurious 404).
    let run_tag = format!("pagination-run-{}", uuid::Uuid::new_v4());

    // Upload 25 documents, all tagged with the run-unique tag
    for i in 1..=25 {
        let upload_req = UploadDocumentRequest {
            title: format!("Pagination Doc {}", i),
            content: format!("Pagination content {}", i),
            doc_type: "markdown".to_string(),
            tags: vec![run_tag.clone()],
            repo_id: None,
            source_type: None,
            source_url: None,
        };

        let resp = client
            .post(format!("{}/api/documents", base_url))
            .header("X-API-Key", &api_key)
            .json(&upload_req)
            .send()
            .await
            .expect("Failed to upload");

        assert_eq!(
            resp.status(),
            StatusCode::CREATED,
            "Upload {} must succeed",
            i
        );
    }

    // Get first page filtered by the run-unique tag so the count is always
    // exactly 25, regardless of documents uploaded by other parallel tests.
    let page1 = client
        .get(format!(
            "{}/api/documents?page=1&limit=10&tag={}",
            base_url, run_tag
        ))
        .send()
        .await
        .expect("Failed to send page 1 request");

    assert_eq!(page1.status(), StatusCode::OK);
    let page1_body: ApiResponse<Value> = page1.json().await.unwrap();
    assert_eq!(
        page1_body.data.as_ref().unwrap()["total_pages"],
        3,
        "Expected 3 pages for 25 docs with limit 10"
    );

    // Get second page (also filtered)
    let page2 = client
        .get(format!(
            "{}/api/documents?page=2&limit=10&tag={}",
            base_url, run_tag
        ))
        .send()
        .await
        .expect("Failed to send page 2 request");

    assert_eq!(page2.status(), StatusCode::OK);
}
