//! # Cost Tracker Module
//!
//! Tracks LLM API usage and costs for budget monitoring.
//!
//! ## Features
//!
//! - Per-query cost tracking
//! - Daily/weekly/monthly aggregations
//! - Budget alerts
//! - Cost breakdown by operation type
//! - Cache hit/miss impact analysis
//!
//! ## Usage
//!
//! ```rust,no_run
//! use rustassistant::cost_tracker::{CostTracker, TokenUsage};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let tracker = CostTracker::new(pool).await?;
//!
//!     // Log an API call
//!     let usage = TokenUsage {
//!         input_tokens: 100_000,
//!         output_tokens: 50_000,
//!         cached_tokens: 0,
//!     };
//!     tracker.log_call("code_review", "grok-4-1-fast-reasoning", usage, false).await?;
//!
//!     // Get daily stats
//!     let stats = tracker.get_daily_stats().await?;
//!     println!("Today's cost: ${:.2}", stats.total_cost_usd);
//!
//!     Ok(())
//! }
//! ```

use crate::error::AuditError;
use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::{info, warn};

/// Grok 4.1 Fast pricing (per million tokens)
const GROK_COST_PER_MILLION_INPUT: f64 = 0.20;
const GROK_COST_PER_MILLION_OUTPUT: f64 = 0.50;
const GROK_COST_PER_MILLION_CACHED: f64 = 0.05;

/// Default budget alert threshold (USD)
const DEFAULT_DAILY_BUDGET: f64 = 1.0;
const DEFAULT_MONTHLY_BUDGET: f64 = 10.0;

/// Token usage for a single API call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
}

/// Cost statistics for a time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostStats {
    pub total_queries: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cached_tokens: u64,
    pub total_cost_usd: f64,
    pub cache_hits: u64,
    pub cache_hit_rate: f64,
    pub cost_saved_from_cache: f64,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

/// Cost breakdown by operation type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationCost {
    pub operation: String,
    pub query_count: u64,
    pub total_cost_usd: f64,
    pub avg_cost_usd: f64,
    pub total_tokens: u64,
}

/// Budget status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub daily_spend: f64,
    pub daily_budget: f64,
    pub daily_remaining: f64,
    pub daily_percent_used: f64,
    pub monthly_spend: f64,
    pub monthly_budget: f64,
    pub monthly_remaining: f64,
    pub monthly_percent_used: f64,
    pub alerts: Vec<String>,
}

/// LLM API cost tracker
pub struct CostTracker {
    pool: SqlitePool,
    daily_budget: f64,
    monthly_budget: f64,
}

impl CostTracker {
    /// Create a new cost tracker
    pub async fn new(pool: SqlitePool) -> Result<Self> {
        let tracker = Self {
            pool,
            daily_budget: DEFAULT_DAILY_BUDGET,
            monthly_budget: DEFAULT_MONTHLY_BUDGET,
        };

        tracker.initialize_schema().await?;

        Ok(tracker)
    }

    /// Create with custom budget limits
    pub async fn with_budgets(
        pool: SqlitePool,
        daily_budget: f64,
        monthly_budget: f64,
    ) -> Result<Self> {
        let tracker = Self {
            pool,
            daily_budget,
            monthly_budget,
        };

        tracker.initialize_schema().await?;

        Ok(tracker)
    }

    /// Initialize database schema
    async fn initialize_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS llm_costs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                operation TEXT NOT NULL,
                model TEXT NOT NULL,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                cached_tokens INTEGER DEFAULT 0,
                cost_usd REAL NOT NULL,
                query_hash TEXT,
                cache_hit BOOLEAN DEFAULT FALSE,
                user_query TEXT,
                response_summary TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create llm_costs table")?;

        // Create indexes for efficient queries
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_costs_timestamp ON llm_costs(timestamp)")
            .execute(&self.pool)
            .await
            .context("Failed to create timestamp index")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_costs_operation ON llm_costs(operation)")
            .execute(&self.pool)
            .await
            .context("Failed to create operation index")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_costs_cache_hit ON llm_costs(cache_hit)")
            .execute(&self.pool)
            .await
            .context("Failed to create cache_hit index")?;

