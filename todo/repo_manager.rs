// src/repo_manager.rs
//! Repository Manager - Clones and updates repos at runtime.
//!
//! Replaces volume mounts by managing repos inside the container.
//! Repos are cloned to REPOS_PATH (default: /app/repos/) and updated
//! via git pull on each scan cycle.

use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{error, info, warn};

use crate::db::scan_events::{log_error, log_info};

/// Where cloned repos live inside the container
fn repos_base_path() -> PathBuf {
    PathBuf::from(
        std::env::var("REPOS_PATH").unwrap_or_else(|_| "/app/repos".to_string()),
    )
}

/// Get the local path for a repository
pub fn repo_local_path(repo_name: &str) -> PathBuf {
    repos_base_path().join(repo_name)
}

/// Result type for repo operations
#[derive(Debug)]
pub struct RepoSyncResult {
    pub local_path: PathBuf,
    pub was_cloned: bool,    // true if freshly cloned
    pub was_updated: bool,   // true if pull had new commits
    pub commit_hash: String,
    pub error: Option<String>,
}

/// Clone or update a repository. Returns the local path.
///
/// If the repo exists locally, does `git pull`.
/// If not, does `git clone --depth 1` for efficiency.
pub async fn ensure_repo(
    pool: &SqlitePool,
    repo_id: &str,
    git_url: &str,
    repo_name: &str,
) -> Result<RepoSyncResult, String> {
    let local_path = repo_local_path(repo_name);

    // Check if already cloned
    if local_path.join(".git").exists() {
        match update_repo(&local_path).await {
            Ok((updated, hash)) => {
                if updated {
                    log_info(pool, Some(repo_id), "repo_updated", &format!(
                        "Updated {} to {}", repo_name, &hash[..8]
                    )).await.ok();
                }
                Ok(RepoSyncResult {
                    local_path,
                    was_cloned: false,
                    was_updated: updated,
                    commit_hash: hash,
                    error: None,
                })
            }
            Err(e) => {
                log_error(pool, Some(repo_id), "repo_update_error", "Failed to update repo", &e).await.ok();
                Err(e)
            }
        }
    } else {
        match clone_repo(git_url, &local_path).await {
            Ok(hash) => {
                log_info(pool, Some(repo_id), "repo_cloned", &format!(
                    "Cloned {} ({})", repo_name, &hash[..8]
                )).await.ok();
                Ok(RepoSyncResult {
                    local_path,
                    was_cloned: true,
                    was_updated: false,
                    commit_hash: hash,
                    error: None,
                })
            }
            Err(e) => {
                log_error(pool, Some(repo_id), "repo_clone_error", "Failed to clone repo", &e).await.ok();
                Err(e)
            }
        }
    }
}

/// Clone a repo with shallow depth for speed
async fn clone_repo(git_url: &str, local_path: &Path) -> Result<String, String> {
    // Create parent directory
    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    info!("Cloning {} to {:?}", git_url, local_path);

    // Build clone command with optional auth
    let mut cmd = Command::new("git");
    cmd.args([
        "clone",
        "--depth", "50",         // Shallow clone for speed, but enough history for diffs
        "--single-branch",
        git_url,
        local_path.to_str().unwrap_or(""),
    ]);

    // Set GitHub token for auth if available
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        cmd.env("GIT_ASKPASS", "echo");
        cmd.env("GIT_TERMINAL_PROMPT", "0");
        // Use token in URL for HTTPS repos
        let authed_url = inject_token_in_url(git_url, &token);
        cmd.args(["--origin", "origin"]);
        // Reset args and use authed URL
        let mut cmd2 = Command::new("git");
        cmd2.args([
            "clone",
            "--depth", "50",
            "--single-branch",
            &authed_url,
            local_path.to_str().unwrap_or(""),
        ]);
        cmd = cmd2;
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute git clone: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git clone failed: {}", stderr));
    }

    get_head_hash(local_path)
}

/// Pull latest changes
async fn update_repo(local_path: &Path) -> Result<(bool, String), String> {
    let before_hash = get_head_hash(local_path)?;

    info!("Updating {:?}", local_path);

    let output = Command::new("git")
        .current_dir(local_path)
        .args(["pull", "--ff-only", "--no-edit"])
        .output()
        .map_err(|e| format!("Failed to execute git pull: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // If pull fails (e.g., diverged), try fetch + reset
        warn!("git pull failed, trying fetch+reset: {}", stderr);
        let _ = Command::new("git")
            .current_dir(local_path)
            .args(["fetch", "origin"])
            .output();
        let _ = Command::new("git")
            .current_dir(local_path)
            .args(["reset", "--hard", "origin/HEAD"])
            .output();
    }

    let after_hash = get_head_hash(local_path)?;
    let was_updated = before_hash != after_hash;

    Ok((was_updated, after_hash))
}

/// Get HEAD commit hash
fn get_head_hash(local_path: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(local_path)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("Failed to get HEAD hash: {}", e))?;

    if !output.status.success() {
        return Err("Failed to get HEAD hash".to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Inject auth token into HTTPS git URL
fn inject_token_in_url(url: &str, token: &str) -> String {
    if url.starts_with("https://github.com/") {
        url.replace(
            "https://github.com/",
            &format!("https://x-access-token:{}@github.com/", token),
        )
    } else if url.starts_with("https://") {
        // Generic HTTPS - insert token after scheme
        url.replacen("https://", &format!("https://oauth2:{}@", token), 1)
    } else {
        // SSH URL or other - return as-is
        url.to_string()
    }
}

/// Build a git URL from a GitHub username and repo name
pub fn github_url(user: &str, repo: &str) -> String {
    format!("https://github.com/{}/{}.git", user, repo)
}

/// Check if a path is a valid git repo
pub fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

/// Get list of changed files since a commit hash
pub fn get_changed_files(local_path: &Path, since_hash: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .current_dir(local_path)
        .args(["diff", "--name-only", since_hash, "HEAD"])
        .output()
        .map_err(|e| format!("Failed to get changed files: {}", e))?;

    if !output.status.success() {
        return Ok(vec![]); // If diff fails, scan everything
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect())
}

/// Count total files in repo (excluding .git)
pub fn count_repo_files(local_path: &Path) -> Result<usize, String> {
    let output = Command::new("git")
        .current_dir(local_path)
        .args(["ls-files"])
        .output()
        .map_err(|e| format!("Failed to list files: {}", e))?;

    if !output.status.success() {
        return Err("Failed to list files".to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_token_github() {
        let url = "https://github.com/user/repo.git";
        let result = inject_token_in_url(url, "ghp_test123");
        assert_eq!(
            result,
            "https://x-access-token:ghp_test123@github.com/user/repo.git"
        );
    }

    #[test]
    fn test_inject_token_generic_https() {
        let url = "https://gitlab.com/user/repo.git";
        let result = inject_token_in_url(url, "token123");
        assert_eq!(
            result,
            "https://oauth2:token123@gitlab.com/user/repo.git"
        );
    }

    #[test]
    fn test_inject_token_ssh_unchanged() {
        let url = "git@github.com:user/repo.git";
        let result = inject_token_in_url(url, "token123");
        assert_eq!(result, url);
    }

    #[test]
    fn test_github_url() {
        assert_eq!(
            github_url("nuniesmith", "rustassistant"),
            "https://github.com/nuniesmith/rustassistant.git"
        );
    }
}
