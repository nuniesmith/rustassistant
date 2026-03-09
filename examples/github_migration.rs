//! GitHub Integration Migration Helper
//!
//! This example demonstrates how to run the GitHub integration migration
//! and initialize the database schema.

use rustassistant::db::init_db;
use sqlx::PgPool;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get database URL from environment or use default
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://rustassistant:changeme@localhost:5432/rustassistant".to_string()
    });

    println!("🔧 Initializing database at: {}", database_url);

    // Initialize the main database
    init_db(&database_url).await?;
    println!("✅ Main database initialized");

    // Connect to database
    let pool = PgPool::connect(&database_url).await?;
    println!("✅ Connected to database");

    // Read and execute the GitHub migration
    let migration_sql = include_str!("../migrations/002_github_integration.sql");

    println!("🚀 Running GitHub integration migration...");

    // Execute the migration
    sqlx::query(migration_sql).execute(&pool).await?;

    println!("✅ GitHub integration migration completed successfully!");

    // Verify tables were created
    let table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name LIKE 'github_%'",
    )
    .fetch_one(&pool)
    .await?;

    println!("📊 Created {} GitHub tables", table_count);

    // List the tables
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'github_%' ORDER BY name",
    )
    .fetch_all(&pool)
    .await?;

    println!("\n📋 GitHub Integration Tables:");
    for table in tables {
        println!("   - {}", table);
    }

    println!("\n✨ Migration complete! You can now:");
    println!("   1. Set GITHUB_TOKEN environment variable");
    println!("   2. Run: cargo run --example github_test");
    println!("   3. Or use CLI: cargo run -- github sync");

    Ok(())
}
