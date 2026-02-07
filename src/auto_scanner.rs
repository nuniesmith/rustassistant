//! Automatic Repository Scanner
//!
//! Provides background scanning of enabled repositories at configurable intervals.
//! Monitors git status and automatically re-analyzes changed files.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::db::{Database, Repository};

use crate::refactor_assistant::RefactorAssistant;
use crate::repo_cache_sql::RepoCacheSql;
use crate::repo_manager::RepoManager;

/// Auto-scanner configuration
#[derive(Debug, Clone)]
pub struct AutoScannerConfig {
    /// Global enable/disable
    pub enabled: bool,
    /// Default scan interval in minutes
    pub default_interval_minutes: u64,
    /// Maximum concurrent scans
    pub max_concurrent_scans: usize,
}

impl Default for AutoScannerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_interval_minutes: 60,
            max_concurrent_scans: 2,
        }
    }
}

/// Git status for a file
#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    Unmodified,
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
}

/// Repository scan state
#[derive(Debug, Clone)]
pub struct RepoScanState {
    pub repo_id: String,
    pub repo_path: PathBuf,
    pub last_scan: Option<i64>,
    pub last_git_hash: Option<String>,
    pub modified_files: Vec<PathBuf>,
}

/// Background repository scanner
pub struct AutoScanner {
    config: AutoScannerConfig,
    pool: sqlx::SqlitePool,
    repos_dir: PathBuf,
    scan_states: Arc<RwLock<HashMap<String, RepoScanState>>>,
    repo_manager: Arc<RepoManager>,
}

impl AutoScanner {
    /// Create a new auto-scanner
    pub fn new(config: AutoScannerConfig, pool: sqlx::SqlitePool, repos_dir: PathBuf) -> Self {
        // Get GitHub token from environment for private repos
        let github_token = std::env::var("GITHUB_TOKEN").ok();

        let repo_manager = Arc::new(
            RepoManager::new(&repos_dir, github_token).expect("Failed to create RepoManager"),
        );

        Self {
            config,
            pool,
            repos_dir,
            scan_states: Arc::new(RwLock::new(HashMap::new())),
            repo_manager,
        }
    }