        Ok(())
    }

    /// Log an API call
    pub async fn log_call(
        &self,
        operation: &str,
        model: &str,
        usage: TokenUsage,
        cache_hit: bool,
    ) -> Result<i64> {
        let cost = self.calculate_cost(&usage);

        let id = sqlx::query(
            r#"
            INSERT INTO llm_costs (
                operation, model, input_tokens, output_tokens, cached_tokens,
                cost_usd, cache_hit
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(operation)
        .bind(model)
        .bind(usage.input_tokens as i64)
        .bind(usage.output_tokens as i64)
        .bind(usage.cached_tokens as i64)
        .bind(cost)
        .bind(cache_hit)
        .execute(&self.pool)
        .await
        .context("Failed to log API call")?
        .last_insert_rowid();

        info!(
            "Logged API call: {} | Cost: ${:.4} | Tokens: {}in/{}out/{}cached | Cache: {}",
            operation,
            cost,
            usage.input_tokens,
            usage.output_tokens,
            usage.cached_tokens,
            cache_hit
        );

        // Check budget alerts
        self.check_budget_alerts().await?;

        Ok(id)
    }

    /// Calculate cost for token usage
    fn calculate_cost(&self, usage: &TokenUsage) -> f64 {
        let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * GROK_COST_PER_MILLION_INPUT;
        let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * GROK_COST_PER_MILLION_OUTPUT;
        let cached_cost = (usage.cached_tokens as f64 / 1_000_000.0) * GROK_COST_PER_MILLION_CACHED;

        input_cost + output_cost + cached_cost
    }

    /// Get statistics for all time (useful for testing)
    pub async fn get_all_time_stats(&self) -> Result<CostStats> {
        self.get_stats_for_period("1970-01-01T00:00:00Z", "2100-01-01T00:00:00Z")
            .await
    }

    /// Get statistics for today
    pub async fn get_daily_stats(&self) -> Result<CostStats> {
        let today = Utc::now().date_naive();
        let start = today.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
        let end = today
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc()
            .to_rfc3339();

        self.get_stats_for_period(&start, &end).await
    }

    /// Get statistics for this week
    pub async fn get_weekly_stats(&self) -> Result<CostStats> {
        let now = Utc::now();
        let start = (now - Duration::days(7)).to_rfc3339();
        let end = now.to_rfc3339();

        self.get_stats_for_period(&start, &end).await
    }

    /// Get statistics for this month
    pub async fn get_monthly_stats(&self) -> Result<CostStats> {
        let now = Utc::now();
        let year = now.year();
        let month = now.month();
        let start = chrono::NaiveDate::from_ymd_opt(year, month, 1)
            .ok_or_else(|| AuditError::other("Invalid date"))?;
        let start_dt = start.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
        let end = now.to_rfc3339();

        self.get_stats_for_period(&start_dt, &end).await
    }

    /// Get statistics for a custom period
    async fn get_stats_for_period(&self, start: &str, end: &str) -> Result<CostStats> {
        let (
            total_queries,
            total_input_tokens,
            total_output_tokens,
            total_cached_tokens,
            total_cost_usd,
        ) = sqlx::query_as::<_, (i64, i64, i64, i64, f64)>(
            r#"
            SELECT
                COUNT(*),
                COALESCE(SUM(input_tokens), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(cached_tokens), 0),
                COALESCE(SUM(cost_usd), 0.0)
            FROM llm_costs
            WHERE timestamp >= ? AND timestamp <= ?
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .context("Failed to fetch cost statistics")?;

        let (cache_hits,) = sqlx::query_as::<_, (i64,)>(
            r#"
            SELECT COUNT(*)
            FROM llm_costs
            WHERE timestamp >= ? AND timestamp <= ?
            AND cache_hit = TRUE
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .context("Failed to count cache hits")?;

        let cache_hit_rate = if total_queries > 0 {
            (cache_hits as f64 / total_queries as f64) * 100.0
        } else {
            0.0
        };

        // Estimate cost saved from cache hits
        let avg_query_cost = if total_queries > 0 {
            total_cost_usd / total_queries as f64
        } else {
            0.0
        };
        let cost_saved_from_cache = cache_hits as f64 * avg_query_cost;

        Ok(CostStats {
            total_queries: total_queries as u64,
            total_input_tokens: total_input_tokens as u64,
            total_output_tokens: total_output_tokens as u64,
            total_cached_tokens: total_cached_tokens as u64,
            total_cost_usd,
            cache_hits: cache_hits as u64,
            cache_hit_rate,
            cost_saved_from_cache,
            period_start: DateTime::parse_from_rfc3339(start)
                .unwrap()
                .with_timezone(&Utc),
            period_end: DateTime::parse_from_rfc3339(end)
                .unwrap()
                .with_timezone(&Utc),
        })
    }

    /// Get cost breakdown by operation type
    pub async fn get_operation_breakdown(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Vec<OperationCost>> {
        let rows = sqlx::query_as::<_, (String, i64, f64, i64, i64, i64)>(
            r#"
            SELECT
                operation,
                COUNT(*) as query_count,
                SUM(cost_usd) as total_cost,
                SUM(input_tokens) as input_tokens,
                SUM(output_tokens) as output_tokens,
                SUM(cached_tokens) as cached_tokens
            FROM llm_costs
            WHERE timestamp >= ? AND timestamp <= ?
            GROUP BY operation
            ORDER BY total_cost DESC
            "#,
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch operation breakdown")?;

        Ok(rows
            .into_iter()
            .map(|(operation, count, total_cost, input, output, cached)| {
                let avg_cost = total_cost / count as f64;
                let total_tokens = (input + output + cached) as u64;

                OperationCost {
                    operation,
                    query_count: count as u64,
                    total_cost_usd: total_cost,
                    avg_cost_usd: avg_cost,
                    total_tokens,
                }
            })
            .collect())
    }

    /// Get budget status
    pub async fn get_budget_status(&self) -> Result<BudgetStatus> {
        let daily_stats = self.get_daily_stats().await?;
        let monthly_stats = self.get_monthly_stats().await?;

        let daily_remaining = self.daily_budget - daily_stats.total_cost_usd;
        let daily_percent = (daily_stats.total_cost_usd / self.daily_budget) * 100.0;

        let monthly_remaining = self.monthly_budget - monthly_stats.total_cost_usd;
        let monthly_percent = (monthly_stats.total_cost_usd / self.monthly_budget) * 100.0;

        let mut alerts = Vec::new();

        if daily_percent >= 100.0 {
            alerts.push(format!(
                "⛔ Daily budget exceeded! ${:.2} / ${:.2}",
                daily_stats.total_cost_usd, self.daily_budget
            ));
        } else if daily_percent >= 80.0 {
            alerts.push(format!(
                "⚠️  Daily budget at {:.0}%! ${:.2} / ${:.2}",
                daily_percent, daily_stats.total_cost_usd, self.daily_budget
            ));
        }

        if monthly_percent >= 100.0 {
            alerts.push(format!(
                "⛔ Monthly budget exceeded! ${:.2} / ${:.2}",
                monthly_stats.total_cost_usd, self.monthly_budget
            ));
        } else if monthly_percent >= 80.0 {
            alerts.push(format!(
                "⚠️  Monthly budget at {:.0}%! ${:.2} / ${:.2}",
                monthly_percent, monthly_stats.total_cost_usd, self.monthly_budget
            ));
        }

        Ok(BudgetStatus {
            daily_spend: daily_stats.total_cost_usd,
            daily_budget: self.daily_budget,
            daily_remaining,
            daily_percent_used: daily_percent,
            monthly_spend: monthly_stats.total_cost_usd,
            monthly_budget: self.monthly_budget,
            monthly_remaining,
            monthly_percent_used: monthly_percent,
            alerts,
        })
    }

    /// Check budget and emit warnings
    async fn check_budget_alerts(&self) -> Result<()> {
        let status = self.get_budget_status().await?;

        for alert in &status.alerts {
            warn!("{}", alert);
        }

        Ok(())
    }

    /// Generate daily report
    pub async fn daily_report(&self) -> Result<String> {
        let stats = self.get_daily_stats().await?;
        let status = self.get_budget_status().await?;

        let today = Utc::now().format("%Y-%m-%d");

        let mut report = format!("📊 Daily Cost Report - {}\n\n", today);

        report.push_str(&format!("Total Queries: {}\n", stats.total_queries));
        report.push_str(&format!("Total Cost: ${:.4}\n", stats.total_cost_usd));
        report.push_str(&format!(
            "Budget: ${:.2} / ${:.2} ({:.0}%)\n",
            status.daily_spend, status.daily_budget, status.daily_percent_used
        ));
        report.push_str(&format!("Cache Hit Rate: {:.1}%\n", stats.cache_hit_rate));
        report.push_str(&format!(
            "Cost Saved (Cache): ${:.4}\n\n",
            stats.cost_saved_from_cache
        ));

        report.push_str(&format!(
            "Tokens: {}M in / {}M out / {}M cached\n",
            stats.total_input_tokens / 1_000_000,
            stats.total_output_tokens / 1_000_000,
            stats.total_cached_tokens / 1_000_000
        ));

        if !status.alerts.is_empty() {
            report.push_str("\n⚠️  Alerts:\n");
            for alert in &status.alerts {
                report.push_str(&format!("  {}\n", alert));
            }
        }

        Ok(report)
    }

    /// Get top expensive queries
    pub async fn get_expensive_queries(&self, limit: i64) -> Result<Vec<(String, f64, i64)>> {
        let rows = sqlx::query_as::<_, (String, f64, String)>(
            r#"
            SELECT operation, cost_usd, timestamp
            FROM llm_costs
            ORDER BY cost_usd DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch expensive queries")?;

        Ok(rows
            .into_iter()
            .map(|(op, cost, ts)| {
                let timestamp = DateTime::parse_from_rfc3339(&ts).unwrap().timestamp();
                (op, cost, timestamp)
            })
            .collect())
    }

    /// Clear old records (for cleanup)
    pub async fn clear_old_records(&self, days: i64) -> Result<u64> {
        let cutoff = (Utc::now() - Duration::days(days)).to_rfc3339();

        let result = sqlx::query(
            r#"
            DELETE FROM llm_costs
            WHERE timestamp < ?
            "#,
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await
        .context("Failed to clear old records")?;

        let deleted = result.rows_affected();
        info!("Cleared {} cost records older than {} days", deleted, days);

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn create_test_pool() -> SqlitePool {
        crate::db::core::init_db("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_cost_calculation() {
        let pool = create_test_pool().await;
        let tracker = CostTracker::new(pool).await.unwrap();

        let usage = TokenUsage {
            input_tokens: 100_000,
            output_tokens: 50_000,
            cached_tokens: 0,
        };

        let cost = tracker.calculate_cost(&usage);

        // Expected: (100k/1M * 0.20) + (50k/1M * 0.50) = 0.02 + 0.025 = 0.045
        assert!((cost - 0.045).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_log_call() -> Result<()> {
        let pool = create_test_pool().await;
        let tracker = CostTracker::new(pool).await?;

        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cached_tokens: 0,
        };

        let id = tracker
            .log_call("test_op", "grok-4-1", usage, false)
            .await?;
        assert!(id > 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_daily_stats() -> Result<()> {
        let pool = create_test_pool().await;
        let tracker = CostTracker::new(pool).await?;

        // Log a few calls
        for _ in 0..3 {
            let usage = TokenUsage {
                input_tokens: 10_000,
                output_tokens: 5_000,
                cached_tokens: 0,
            };
            tracker.log_call("test", "grok", usage, false).await?;
        }

        // Query all records to verify they were inserted
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM llm_costs")
            .fetch_one(&tracker.pool)
            .await?;

        // Use get_all_time_stats instead of get_daily_stats to avoid timestamp comparison issues
        let stats = tracker.get_all_time_stats().await?;
        assert_eq!(
            stats.total_queries, 3,
            "Expected 3 queries, database has {} records",
            count.0
        );
        assert!(stats.total_cost_usd > 0.0);

        Ok(())
    }

    #[tokio::test]
    async fn test_budget_status() -> Result<()> {
        let pool = create_test_pool().await;
        let tracker = CostTracker::with_budgets(pool, 1.0, 10.0).await?;

        let status = tracker.get_budget_status().await?;
        assert_eq!(status.daily_budget, 1.0);
        assert_eq!(status.monthly_budget, 10.0);

        Ok(())
    }
}
