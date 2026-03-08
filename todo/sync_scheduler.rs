// src/sync_scheduler.rs
// STUB: Background sync scheduler for registered repos
// TODO: integrate with tokio task manager / existing job queue if present
// TODO: hook into GitHub webhook events for push-triggered syncs

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;
use tracing::{error, info, warn};

use crate::repo_sync::RepoSyncService;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SyncSchedulerConfig {
    /// How often to run background syncs (default: 5 minutes)
    pub interval: Duration,
    /// Max repos to sync concurrently
    pub concurrency: usize,
    /// Skip repos that were synced within this window
    pub skip_if_synced_within: Duration,
}

impl Default for SyncSchedulerConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(300), // 5 min
            concurrency: 3,
            skip_if_synced_within: Duration::from_secs(60),
        }
    }
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

pub struct SyncScheduler {
    config: SyncSchedulerConfig,
    service: Arc<RwLock<RepoSyncService>>,
}

impl SyncScheduler {
    pub fn new(config: SyncSchedulerConfig, service: Arc<RwLock<RepoSyncService>>) -> Self {
        Self { config, service }
    }

    /// Spawn the background sync loop. Call once at startup.
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!(
                interval_secs = self.config.interval.as_secs(),
                "SyncScheduler started"
            );
            let mut ticker = time::interval(self.config.interval);
            loop {
                ticker.tick().await;
                self.run_sync_pass().await;
            }
        })
    }

    async fn run_sync_pass(&self) {
        let repo_ids: Vec<String> = {
            let service = self.service.read().await;
            service.list_repos().iter().map(|r| r.id.clone()).collect()
        };

        if repo_ids.is_empty() {
            return;
        }

        info!(count = repo_ids.len(), "Running scheduled sync pass");

        // TODO: use tokio::task::JoinSet with concurrency limit instead of sequential
        for id in repo_ids {
            // Check if recently synced
            let skip = {
                let service = self.service.read().await;
                if let Some(repo) = service.get_repo(&id) {
                    if let Some(last) = repo.last_synced {
                        let elapsed = crate::sync_scheduler::unix_now().saturating_sub(last);
                        elapsed < self.config.skip_if_synced_within.as_secs()
                    } else {
                        false
                    }
                } else {
                    true
                }
            };

            if skip {
                continue;
            }

            let mut service = self.service.write().await;
            match service.sync(&id).await {
                Ok(result) => {
                    info!(
                        repo = %id,
                        files = result.files_walked,
                        todos = result.todos_found,
                        symbols = result.symbols_found,
                        "Scheduled sync complete"
                    );
                }
                Err(e) => {
                    error!(repo = %id, error = %e, "Scheduled sync failed");
                }
            }
        }
    }
}

fn unix_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}


// =============================================================================
// .rustassistant/  directory spec
// =============================================================================
//
// Written here as a doc constant so it can be emitted by the registration flow.

pub const RUSTASSISTANT_DIR_SPEC: &str = r#"
# .rustassistant/ Directory Specification
# Generated and managed by RustAssistant — do not edit manually.
#
# manifest.json   — repo identity, sync timestamps, crate metadata
# tree.txt        — full file tree snapshot (regenerated on sync)
# todos.json      — all TODO/STUB/FIXME/HACK tags with file:line refs
# symbols.json    — public functions, structs, traits, impls
# context.md      — human-readable summary injected into LLM prompts
# embeddings.bin  — cached vector embeddings (excluded from git)
#
# Add to .gitignore:
#   .rustassistant/embeddings.bin
#
# Commit everything else — the cache files are useful diffs across branches.
"#;
