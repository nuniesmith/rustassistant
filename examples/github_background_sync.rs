//! GitHub Background Sync Example
//!
//! This example demonstrates how to set up and run the background sync system
//! for GitHub integration. It shows:
//! - Configuring sync intervals
//! - Starting background sync jobs
//! - Monitoring sync progress
//! - Manual sync triggers

use rustassistant::github::{
    start_background_sync_with_config, BackgroundSyncConfig, BackgroundSyncManager, GitHubClient,
};
use sqlx::SqlitePool;
use std::env;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info,rustassistant=debug")
        .init();

    println!("🚀 GitHub Background Sync Example\n");

    // Check for GitHub token
    let token = match env::var("GITHUB_TOKEN") {
        Ok(t) => t,
        Err(_) => {
            eprintln!("❌ Error: GITHUB_TOKEN environment variable not set");
            eprintln!("\nTo use GitHub integration:");
            eprintln!("  1. Create a personal access token at https://github.com/settings/tokens");
            eprintln!("  2. Set the token: export GITHUB_TOKEN=your_token_here");
            eprintln!("  3. Run this example again");
            std::process::exit(1);
        }
    };

    // Get database URL
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:rustassistant.db".to_string());

    println!("🔧 Connecting to database: {}", database_url);
    let pool = SqlitePool::connect(&database_url).await?;

    // Initialize GitHub client
    println!("🔑 Initializing GitHub client...");
    let client = GitHubClient::new(token)?;

    // Configure background sync
    let config = BackgroundSyncConfig {
        full_sync_interval: 3600,       // 1 hour for demo (normally 24 hours)
        incremental_sync_interval: 300, // 5 minutes for demo (normally 1 hour)
        max_items_per_repo: Some(50),
        sync_on_startup: true,
    };

    println!("\n⚙️  Background Sync Configuration:");
    println!(
        "  Full sync interval:        {} seconds ({} minutes)",
        config.full_sync_interval,
        config.full_sync_interval / 60
    );
    println!(
        "  Incremental sync interval: {} seconds ({} minutes)",
        config.incremental_sync_interval,
        config.incremental_sync_interval / 60
    );
    println!(
        "  Max items per repo:        {}",
        config
            .max_items_per_repo
            .map_or("unlimited".to_string(), |n| n.to_string())
    );
    println!("  Sync on startup:           {}", config.sync_on_startup);

    // Create background sync manager
    let manager = BackgroundSyncManager::new(pool.clone(), client.clone(), config.clone());

    println!("\n🎯 Starting background sync system...");
    println!("   Press Ctrl+C to stop\n");

    // Spawn the background sync in a separate task
    let sync_handle = tokio::spawn(async move {
        if let Err(e) = start_background_sync_with_config(pool, client, config).await {
            eprintln!("❌ Background sync error: {}", e);
        }
    });

    // Monitor sync progress for 30 seconds (for demonstration)
    println!("📊 Monitoring sync progress for 30 seconds...\n");

    for i in 1..=6 {
        sleep(Duration::from_secs(5)).await;

        // Check rate limits
        if let Err(e) = manager.check_rate_limits().await {
            eprintln!("⚠️  Failed to check rate limits: {}", e);
        }

        // Get last sync time
        match manager.get_last_sync_time().await {
            Ok(Some(time)) => {
                println!("  [{}s] Last sync: {}", i * 5, time);
            }
            Ok(None) => {
                println!("  [{}s] No sync completed yet", i * 5);
            }
            Err(e) => {
                eprintln!("  [{}s] Error getting sync time: {}", i * 5, e);
            }
        }
    }

    println!("\n✨ Demo complete!");
    println!("\nThe background sync will continue running until you press Ctrl+C");
    println!("\n💡 In production, you would:");
    println!("   1. Run this as a long-lived service");
    println!("   2. Set up proper logging and monitoring");
    println!("   3. Configure appropriate sync intervals");
    println!("   4. Set up webhook endpoints for real-time updates");
    println!("   5. Implement health checks and error recovery");

    // Wait for background task (runs indefinitely)
    sync_handle.await?;

    Ok(())
}
