//! Integration Tests for RAG API
//!
//! Comprehensive end-to-end tests for the RAG API system including:
//! - Document upload and management
//! - Search functionality (semantic, keyword, hybrid)
//! - Background indexing jobs
//! - Authentication and rate limiting
//! - Webhook delivery
//! - Cache behavior

use axum::http::StatusCode;
use rustassistant::{
    api::{ApiConfig, ApiResponse, SearchRequest, SearchType, UploadDocumentRequest},
    init_db,
};
use serde_json::Value;
use sqlx::SqlitePool;
use std::time::Duration;
use tokio::time::sleep;

// ============================================================================
// Test Setup
// ============================================================================

/// Setup test database and API
async fn setup_test_env() -> (SqlitePool, String) {
    // Create in-memory database
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Initialize database
    init_db(&pool).await.expect("Failed to initialize database");

    // Generate test API key
    let api_key = "test_api_key_12345".to_string();

    (pool, api_key)
}

/// Create test server
async fn create_test_server(pool: SqlitePool, api_key: String) -> String {
    use axum::Router;
    use rustassistant::api::create_api_router;
    use std::net::TcpListener;

    let config = ApiConfig::development().with_api_key(api_key);

    let app = Router::new().nest("/api", config.build_router(pool).await);

    // Find available port
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    // Start server in background
    tokio::spawn(async move {
        axum::Server::from_tcp(listener)
            .unwrap()
            .serve(app.into_make_service())
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
        metadata: serde_json::json!({"author": "test"}),
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let response = client
        .post(&format!("{}/api/documents", base_url))
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
    assert!(data["id"].as_i64().is_some());
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
            metadata: serde_json::json!({}),
            repo_id: None,
            source_type: None,
            source_url: None,
        };

        client
            .post(&format!("{}/api/documents", base_url))
            .header("X-API-Key", &api_key)
            .json(&upload_req)
            .send()
            .await
            .expect("Failed to upload document");
    }

    // List documents
    let response = client
        .get(&format!("{}/api/documents?limit=10", base_url))
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
        metadata: serde_json::json!({}),
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let upload_response = client
        .post(&format!("{}/api/documents", base_url))
        .header("X-API-Key", &api_key)
        .json(&upload_req)
        .send()
        .await
        .expect("Failed to upload");

    let upload_body: ApiResponse<Value> = upload_response.json().await.unwrap();
    let doc_id = upload_body.data.unwrap()["id"].as_i64().unwrap();

    // Get document
    let response = client
        .get(&format!("{}/api/documents/{}", base_url, doc_id))
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
        metadata: serde_json::json!({}),
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let upload_response = client
        .post(&format!("{}/api/documents", base_url))
        .header("X-API-Key", &api_key)
        .json(&upload_req)
        .send()
        .await
        .expect("Failed to upload");

    let upload_body: ApiResponse<Value> = upload_response.json().await.unwrap();
    let doc_id = upload_body.data.unwrap()["id"].as_i64().unwrap();

    // Delete document
    let response = client
        .delete(&format!("{}/api/documents/{}", base_url, doc_id))
        .header("X-API-Key", &api_key)
        .send()
        .await
        .expect("Failed to delete");

    assert_eq!(response.status(), StatusCode::OK);

    // Verify deleted
    let get_response = client
        .get(&format!("{}/api/documents/{}", base_url, doc_id))
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
            metadata: serde_json::json!({}),
            repo_id: None,
            source_type: None,
            source_url: None,
        };

        client
            .post(&format!("{}/api/documents", base_url))
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
        .post(&format!("{}/api/search", base_url))
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
        metadata: serde_json::json!({}),
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let response = client
        .post(&format!("{}/api/documents", base_url))
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
        metadata: serde_json::json!({}),
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let response = client
        .post(&format!("{}/api/documents", base_url))
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
        .get(&format!("{}/api/health", base_url))
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
    let base_url = create_test_server(pool, api_key.clone()).await;

    let client = reqwest::Client::new();

    // Make many rapid requests
    let mut responses = vec![];
    for _ in 0..150 {
        let response = client
            .get(&format!("{}/api/health", base_url))
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
        metadata: serde_json::json!({}),
        repo_id: None,
        source_type: None,
        source_url: None,
    };

    let upload_response = client
        .post(&format!("{}/api/documents", base_url))
        .header("X-API-Key", &api_key)
        .json(&upload_req)
        .send()
        .await
        .expect("Failed to upload");

    let upload_body: ApiResponse<Value> = upload_response.json().await.unwrap();
    let doc_id = upload_body.data.as_ref().unwrap()["id"].as_i64().unwrap();

    // Trigger indexing
    let index_req = serde_json::json!({
        "document_id": doc_id,
        "force_reindex": false
    });

    let index_response = client
        .post(&format!("{}/api/index", base_url))
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
        .get(&format!("{}/api/index/jobs/{}", base_url, job_id))
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
        .get(&format!("{}/api/health", base_url))
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
        .get(&format!("{}/api/stats", base_url))
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
        .get(&format!("{}/api/documents/99999", base_url))
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
        .post(&format!("{}/api/documents", base_url))
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

    // Upload 25 documents
    for i in 1..=25 {
        let upload_req = UploadDocumentRequest {
            title: format!("Document {}", i),
            content: format!("Content {}", i),
            doc_type: "markdown".to_string(),
            tags: vec![],
            metadata: serde_json::json!({}),
            repo_id: None,
            source_type: None,
            source_url: None,
        };

        client
            .post(&format!("{}/api/documents", base_url))
            .header("X-API-Key", &api_key)
            .json(&upload_req)
            .send()
            .await
            .expect("Failed to upload");
    }

    // Get first page
    let page1 = client
        .get(&format!("{}/api/documents?page=1&limit=10", base_url))
        .send()
        .await
        .expect("Failed to send");

    let page1_body: ApiResponse<Value> = page1.json().await.unwrap();
    assert_eq!(page1_body.data.as_ref().unwrap()["total_pages"], 3);

    // Get second page
    let page2 = client
        .get(&format!("{}/api/documents?page=2&limit=10", base_url))
        .send()
        .await
        .expect("Failed to send");

    assert_eq!(page2.status(), StatusCode::OK);
}
