//! Audit service server binary
//!
//! Starts the Axum web server for the audit service.

use rustassistant::prelude::*;
use rustassistant::run_server;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = Config::load()?;

    // Validate configuration
    config.validate()?;

    info!("Audit Service Starting...");
    info!("Server: {}:{}", config.server.host, config.server.port);
    info!("LLM Enabled: {}", config.llm.enabled);
    if config.llm.enabled {
        info!("LLM Model: {}", config.llm.model);
    }
    info!("Workspace: {}", config.git.workspace_dir.display());

    // Run the server
    run_server(config).await?;

    Ok(())
}
