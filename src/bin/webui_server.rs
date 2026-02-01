//! Rustassistant Web UI Server
//!
//! Standalone web server that serves the HTMX-based web interface for Rustassistant.

use axum::Router;
use rustassistant::db::Database;
use rustassistant::web_ui::{create_router, WebAppState};
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    info!("🚀 Starting Rustassistant Web UI Server...");

    // Initialize database
    let db_path =
        std::env::var("DATABASE_PATH").unwrap_or_else(|_| "data/rustassistant.db".to_string());

    info!("📊 Connecting to database: {}", db_path);
    let db = Database::new(&db_path).await?;

    // Create web app state
    let state = WebAppState::new(db);

    // Create web UI router
    let web_router = create_router(state);

    // Serve static files
    let static_service = ServeDir::new("static");

    // Combine routers
    let app = Router::new()
        .merge(web_router)
        .nest_service("/static", static_service)
        .layer(TraceLayer::new_for_http());

    // Get server address from environment or use default
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3001".to_string())
        .parse::<u16>()
        .unwrap_or(3001);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    info!("✅ Rustassistant Web UI is running!");
    info!("🌐 Open your browser at: http://{}", addr);
    info!("📝 Dashboard: http://{}/", addr);
    info!("📋 Notes: http://{}/notes", addr);
    info!("📦 Repositories: http://{}/repos", addr);
    info!("💰 Costs: http://{}/costs", addr);
    info!("🔍 Analyze: http://{}/analyze", addr);
    info!("");
    info!("Press Ctrl+C to stop the server");

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
