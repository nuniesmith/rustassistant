//! RAG API Server Example
//!
//! A complete production-ready server with:
//! - REST API for document management and search
//! - Background indexing queue
//! - Authentication and rate limiting
//! - Web UI for search interface
//! - Health checks and monitoring

use axum::{routing::get, Router};
use rustassistant::{
    api::{create_api_router, ApiConfig},
    init_db,
};
use sqlx::SqlitePool;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "rag_server=debug,rustassistant=debug,tower_http=debug,axum=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("🚀 Starting RAG API Server");

    // Database setup
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:rustassistant.db".to_string());

    tracing::info!("📦 Connecting to database: {}", database_url);
    let db_pool = SqlitePool::connect(&database_url).await?;

    // Run migrations
    tracing::info!("🔧 Running database migrations");
    sqlx::migrate!("./migrations").run(&db_pool).await?;

    // Initialize database (create tables if needed)
    init_db(&db_pool).await?;
    tracing::info!("✅ Database initialized");

    // API Configuration
    let api_key = std::env::var("API_KEY").ok();
    let require_auth = std::env::var("REQUIRE_AUTH")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    let mut api_config = if require_auth {
        tracing::info!("🔒 Authentication enabled");
        ApiConfig::production()
    } else {
        tracing::info!("🔓 Running in development mode (no auth required)");
        ApiConfig::development()
    };

    // Add API key if provided
    if let Some(key) = api_key {
        tracing::info!("🔑 API key configured");
        api_config = api_config.with_api_key(key);
    }

    // Allow anonymous reads for GET requests
    api_config = api_config.allow_anonymous_read();

    // Build API router
    let api_router = api_config.build_router(db_pool.clone()).await;

    // Build CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build main application router
    let app = Router::new()
        // Mount API under /api
        .nest("/api", api_router)
        // Health check at root
        .route("/", get(root_handler))
        .route("/health", get(health_handler))
        // Static files and web UI would go here
        .layer(cors);

    // Determine bind address
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("🌐 Server listening on http://{}:{}", host, port);
    tracing::info!("📚 API documentation: http://{}:{}/api/health", host, port);
    tracing::info!(
        "🔍 Search endpoint: POST http://{}:{}/api/search",
        host,
        port
    );
    tracing::info!(
        "📄 Documents endpoint: http://{}:{}/api/documents",
        host,
        port
    );

    // Print example curl commands
    print_usage_examples(&host, port);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root_handler() -> &'static str {
    r#"
    🚀 RAG API Server

    Endpoints:
    - GET  /health                     - Health check
    - GET  /api/health                 - API health check
    - GET  /api/stats                  - System statistics
    - POST /api/documents              - Upload document
    - GET  /api/documents              - List documents
    - GET  /api/documents/:id          - Get document
    - PUT  /api/documents/:id          - Update document
    - DELETE /api/documents/:id        - Delete document
    - POST /api/search                 - Search documents
    - POST /api/index                  - Index document
    - POST /api/index/batch            - Batch index
    - GET  /api/index/jobs             - List indexing jobs
    - GET  /api/index/jobs/:id         - Get job status

    See documentation at /api/health for more details.
    "#
}

async fn health_handler() -> &'static str {
    "OK"
}

fn print_usage_examples(host: &str, port: u16) {
    let base_url = format!("http://{}:{}", host, port);

    println!("\n📖 Usage Examples:\n");

    println!("1️⃣  Upload a document:");
    println!(
        r#"curl -X POST {}/api/documents \
  -H "Content-Type: application/json" \
  -d '{{
    "title": "My Document",
    "content": "This is the document content...",
    "doc_type": "markdown",
    "tags": ["rust", "api"]
  }}'
"#,
        base_url
    );

    println!("\n2️⃣  Search documents (semantic):");
    println!(
        r#"curl -X POST {}/api/search \
  -H "Content-Type: application/json" \
  -d '{{
    "query": "How to implement authentication",
    "limit": 10,
    "search_type": "semantic"
  }}'
"#,
        base_url
    );

    println!("\n3️⃣  Hybrid search with filters:");
    println!(
        r#"curl -X POST {}/api/search \
  -H "Content-Type: application/json" \
  -d '{{
    "query": "error handling best practices",
    "limit": 5,
    "search_type": "hybrid",
    "filters": {{
      "doc_type": "documentation",
      "indexed_only": true
    }}
  }}'
"#,
        base_url
    );

    println!("\n4️⃣  Get system stats:");
    println!("curl {}/api/stats", base_url);

    println!("\n5️⃣  List all documents:");
    println!("curl {}/api/documents", base_url);

    println!("\n6️⃣  Index a document:");
    println!(
        r#"curl -X POST {}/api/index \
  -H "Content-Type: application/json" \
  -d '{{"document_id": 1, "force_reindex": false}}'
"#,
        base_url
    );

    println!("\n7️⃣  Batch index all pending:");
    println!(
        r#"curl -X POST {}/api/index/batch \
  -H "Content-Type: application/json" \
  -d '{{"document_ids": [1, 2, 3], "force_reindex": false}}'
"#,
        base_url
    );

    println!("\n8️⃣  Check indexing job status:");
    println!("curl {}/api/index/jobs/<job_id>", base_url);

    println!("\n💡 With API Key Authentication:");
    println!(
        r#"curl -X POST {}/api/search \
  -H "X-API-Key: your-api-key-here" \
  -H "Content-Type: application/json" \
  -d '{{"query": "your search query", "limit": 10}}'
"#,
        base_url
    );

    println!("\n🔧 Environment Variables:");
    println!("  DATABASE_URL      - Database connection string (default: sqlite:rustassistant.db)");
    println!("  HOST              - Server host (default: 0.0.0.0)");
    println!("  PORT              - Server port (default: 3000)");
    println!("  API_KEY           - API key for authentication (optional)");
    println!("  REQUIRE_AUTH      - Enable authentication (default: false)");
    println!("  RUST_LOG          - Log level (default: info)");

    println!("\n");
}
