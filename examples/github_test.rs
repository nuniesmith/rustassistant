//! GitHub Integration Test Example
//!
//! This example demonstrates:
//! - Initializing the GitHub client
//! - Running initial sync
//! - Searching repositories, issues, and PRs
//! - Checking rate limits

use rustassistant::github::search::{GitHubSearcher, SearchQuery, SearchType};
use rustassistant::github::{GitHubClient, SyncEngine, SyncOptions};
use sqlx::SqlitePool;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

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
    println!("🚀 Initializing GitHub client...");
    let client = GitHubClient::new(token)?;

    // Check rate limits
    println!("\n📊 Checking GitHub API rate limits...");
    let rate_limit = client.get_rate_limit().await?;
    println!(
        "   Core API: {}/{} remaining (resets at {})",
        rate_limit.resources.core.remaining,
        rate_limit.resources.core.limit,
        rate_limit.resources.core.reset
    );
    println!(
        "   Search API: {}/{} remaining",
        rate_limit.resources.search.remaining, rate_limit.resources.search.limit
    );

    // Initialize sync engine
    println!("\n🔄 Initializing sync engine...");
    let sync_engine = SyncEngine::new(client.clone(), pool.clone());

    // Get authenticated user
    println!("\n👤 Getting authenticated user...");
    let user = client.get_authenticated_user().await?;
    println!(
        "   Logged in as: {} (@{})",
        user.name.as_deref().unwrap_or("N/A"),
        user.login
    );

    // Sync all repositories
    println!("\n📦 Syncing GitHub data...");
    println!("   (This may take a few moments for the first sync)");

    let sync_options = SyncOptions::default();
    let result = sync_engine.sync_with_options(sync_options).await?;

    println!("✅ Sync completed:");
    println!("   Repositories: {}", result.repos_synced);
    println!("   Issues: {}", result.issues_synced);
    println!("   Pull Requests: {}", result.prs_synced);
    println!("   Duration: {:.2}s", result.duration_secs);

    // Initialize searcher
    println!("\n🔍 Testing search functionality...");
    let searcher = GitHubSearcher::new(pool.clone());

    // Search for repositories
    println!("\n   Searching for repositories...");
    let repo_query = SearchQuery::new("")
        .with_type(SearchType::Repositories)
        .limit(5);
    let repos = searcher.search(repo_query).await?;
    println!("   Found {} repositories:", repos.len());
    for repo in repos.iter().take(5) {
        if let rustassistant::github::search::SearchResult::Repository(repo_data) = repo {
            println!("      - {} (⭐ {})", repo_data.full_name, repo_data.stars);
        }
    }

    // Search for open issues
    println!("\n   Searching for open issues...");
    let issue_query = SearchQuery::new("")
        .with_type(SearchType::Issues)
        .only_open()
        .limit(5);
    let issues = searcher.search(issue_query).await?;
    println!("   Found {} open issues:", issues.len());
    for issue in issues.iter().take(5) {
        if let rustassistant::github::search::SearchResult::Issue(issue_data) = issue {
            println!("      - #{}: {}", issue_data.number, issue_data.title);
        }
    }

    // Search for open pull requests
    println!("\n   Searching for open pull requests...");
    let pr_query = SearchQuery::new("")
        .with_type(SearchType::PullRequests)
        .only_open()
        .limit(5);
    let prs = searcher.search(pr_query).await?;
    println!("   Found {} open pull requests:", prs.len());
    for pr in prs.iter().take(5) {
        if let rustassistant::github::search::SearchResult::PullRequest(pr_data) = pr {
            println!("      - #{}: {}", pr_data.number, pr_data.title);
        }
    }

    // Get sync statistics
    println!("\n📊 Final Statistics:");
    let stats: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            (SELECT COUNT(*) FROM github_repositories) as repos,
            (SELECT COUNT(*) FROM github_issues) as issues,
            (SELECT COUNT(*) FROM github_pull_requests) as prs,
            (SELECT COUNT(*) FROM github_commits) as commits
        "#,
    )
    .fetch_one(&pool)
    .await?;

    println!("   📦 Repositories: {}", stats.0);
    println!("   🐛 Issues: {}", stats.1);
    println!("   🔀 Pull Requests: {}", stats.2);
    println!("   📝 Commits: {}", stats.3);

    println!("\n✨ GitHub integration test completed successfully!");
    println!("\n💡 Next steps:");
    println!("   - Use CLI commands: cargo run -- github stats");
    println!("   - Search via CLI: cargo run -- github search 'query'");
    println!("   - Check issues: cargo run -- github issues");
    println!("   - View PRs: cargo run -- github prs");

    Ok(())
}
