//! Test fetching commits from a repository

use rustassistant::github::GitHubClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get GitHub token
    let token = env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");

    println!("🔧 Initializing GitHub client...");
    let client = GitHubClient::new(&token)?;

    // Test repository
    let owner = "nuniesmith";
    let repo = "fks";

    println!("📦 Fetching repository: {}/{}", owner, repo);

    // Get repository info
    match client.get_repo(owner, repo).await {
        Ok(repository) => {
            println!("✅ Repository found: {}", repository.full_name);
            println!("   Description: {:?}", repository.description);
            println!("   Language: {:?}", repository.language);
            println!("   Default branch: {}", repository.default_branch);
            println!("   Stars: {}", repository.stargazers_count);
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch repository: {}", e);
            return Err(e.into());
        }
    }

    println!("\n📝 Fetching commits...");

    // Try to fetch commits
    match client.list_commits(owner, repo, Some(10)).await {
        Ok(commits) => {
            println!("✅ Found {} commits", commits.len());

            if commits.is_empty() {
                println!("⚠️  Repository has no commits!");
            } else {
                println!("\n📋 Recent commits:");
                for (i, commit) in commits.iter().take(5).enumerate() {
                    println!(
                        "   {}. {} - {}",
                        i + 1,
                        &commit.sha[..8],
                        commit
                            .commit
                            .message
                            .lines()
                            .next()
                            .unwrap_or("(no message)")
                    );
                    println!("      Author: {}", commit.commit.author.name);
                    println!("      Date: {}", commit.commit.author.date);
                    if let Some(stats) = &commit.stats {
                        println!("      Changes: +{} -{}", stats.additions, stats.deletions);
                    }
                    println!();
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch commits: {}", e);
            eprintln!("   This might mean:");
            eprintln!("   - The repository is empty (no commits yet)");
            eprintln!("   - You don't have access to this repository");
            eprintln!("   - There's an API error");
            return Err(e.into());
        }
    }

    println!("\n✨ Test complete!");

    Ok(())
}