    /// Start the background scanner
    pub async fn start(self: Arc<Self>) -> Result<()> {
        if !self.config.enabled {
            info!("Auto-scanner is disabled");
            return Ok(());
        }

        info!(
            "Starting auto-scanner with {} minute intervals",
            self.config.default_interval_minutes
        );

        // Main scan loop
        loop {
            if let Err(e) = self.scan_enabled_repos().await {
                error!("Error during scan cycle: {}", e);
            }

            // Sleep for 1 minute, then check which repos need scanning
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }

    /// Scan all enabled repositories
    async fn scan_enabled_repos(&self) -> Result<()> {
        let repos = self.get_enabled_repos().await?;

        if repos.is_empty() {
            debug!("No enabled repositories to scan");
            return Ok(());
        }

        info!("Checking {} enabled repositories", repos.len());

        // Process repos in parallel (limited concurrency)
        let semaphore = Arc::new(tokio::sync::Semaphore::new(
            self.config.max_concurrent_scans,
        ));
        let mut tasks = vec![];

        for repo in repos {
            let self_clone = Arc::new(self.clone_scanner());
            let semaphore_clone = semaphore.clone();

            let task = tokio::spawn(async move {
                let _permit = semaphore_clone.acquire().await.ok();
                if let Err(e) = self_clone.check_and_scan_repo(&repo).await {
                    error!("Failed to scan repo {}: {}", repo.name, e);
                }
            });

            tasks.push(task);
        }

        // Wait for all scans to complete
        for task in tasks {
            let _ = task.await;
        }

        Ok(())
    }

    /// Get all repositories with auto_scan_enabled = 1
    async fn get_enabled_repos(&self) -> Result<Vec<Repository>> {
        let repos = sqlx::query_as::<_, Repository>(
            r#"
            SELECT id, path, name, status, last_analyzed, metadata,
                   auto_scan_enabled, scan_interval_minutes, last_scan_check,
                   last_commit_hash, git_url, created_at, updated_at
            FROM repositories
            WHERE auto_scan_enabled = 1 AND status = 'active'
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(repos)
    }

    /// Check if repo needs scanning and scan if necessary
    async fn check_and_scan_repo(&self, repo: &Repository) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let interval_secs = repo.scan_interval_minutes * 60;

        // Check if enough time has passed since last scan
        if let Some(last_check) = repo.last_scan_check {
            if now - last_check < interval_secs {
                debug!(
                    "Skipping {} - scanned {} seconds ago",
                    repo.name,
                    now - last_check
                );
                return Ok(());
            }
        }

        info!("Scanning repository: {} ({})", repo.name, repo.path);

        // Track scan start time for duration calculation
        let scan_start = std::time::Instant::now();

        // Ensure the repo exists locally — clone from git_url if missing
        let repo_path = PathBuf::from(&repo.path);
        let repo_path = if !repo_path.exists() || !repo_path.join(".git").exists() {
            if let Some(ref git_url) = repo.git_url {
                info!(
                    "Local path {} not found, cloning from {}",
                    repo_path.display(),
                    git_url
                );
                match self.clone_or_update_repo(git_url, &repo.name) {
                    Ok(cloned_path) => {
                        // Update the stored path in the database to the new clone location
                        let new_path = cloned_path.to_string_lossy().to_string();
                        if let Err(e) = self.update_repo_path(&repo.id, &new_path).await {
                            error!("Failed to update repo path in DB: {}", e);
                        }
                        info!("Cloned {} to {}", repo.name, cloned_path.display());
                        cloned_path
                    }
                    Err(e) => {
                        error!("Failed to clone {} from {}: {}", repo.name, git_url, e);
                        return Ok(());
                    }
                }
            } else {
                warn!(
                    "Repo {} path {} does not exist and no git_url configured — skipping",
                    repo.name,
                    repo_path.display()
                );
                return Ok(());
            }
        } else {
            repo_path
        };

        // Update repository if it exists (git pull)
        if let Some(ref git_url) = repo.git_url {
            if let Err(e) = self.clone_or_update_repo(git_url, &repo.name) {
                warn!("Failed to update {}: {}", repo.name, e);
            }
        }

        // Check for changes (both committed and uncommitted)
        let current_head = self.get_head_hash(&repo_path)?;
        let changed_files = self
            .get_changed_files(
                &repo_path,
                repo.last_commit_hash.as_deref(),
                current_head.as_deref(),
            )
            .await?;

        if changed_files.is_empty() {
            debug!("No changes detected in {}", repo.name);
            // Still update the commit hash so we don't re-diff the same range
            if let Some(ref hash) = current_head {
                self.update_last_commit_hash(&repo.id, hash).await?;
            }
            // Update last_scan_check for interval tracking
            self.update_last_scan_check(&repo.id, now).await?;
            return Ok(());
        }

        info!(
            "Found {} changed files in {}",
            changed_files.len(),
            repo.name
        );

        // Start progress tracking
        let total_files = changed_files.len() as i64;
        if let Err(e) = crate::db::core::start_scan(&self.pool, &repo.id, total_files).await {
            error!("Failed to start scan progress tracking: {}", e);
        }

        // Analyze changed files with progress tracking
        let result = self
            .analyze_changed_files_with_progress(&repo.id, &repo_path, &changed_files)
            .await;

        match result {
            Ok((files_analyzed, issues_found)) => {
                // Calculate scan duration
                let duration_ms = scan_start.elapsed().as_millis() as i64;

                // Complete scan with metrics
                if let Err(e) = crate::db::core::complete_scan(
                    &self.pool,
                    &repo.id,
                    duration_ms,
                    files_analyzed,
                    issues_found,
                )
                .await
                {
                    error!("Failed to complete scan progress tracking: {}", e);
                }

                info!(
                    "Scan completed for {}: {} files, {} issues in {}ms",
                    repo.name, files_analyzed, issues_found, duration_ms
                );

                // Update last_analyzed and commit hash
                self.update_last_analyzed(&repo.id, now).await?;
                if let Some(ref hash) = current_head {
                    self.update_last_commit_hash(&repo.id, hash).await?;
                }
            }
            Err(e) => {
                error!("Scan failed for {}: {}", repo.name, e);
                if let Err(err) =
                    crate::db::core::fail_scan(&self.pool, &repo.id, &e.to_string()).await
                {
                    error!("Failed to mark scan as failed: {}", err);
                }
                return Err(e);
            }
        }

        Ok(())
    }

    /// Clone or update a repository from a git URL into the repos directory
    fn clone_or_update_repo(&self, git_url: &str, name: &str) -> Result<PathBuf> {
        self.repo_manager
            .clone_or_update(git_url, name)
            .context(format!(
                "Failed to clone or update {} from {}",
                name, git_url
            ))
    }

    /// Update the stored path for a repository in the database
    async fn update_repo_path(&self, repo_id: &str, new_path: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE repositories
            SET path = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(new_path)
        .bind(chrono::Utc::now().timestamp())
        .bind(repo_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get the current HEAD commit hash for a repository
    fn get_head_hash(&self, repo_path: &Path) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_path)
            .output()
            .context("Failed to run git rev-parse HEAD")?;

        if !output.status.success() {
            warn!("git rev-parse HEAD failed for {}", repo_path.display());
            return Ok(None);
        }

        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if hash.is_empty() {
            Ok(None)
        } else {
            Ok(Some(hash))
        }
    }

    /// Get list of modified files from both committed and uncommitted changes
    async fn get_changed_files(
        &self,
        repo_path: &Path,
        last_commit_hash: Option<&str>,
        current_head: Option<&str>,
    ) -> Result<Vec<PathBuf>> {
        use std::collections::HashSet;
        use std::process::Command;

        let mut changed_set: HashSet<PathBuf> = HashSet::new();

        // 1. Check for committed changes since last known hash
        if let (Some(old_hash), Some(new_hash)) = (last_commit_hash, current_head) {
            if old_hash != new_hash {
                let output = Command::new("git")
                    .args(["diff", "--name-status", old_hash, new_hash])
                    .current_dir(repo_path)
                    .output();

                match output {
                    Ok(out) if out.status.success() => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        for line in stdout.lines() {
                            let parts: Vec<&str> = line.split('\t').collect();
                            if parts.len() < 2 {
                                continue;
                            }
                            let status = parts[0];
                            // Skip deleted files
                            if status.starts_with('D') {
                                continue;
                            }
                            // For renames (R100), the new path is the last element
                            let file_path = parts.last().unwrap().trim();
                            if Self::is_analyzable_file(file_path) {
                                changed_set.insert(repo_path.join(file_path));
                            }
                        }
                        info!(
                            "Found {} files changed between commits {}..{}",
                            changed_set.len(),
                            &old_hash[..8.min(old_hash.len())],
                            &new_hash[..8.min(new_hash.len())]
                        );
                    }
                    Ok(out) => {
                        // git diff failed - old hash may no longer exist (force push, etc.)
                        // Fall back to listing all files in the latest commit
                        warn!(
                            "git diff failed for {}..{} ({}), falling back to HEAD diff",
                            &old_hash[..8.min(old_hash.len())],
                            &new_hash[..8.min(new_hash.len())],
                            String::from_utf8_lossy(&out.stderr).trim()
                        );
                        self.get_files_from_recent_commits(repo_path, &mut changed_set)?;
                    }
                    Err(e) => {
                        warn!("Failed to run git diff: {}", e);
                    }
                }
            }
        } else if last_commit_hash.is_none() && current_head.is_some() {
            // First scan - no stored hash yet. Check recent commits to seed initial analysis.
            info!(
                "First scan for {} - checking recent commits",
                repo_path.display()
            );
            self.get_files_from_recent_commits(repo_path, &mut changed_set)?;
        }

        // 2. Also check for uncommitted changes (working tree + staged)
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo_path)
            .output()
            .context("Failed to run git status")?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.len() < 3 {
                    continue;
                }

