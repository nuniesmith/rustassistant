//! Queue Processor
//!
//! Handles moving items through processing stages:
//! Inbox → PendingAnalysis → Analyzing → PendingTagging → Ready

use crate::db::queue::{QueueItem, QueuePriority, QueueSource, QueueStage};
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

// ============================================================================
// Queue Operations
// ============================================================================

/// Add raw content to the queue for processing
pub async fn enqueue(
    pool: &SqlitePool,
    content: &str,
    source: QueueSource,
    priority: QueuePriority,
    repo_id: Option<&str>,
    file_path: Option<&str>,
    line_number: Option<i32>,
) -> Result<QueueItem> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    let content_hash = format!("{:x}", md5::compute(content.as_bytes()));

    // Check for duplicate
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM queue_items WHERE content_hash = ? AND stage != 'archived'")
            .bind(&content_hash)
            .fetch_optional(pool)
            .await?;

    if let Some((existing_id,)) = existing {
        warn!(
            "Duplicate content detected, returning existing item: {}",
            existing_id
        );
        return get_queue_item(pool, &existing_id).await;
    }

    sqlx::query(
        r#"
        INSERT INTO queue_items
        (id, content, stage, source, priority, repo_id, file_path, line_number,
         content_hash, retry_count, created_at, updated_at)
        VALUES (?, ?, 'inbox', ?, ?, ?, ?, ?, ?, 0, ?, ?)
    "#,
    )
    .bind(&id)
    .bind(content)
    .bind(format!("{:?}", source).to_lowercase())
    .bind(priority as i32)
    .bind(repo_id)
    .bind(file_path)
    .bind(line_number)
    .bind(&content_hash)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    info!("Enqueued item {} from {:?}", id, source);
    get_queue_item(pool, &id).await
}

/// Get a queue item by ID
pub async fn get_queue_item(pool: &SqlitePool, id: &str) -> Result<QueueItem> {
    sqlx::query_as::<_, QueueItem>("SELECT * FROM queue_items WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

/// Move item to next stage
pub async fn advance_stage(pool: &SqlitePool, id: &str) -> Result<QueueStage> {
    let item = get_queue_item(pool, id).await?;
    let current = parse_stage(&item.stage);

    let next = match current {
        QueueStage::Inbox => QueueStage::PendingAnalysis,
        QueueStage::PendingAnalysis => QueueStage::Analyzing,
        QueueStage::Analyzing => QueueStage::PendingTagging,
        QueueStage::PendingTagging => QueueStage::Ready,
        QueueStage::Ready => QueueStage::Ready, // Already done
        QueueStage::Failed => QueueStage::PendingAnalysis, // Retry
        QueueStage::Archived => QueueStage::Archived,
    };

    let now = Utc::now().timestamp();
    let processed_at = if next == QueueStage::Ready {
        Some(now)
    } else {
        None
    };

    sqlx::query("UPDATE queue_items SET stage = ?, updated_at = ?, processed_at = COALESCE(?, processed_at) WHERE id = ?")
        .bind(format!("{:?}", next).to_lowercase())
        .bind(now)
        .bind(processed_at)
        .bind(id)
        .execute(pool)
        .await?;

    info!("Item {} moved from {:?} to {:?}", id, current, next);
    Ok(next)
}

/// Mark item as failed
pub async fn mark_failed(pool: &SqlitePool, id: &str, error: &str) -> Result<()> {
    let now = Utc::now().timestamp();

    sqlx::query(
        "UPDATE queue_items SET stage = 'failed', last_error = ?, retry_count = retry_count + 1, updated_at = ? WHERE id = ?"
    )
    .bind(error)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    error!("Item {} failed: {}", id, error);
    Ok(())
}

/// Update item with analysis results
pub async fn update_analysis(pool: &SqlitePool, id: &str, analysis: &AnalysisResult) -> Result<()> {
    let now = Utc::now().timestamp();
    let analysis_json = serde_json::to_string(analysis)?;
    let tags = analysis.tags.join(",");

    sqlx::query(r#"
        UPDATE queue_items
        SET analysis = ?, tags = ?, category = ?, score = ?, stage = 'pending_tagging', updated_at = ?
        WHERE id = ?
    "#)
    .bind(&analysis_json)
    .bind(&tags)
    .bind(&analysis.category)
    .bind(analysis.score)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get next items to process for a given stage
pub async fn get_pending_items(
    pool: &SqlitePool,
    stage: QueueStage,
    limit: i32,
) -> Result<Vec<QueueItem>> {
    let stage_str = format!("{:?}", stage).to_lowercase();

    sqlx::query_as::<_, QueueItem>(
        "SELECT * FROM queue_items WHERE stage = ? ORDER BY priority ASC, created_at ASC LIMIT ?",
    )
    .bind(&stage_str)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Get items that failed but can be retried
pub async fn get_retriable_items(pool: &SqlitePool, max_retries: i32) -> Result<Vec<QueueItem>> {
    sqlx::query_as::<_, QueueItem>(
        "SELECT * FROM queue_items WHERE stage = 'failed' AND retry_count < ? ORDER BY priority ASC"
    )
    .bind(max_retries)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Get queue statistics
pub async fn get_queue_stats(pool: &SqlitePool) -> Result<QueueStats> {
    let counts: Vec<(String, i64)> =
        sqlx::query_as("SELECT stage, COUNT(*) as count FROM queue_items GROUP BY stage")
            .fetch_all(pool)
            .await?;

    let mut stats = QueueStats::default();
    for (stage, count) in counts {
        match stage.as_str() {
            "inbox" => stats.inbox = count,
            "pending_analysis" => stats.pending_analysis = count,
            "analyzing" => stats.analyzing = count,
            "pending_tagging" => stats.pending_tagging = count,
            "ready" => stats.ready = count,
            "failed" => stats.failed = count,
            "archived" => stats.archived = count,
            _ => {}
        }
    }

    Ok(stats)
}

// ============================================================================
// Analysis Result Types
// ============================================================================

/// LLM analysis output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Short summary of the content
    pub summary: String,

    /// Suggested tags
    pub tags: Vec<String>,

    /// Category (docs, code, idea, task, research, etc)
    pub category: String,

    /// Importance/quality score (1-10)
    pub score: i32,

    /// Actionable items extracted
    pub action_items: Vec<String>,

    /// Related concepts/topics
    pub related_topics: Vec<String>,

    /// Suggested project association
    pub suggested_project: Option<String>,
}

/// Queue statistics
#[derive(Debug, Default, Serialize)]
pub struct QueueStats {
    pub inbox: i64,
    pub pending_analysis: i64,
    pub analyzing: i64,
    pub pending_tagging: i64,
    pub ready: i64,
    pub failed: i64,
    pub archived: i64,
}

impl QueueStats {
    pub fn total_pending(&self) -> i64 {
        self.inbox + self.pending_analysis + self.analyzing + self.pending_tagging
    }
}

// ============================================================================
// Queue Processor (Background Worker)
// ============================================================================

/// Background processor configuration
pub struct ProcessorConfig {
    /// How many items to process per batch
    pub batch_size: i32,

    /// Delay between batches (ms)
    pub batch_delay_ms: u64,

    /// Maximum retries before giving up
    pub max_retries: i32,

    /// Delay before retrying failed items (seconds)
    pub retry_delay_secs: u64,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            batch_delay_ms: 1000,
            max_retries: 3,
            retry_delay_secs: 300, // 5 minutes
        }
    }
}

/// The background queue processor
pub struct QueueProcessor {
    pool: SqlitePool,
    config: ProcessorConfig,
    llm_client: Box<dyn LlmAnalyzer + Send + Sync>,
}

/// Trait for LLM analysis (implement with your Grok client)
#[async_trait::async_trait]
pub trait LlmAnalyzer {
    async fn analyze_content(&self, content: &str, source: &str) -> Result<AnalysisResult>;
    async fn analyze_file(
        &self,
        content: &str,
        file_path: &str,
        language: &str,
    ) -> Result<FileAnalysisResult>;
}

/// File-specific analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysisResult {
    pub summary: String,
    pub purpose: String,
    pub language: String,
    pub complexity_score: i32,
    pub quality_score: i32,
    pub security_notes: Vec<String>,
    pub improvements: Vec<String>,
    pub dependencies: Vec<String>,
    pub exports: Vec<String>,
    pub tags: Vec<String>,
    pub needs_attention: bool,
    pub tokens_used: Option<usize>,
}

impl QueueProcessor {
    pub fn new(
        pool: SqlitePool,
        config: ProcessorConfig,
        llm_client: Box<dyn LlmAnalyzer + Send + Sync>,
    ) -> Self {
        Self {
            pool,
            config,
            llm_client,
        }
    }

    /// Run the processor loop
    pub async fn run(&self) -> Result<()> {
        info!("Queue processor started");

        loop {
            // Process inbox items (move to pending_analysis)
            self.process_inbox().await?;

            // Process pending analysis items
            self.process_analysis().await?;

            // Process pending tagging items
            self.process_tagging().await?;

            // Retry failed items
            self.retry_failed().await?;

            // Brief pause between cycles
            sleep(Duration::from_millis(self.config.batch_delay_ms)).await;
        }
    }

    /// Move inbox items to pending_analysis
    async fn process_inbox(&self) -> Result<()> {
        let items =
            get_pending_items(&self.pool, QueueStage::Inbox, self.config.batch_size).await?;

        for item in items {
            // Simple validation - if content is too short, skip
            if item.content.trim().len() < 5 {
                mark_failed(&self.pool, &item.id, "Content too short").await?;
                continue;
            }

            advance_stage(&self.pool, &item.id).await?;
        }

        Ok(())
    }

    /// Run LLM analysis on pending items
    async fn process_analysis(&self) -> Result<()> {
        let items = get_pending_items(
            &self.pool,
            QueueStage::PendingAnalysis,
            self.config.batch_size,
        )
        .await?;

        for item in items {
            // Mark as analyzing
            advance_stage(&self.pool, &item.id).await?;

            // Run LLM analysis
            match self
                .llm_client
                .analyze_content(&item.content, &item.source)
                .await
            {
                Ok(analysis) => {
                    update_analysis(&self.pool, &item.id, &analysis).await?;
                    info!(
                        "Analyzed item {}: category={}, score={}",
                        item.id, analysis.category, analysis.score
                    );
                }
                Err(e) => {
                    mark_failed(&self.pool, &item.id, &e.to_string()).await?;
                }
            }
        }

        Ok(())
    }

    /// Finalize tagging and move to ready
    async fn process_tagging(&self) -> Result<()> {
        let items = get_pending_items(
            &self.pool,
            QueueStage::PendingTagging,
            self.config.batch_size,
        )
        .await?;

        for item in items {
            // TODO: Additional tag refinement, linking to projects, etc.
            // For now, just advance to ready
            advance_stage(&self.pool, &item.id).await?;
            info!("Item {} is now ready", item.id);
        }

        Ok(())
    }

    /// Retry failed items
    async fn retry_failed(&self) -> Result<()> {
        let items = get_retriable_items(&self.pool, self.config.max_retries).await?;

        for item in items {
            info!(
                "Retrying failed item {} (attempt {})",
                item.id,
                item.retry_count + 1
            );
            advance_stage(&self.pool, &item.id).await?; // Moves back to pending_analysis
        }

        Ok(())
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_stage(s: &str) -> QueueStage {
    match s {
        "inbox" => QueueStage::Inbox,
        "pending_analysis" => QueueStage::PendingAnalysis,
        "analyzing" => QueueStage::Analyzing,
        "pending_tagging" => QueueStage::PendingTagging,
        "ready" => QueueStage::Ready,
        "failed" => QueueStage::Failed,
        "archived" => QueueStage::Archived,
        _ => QueueStage::Inbox,
    }
}

// ============================================================================
// Quick Capture Functions
// ============================================================================

/// Quick capture for random thoughts
pub async fn capture_thought(pool: &SqlitePool, text: &str) -> Result<QueueItem> {
    enqueue(
        pool,
        text,
        QueueSource::RawThought,
        QueuePriority::Normal,
        None,
        None,
        None,
    )
    .await
}

/// Quick capture for notes
pub async fn capture_note(
    pool: &SqlitePool,
    text: &str,
    project: Option<&str>,
) -> Result<QueueItem> {
    // If project specified, try to find matching repo
    let repo_id = if let Some(p) = project {
        sqlx::query_as::<_, (String,)>("SELECT id FROM repositories WHERE name = ?")
            .bind(p)
            .fetch_optional(pool)
            .await?
            .map(|(id,)| id)
    } else {
        None
    };

    enqueue(
        pool,
        text,
        QueueSource::Note,
        QueuePriority::Normal,
        repo_id.as_deref(),
        None,
        None,
    )
    .await
}

/// Capture a TODO found in code
pub async fn capture_todo(
    pool: &SqlitePool,
    content: &str,
    repo_id: &str,
    file_path: &str,
    line_number: i32,
) -> Result<QueueItem> {
    enqueue(
        pool,
        content,
        QueueSource::TodoComment,
        QueuePriority::High, // TODOs get higher priority
        Some(repo_id),
        Some(file_path),
        Some(line_number),
    )
    .await
}