                let status = &line[0..2];
                let file_path = line[3..].trim();

                // Skip deleted files
                if status.contains('D') {
                    continue;
                }

                if Self::is_analyzable_file(file_path) {
                    changed_set.insert(repo_path.join(file_path));
                }
            }
        }

        Ok(changed_set.into_iter().collect())
    }

    /// Get changed files from recent commits (used for first scan or fallback)
    fn get_files_from_recent_commits(
        &self,
        repo_path: &Path,
        changed_set: &mut std::collections::HashSet<PathBuf>,
    ) -> Result<()> {
        use std::process::Command;

        // Look at files changed in the last 5 commits
        let output = Command::new("git")
            .args(["diff", "--name-only", "HEAD~5", "HEAD"])
            .current_dir(repo_path)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines() {
                    let file_path = line.trim();
                    if !file_path.is_empty() && Self::is_analyzable_file(file_path) {
                        changed_set.insert(repo_path.join(file_path));
                    }
                }
            }
            _ => {
                debug!("Could not get recent commits for {}", repo_path.display());
            }
        }

        Ok(())
    }

    /// Check if a file extension is one we should analyze
    fn is_analyzable_file(file_path: &str) -> bool {
        file_path.ends_with(".rs")
            || file_path.ends_with(".py")
            || file_path.ends_with(".js")
            || file_path.ends_with(".ts")
            || file_path.ends_with(".tsx")
            || file_path.ends_with(".sh")
            || file_path.ends_with(".kt")
            || file_path.ends_with(".java")
            || file_path.ends_with(".go")
            || file_path.ends_with(".rb")
    }

    /// Analyze changed files with progress tracking
    /// Returns (files_analyzed, issues_found)
    async fn analyze_changed_files_with_progress(
        &self,
        repo_id: &str,
        repo_path: &Path,
        files: &[PathBuf],
    ) -> Result<(i64, i64)> {
        let cache = RepoCacheSql::new_for_repo(repo_path).await?;
        let mut files_analyzed = 0i64;
        let mut issues_found = 0i64;
        let progress_update_interval = 5; // Update progress every N files

        for (idx, file) in files.iter().enumerate() {
            // Update progress periodically
            if idx % progress_update_interval == 0 || idx == files.len() - 1 {
                let current_file = file
                    .strip_prefix(repo_path)
                    .unwrap_or(file)
                    .to_string_lossy()
                    .to_string();

                if let Err(e) = crate::db::core::update_scan_progress(
                    &self.pool,
                    repo_id,
                    idx as i64,
                    Some(&current_file),
                )
                .await
                {
                    error!("Failed to update scan progress: {}", e);
                }
            }

            match self.analyze_file(repo_path, file, &cache).await {
                Ok(found_issues) => {
                    files_analyzed += 1;
                    issues_found += found_issues;
                }
                Err(e) => {
                    error!("Failed to analyze {}: {}", file.display(), e);
                }
            }
        }

        Ok((files_analyzed, issues_found))
    }

    /// Analyze a single file
    /// Returns the number of issues found (0 or 1 for now)
    async fn analyze_file(
        &self,
        repo_path: &Path,
        file_path: &Path,
        cache: &RepoCacheSql,
    ) -> Result<i64> {
        // Read file content
        let content = match tokio::fs::read_to_string(file_path).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Cannot read {}: {}", file_path.display(), e);
                return Ok(0);
            }
        };

        // Get relative path
        let rel_path = file_path
            .strip_prefix(repo_path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        // Check cache first
        if cache
            .get(
                crate::repo_cache::CacheType::Refactor,
                &rel_path,
                &content,
                "xai",
                "grok-beta",
                None,
                None,
            )
            .await?
            .is_some()
        {
            debug!("Cache hit for {}", rel_path);
            return Ok(0);
        }

        info!("Analyzing {}", rel_path);

        // Create RefactorAssistant for analysis
        let db = Database::from_pool(self.pool.clone());
        let assistant = RefactorAssistant::new(db).await?;

        // Analyze with LLM
        let analysis = assistant.analyze_file(file_path).await?;

        // Cache the result
        let result_json = serde_json::to_value(&analysis)?;
        cache
            .set(crate::repo_cache_sql::CacheSetParams {
                cache_type: crate::repo_cache::CacheType::Refactor,
                repo_path: &repo_path.to_string_lossy(),
                file_path: &rel_path,
                content: &content,
                provider: "xai",
                model: "grok-beta",
                result: result_json,
                tokens_used: analysis.tokens_used,
                prompt_hash: None,
                schema_version: None,
            })
            .await?;

        debug!("Cached analysis for {}", rel_path);

        // For now, count any analysis as 1 issue found
        // TODO: Parse analysis.suggestions to count actual issues
        Ok(1)
    }

    /// Update last_scan_check timestamp
    async fn update_last_scan_check(&self, repo_id: &str, timestamp: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE repositories
            SET last_scan_check = ?
            WHERE id = ?
            "#,
        )
        .bind(timestamp)
        .bind(repo_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update last_analyzed timestamp
    async fn update_last_analyzed(&self, repo_id: &str, timestamp: i64) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE repositories
            SET last_analyzed = ?
            WHERE id = ?
            "#,
        )
        .bind(timestamp)
        .bind(repo_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update last_commit_hash for a repository
    async fn update_last_commit_hash(&self, repo_id: &str, hash: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE repositories
            SET last_commit_hash = ?
            WHERE id = ?
            "#,
        )
        .bind(hash)
        .bind(repo_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Clone scanner for async tasks
    fn clone_scanner(&self) -> Self {
        Self {
            config: self.config.clone(),
            pool: self.pool.clone(),
            repos_dir: self.repos_dir.clone(),
            scan_states: self.scan_states.clone(),
            repo_manager: self.repo_manager.clone(),
        }
    }
}

/// Enable auto-scan for a repository
pub async fn enable_auto_scan(
    pool: &sqlx::SqlitePool,
    repo_id: &str,
    interval_minutes: Option<i64>,
) -> Result<()> {
    let interval = interval_minutes.unwrap_or(60);

    sqlx::query(
        r#"
        UPDATE repositories
        SET auto_scan_enabled = 1, scan_interval_minutes = ?
        WHERE id = ?
        "#,
    )
    .bind(interval)
    .bind(repo_id)
    .execute(pool)
    .await?;

    info!(
        "Enabled auto-scan for repo {} (interval: {} minutes)",
        repo_id, interval
    );

    Ok(())
}

/// Disable auto-scan for a repository
pub async fn disable_auto_scan(pool: &sqlx::SqlitePool, repo_id: &str) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE repositories
        SET auto_scan_enabled = 0
        WHERE id = ?
        "#,
    )
    .bind(repo_id)
    .execute(pool)
    .await?;

    info!("Disabled auto-scan for repo {}", repo_id);

    Ok(())
}

/// Force a scan check for a repository (reset last_scan_check)
pub async fn force_scan(pool: &sqlx::SqlitePool, repo_id: &str) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE repositories
        SET last_scan_check = NULL
        WHERE id = ?
        "#,
    )
    .bind(repo_id)
    .execute(pool)
    .await?;

    info!("Forced scan check for repo {}", repo_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AutoScannerConfig::default();
        assert!(config.enabled);
        assert_eq!(config.default_interval_minutes, 60);
        assert_eq!(config.max_concurrent_scans, 2);
    }

    #[test]
    fn test_file_status() {
        let status = FileStatus::Modified;
        assert_eq!(status, FileStatus::Modified);
        assert_ne!(status, FileStatus::Unmodified);
    }
}
