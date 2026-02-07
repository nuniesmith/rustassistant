//! Multi-Tenancy Module
//!
//! Provides organization isolation for SaaS deployments.
//! Supports tenant quotas, resource limits, and usage tracking.
//!
//! # Features
//!
//! - **Tenant Isolation**: Complete data separation per organization
//! - **Resource Quotas**: Configurable limits on documents, searches, storage
//! - **Usage Tracking**: Monitor resource consumption per tenant
//! - **Billing Metrics**: Track usage for invoicing
//! - **Custom Domains**: Support for white-label deployments
//!
//! # Example
//!
//! ```rust,no_run
//! use rustassistant::multi_tenant::{TenantManager, TenantQuota};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let tenant_mgr = TenantManager::new(db_pool).await?;
//!
//! // Create new tenant
//! let tenant = tenant_mgr.create_tenant(
//!     "acme-corp",
//!     "ACME Corporation",
//!     TenantQuota::standard()
//! ).await?;
//!
//! // Check quota before operation
//! tenant_mgr.check_quota(&tenant.id, QuotaType::Documents).await?;
//!
//! // Track usage
//! tenant_mgr.increment_usage(&tenant.id, UsageMetric::Documents(1)).await?;
//! # Ok(())
//! # }
//! ```

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;

// ============================================================================
// Data Structures
// ============================================================================

/// Tenant/Organization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub quota: TenantQuota,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub custom_domain: Option<String>,
}

/// Resource quotas for a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantQuota {
    pub max_documents: i64,
    pub max_storage_mb: i64,
    pub max_searches_per_day: i64,
    pub max_api_keys: i32,
    pub max_webhooks: i32,
}

impl Default for TenantQuota {
    fn default() -> Self {
        Self::standard()
    }
}

impl TenantQuota {
    /// Free tier quota
    pub fn free() -> Self {
        Self {
            max_documents: 100,
            max_storage_mb: 100,
            max_searches_per_day: 1000,
            max_api_keys: 2,
            max_webhooks: 1,
        }
    }

    /// Standard paid tier
    pub fn standard() -> Self {
        Self {
            max_documents: 10000,
            max_storage_mb: 10240,
            max_searches_per_day: 100000,
            max_api_keys: 10,
            max_webhooks: 5,
        }
    }

    /// Enterprise tier
    pub fn enterprise() -> Self {
        Self {
            max_documents: 1000000,
            max_storage_mb: 1048576,
            max_searches_per_day: 10000000,
            max_api_keys: 100,
            max_webhooks: 50,
        }
    }

    /// Unlimited (for internal use)
    pub fn unlimited() -> Self {
        Self {
            max_documents: i64::MAX,
            max_storage_mb: i64::MAX,
            max_searches_per_day: i64::MAX,
            max_api_keys: i32::MAX,
            max_webhooks: i32::MAX,
        }
    }
}

/// Current usage metrics for a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantUsage {
    pub tenant_id: String,
    pub document_count: i64,
    pub storage_mb: i64,
    pub searches_today: i64,
    pub api_key_count: i32,
    pub webhook_count: i32,
    pub last_updated: DateTime<Utc>,
}

/// Usage metric types
#[derive(Debug, Clone)]
pub enum UsageMetric {
    Documents(i64),
    StorageMb(i64),
    Searches(i64),
    ApiKeys(i32),
    Webhooks(i32),
}

/// Quota check result
#[derive(Debug, Clone)]
pub enum QuotaType {
    Documents,
    Storage,
    SearchesPerDay,
    ApiKeys,
    Webhooks,
}

// ============================================================================
// Tenant Manager
// ============================================================================

pub struct TenantManager {
    db_pool: SqlitePool,
}

impl TenantManager {
    /// Create new tenant manager
    pub async fn new(db_pool: SqlitePool) -> Result<Self> {
        let manager = Self { db_pool };
        manager.init_tables().await?;
        Ok(manager)
    }

    /// Initialize database tables
    async fn init_tables(&self) -> Result<()> {
        // Organizations table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS organizations (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                slug TEXT UNIQUE NOT NULL,
                max_documents INTEGER NOT NULL DEFAULT 10000,
                max_storage_mb INTEGER NOT NULL DEFAULT 10240,
                max_searches_per_day INTEGER NOT NULL DEFAULT 100000,
                max_api_keys INTEGER NOT NULL DEFAULT 10,
                max_webhooks INTEGER NOT NULL DEFAULT 5,
                enabled BOOLEAN NOT NULL DEFAULT 1,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                custom_domain TEXT
            )
            "#,
        )
        .execute(&self.db_pool)
        .await
        .context("Failed to create organizations table")?;

        // Usage tracking table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tenant_usage (
                tenant_id TEXT PRIMARY KEY,
                document_count INTEGER NOT NULL DEFAULT 0,
                storage_mb INTEGER NOT NULL DEFAULT 0,
                searches_today INTEGER NOT NULL DEFAULT 0,
                api_key_count INTEGER NOT NULL DEFAULT 0,
                webhook_count INTEGER NOT NULL DEFAULT 0,
                last_updated DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (tenant_id) REFERENCES organizations(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.db_pool)
        .await
        .context("Failed to create tenant_usage table")?;

        // Daily usage history
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tenant_usage_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tenant_id TEXT NOT NULL,
                date DATE NOT NULL,
                documents_created INTEGER NOT NULL DEFAULT 0,
                searches_performed INTEGER NOT NULL DEFAULT 0,
                storage_mb INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (tenant_id) REFERENCES organizations(id) ON DELETE CASCADE,
                UNIQUE(tenant_id, date)
            )
            "#,
        )
        .execute(&self.db_pool)
        .await
        .context("Failed to create tenant_usage_history table")?;

        Ok(())
    }

    /// Create new tenant
    pub async fn create_tenant(
        &self,
        slug: &str,
        name: &str,
        quota: TenantQuota,
    ) -> Result<Tenant> {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO organizations (
                id, name, slug, max_documents, max_storage_mb, max_searches_per_day,
                max_api_keys, max_webhooks, created_at, enabled
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(&id)
        .bind(name)
        .bind(slug)
        .bind(quota.max_documents)
        .bind(quota.max_storage_mb)
        .bind(quota.max_searches_per_day)
        .bind(quota.max_api_keys)
        .bind(quota.max_webhooks)
        .bind(created_at)
        .execute(&self.db_pool)
        .await
        .context("Failed to create tenant")?;

        // Initialize usage tracking
        sqlx::query(
            r#"
            INSERT INTO tenant_usage (tenant_id, last_updated)
            VALUES (?, ?)
            "#,
        )
        .bind(&id)
        .bind(created_at)
        .execute(&self.db_pool)
        .await?;

        Ok(Tenant {
            id,
            name: name.to_string(),
            slug: slug.to_string(),
            quota,
            enabled: true,
            created_at,
            custom_domain: None,
        })
    }

    /// Get tenant by ID
    pub async fn get_tenant(&self, tenant_id: &str) -> Result<Option<Tenant>> {
        let row = sqlx::query_as::<_, (
            String,
            String,
            String,
            i64,
            i64,
            i64,
            i32,
            i32,
            bool,
            String,
            Option<String>,
        )>(
            r#"
            SELECT id, name, slug, max_documents, max_storage_mb, max_searches_per_day,
                   max_api_keys, max_webhooks, enabled, created_at, custom_domain
            FROM organizations
            WHERE id = ?
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(&self.db_pool)
        .await?;

        if let Some((
            id,
            name,
            slug,
            max_docs,
            max_storage,
            max_searches,
            max_keys,
            max_hooks,
            enabled,
            created,
            domain,
        )) = row
        {
            Ok(Some(Tenant {
                id,
                name,
                slug,
                quota: TenantQuota {
                    max_documents: max_docs,
                    max_storage_mb: max_storage,
                    max_searches_per_day: max_searches,
                    max_api_keys: max_keys,
                    max_webhooks: max_hooks,
                },
                enabled,
                created_at: created.parse().unwrap_or(Utc::now()),
                custom_domain: domain,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get tenant by slug
    pub async fn get_tenant_by_slug(&self, slug: &str) -> Result<Option<Tenant>> {
        let row = sqlx::query_scalar::<_, String>(
            "SELECT id FROM organizations WHERE slug = ?",
        )
        .bind(slug)
        .fetch_optional(&self.db_pool)
        .await?;

        if let Some(id) = row {
            self.get_tenant(&id).await
        } else {
            Ok(None)
        }
    }

    /// Get tenant by API key
    pub async fn get_tenant_by_key(&self, api_key_hash: &str) -> Result<Option<Tenant>> {
        let tenant_id = sqlx::query_scalar::<_, String>(
            "SELECT tenant_id FROM api_keys WHERE key_hash = ?",
        )
        .bind(api_key_hash)
        .fetch_optional(&self.db_pool)
        .await?;

        if let Some(id) = tenant_id {
            self.get_tenant(&id).await
        } else {
            Ok(None)
        }
    }

    /// Get current usage for tenant
    pub async fn get_usage(&self, tenant_id: &str) -> Result<TenantUsage> {
        let row = sqlx::query_as::<_, (i64, i64, i64, i32, i32, String)>(
            r#"
            SELECT document_count, storage_mb, searches_today, api_key_count, webhook_count, last_updated
            FROM tenant_usage
            WHERE tenant_id = ?
            "#,
        )
        .bind(tenant_id)
        .fetch_one(&self.db_pool)
        .await
        .context("Failed to fetch tenant usage")?;

        Ok(TenantUsage {
            tenant_id: tenant_id.to_string(),
            document_count: row.0,
            storage_mb: row.1,
            searches_today: row.2,
            api_key_count: row.3,
            webhook_count: row.4,
            last_updated: row.5.parse().unwrap_or(Utc::now()),
        })
    }

    /// Check if operation would exceed quota
    pub async fn check_quota(&self, tenant_id: &str, quota_type: QuotaType) -> Result<()> {
        let tenant = self
            .get_tenant(tenant_id)
            .await?
            .ok_or_else(|| anyhow!("Tenant not found"))?;

        if !tenant.enabled {
            return Err(anyhow!("Tenant is disabled"));
        }

        let usage = self.get_usage(tenant_id).await?;

        match quota_type {
            QuotaType::Documents => {
                if usage.document_count >= tenant.quota.max_documents {
                    return Err(anyhow!(
                        "Document quota exceeded ({}/{})",
                        usage.document_count,
                        tenant.quota.max_documents
                    ));
                }
            }
            QuotaType::Storage => {
                if usage.storage_mb >= tenant.quota.max_storage_mb {
                    return Err(anyhow!(
                        "Storage quota exceeded ({} MB / {} MB)",
                        usage.storage_mb,
                        tenant.quota.max_storage_mb
                    ));
                }
            }
            QuotaType::SearchesPerDay => {
                if usage.searches_today >= tenant.quota.max_searches_per_day {
                    return Err(anyhow!(
                        "Daily search quota exceeded ({}/{})",
                        usage.searches_today,
                        tenant.quota.max_searches_per_day
                    ));
                }
            }
            QuotaType::ApiKeys => {
                if usage.api_key_count >= tenant.quota.max_api_keys {
                    return Err(anyhow!(
                        "API key quota exceeded ({}/{})",
                        usage.api_key_count,
                        tenant.quota.max_api_keys
                    ));
                }
            }
            QuotaType::Webhooks => {
                if usage.webhook_count >= tenant.quota.max_webhooks {
                    return Err(anyhow!(
                        "Webhook quota exceeded ({}/{})",
                        usage.webhook_count,
                        tenant.quota.max_webhooks
                    ));
                }
            }
        }

        Ok(())
    }

    /// Increment usage counter
    pub async fn increment_usage(&self, tenant_id: &str, metric: UsageMetric) -> Result<()> {
        let (field, value) = match metric {
            UsageMetric::Documents(n) => ("document_count", n),
            UsageMetric::StorageMb(n) => ("storage_mb", n),
            UsageMetric::Searches(n) => ("searches_today", n),
            UsageMetric::ApiKeys(n) => ("api_key_count", n as i64),
            UsageMetric::Webhooks(n) => ("webhook_count", n as i64),
        };

        let query = format!(
            "UPDATE tenant_usage SET {} = {} + ?, last_updated = ? WHERE tenant_id = ?",
            field, field
        );

        sqlx::query(&query)
            .bind(value)
            .bind(Utc::now().to_rfc3339())
            .bind(tenant_id)
            .execute(&self.db_pool)
            .await?;

        Ok(())
    }

    /// Decrement usage counter
    pub async fn decrement_usage(&self, tenant_id: &str, metric: UsageMetric) -> Result<()> {
        let (field, value) = match metric {
            UsageMetric::Documents(n) => ("document_count", n),
            UsageMetric::StorageMb(n) => ("storage_mb", n),
            UsageMetric::Searches(n) => ("searches_today", n),
            UsageMetric::ApiKeys(n) => ("api_key_count", n as i64),
            UsageMetric::Webhooks(n) => ("webhook_count", n as i64),
        };

        let query = format!(
            "UPDATE tenant_usage SET {} = MAX(0, {} - ?), last_updated = ? WHERE tenant_id = ?",
            field, field
        );

        sqlx::query(&query)
            .bind(value)
            .bind(Utc::now().to_rfc3339())
            .bind(tenant_id)
            .execute(&self.db_pool)
            .await?;

        Ok(())
    }

    /// Reset daily search counter (call this daily)
    pub async fn reset_daily_searches(&self) -> Result<u64> {
        let result = sqlx::query("UPDATE tenant_usage SET searches_today = 0, last_updated = ?")
            .bind(Utc::now().to_rfc3339())
            .execute(&self.db_pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Update tenant quota
    pub async fn update_quota(&self, tenant_id: &str, quota: TenantQuota) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE organizations
            SET max_documents = ?, max_storage_mb = ?, max_searches_per_day = ?,
                max_api_keys = ?, max_webhooks = ?
            WHERE id = ?
            "#,
        )
        .bind(quota.max_documents)
        .bind(quota.max_storage_mb)
        .bind(quota.max_searches_per_day)
        .bind(quota.max_api_keys)
        .bind(quota.max_webhooks)
        .bind(tenant_id)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Enable/disable tenant
    pub async fn set_tenant_enabled(&self, tenant_id: &str, enabled: bool) -> Result<()> {
        sqlx::query("UPDATE organizations SET enabled = ? WHERE id = ?")
            .bind(enabled)
            .bind(tenant_id)
            .execute(&self.db_pool)
            .await?;

        Ok(())
    }

    /// List all tenants
    pub async fn list_tenants(&self) -> Result<Vec<Tenant>> {
        let rows = sqlx::query_as::<_, (
            String,
            String,
            String,
            i64,
            i64,
            i64,
            i32,
            i32,
            bool,
            String,
            Option<String>,
        )>(
            r#"
            SELECT id, name, slug, max_documents, max_storage_mb, max_searches_per_day,
                   max_api_keys, max_webhooks, enabled, created_at, custom_domain
            FROM organizations
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.db_pool)
        .await?;

        let mut tenants = Vec::new();
        for (id, name, slug, max_docs, max_storage, max_searches, max_keys, max_hooks, enabled, created, domain) in rows {
            tenants.push(Tenant {
                id,
                name,
                slug,
                quota: TenantQuota {
                    max_documents: max_docs,
                    max_storage_mb: max_storage,
                    max_searches_per_day: max_searches,
                    max_api_keys: max_keys,
                    max_webhooks: max_hooks,
                },
                enabled,
                created_at: created.parse().unwrap_or(Utc::now()),
                custom_domain: domain,
            });
        }

        Ok(tenants)
    }

    /// Get billing metrics for tenant
    pub async fn get_billing_metrics(
        &self,
        tenant_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<HashMap<String, i64>> {
        let rows = sqlx::query_as::<_, (i64, i64, i64)>(
            r#"
            SELECT
                SUM(documents_created) as total_documents,
                SUM(searches_performed) as total_searches,
                AVG(storage_mb) as avg_storage
            FROM tenant_usage_history
            WHERE tenant_id = ? AND date BETWEEN ? AND ?
            "#,
        )
        .bind(tenant_id)
        .bind(start.format("%Y-%m-%d").to_string())
        .bind(end.format("%Y-%m-%d").to_string())
        .fetch_one(&self.db_pool)
        .await?;

        let mut metrics = HashMap::new();
        metrics.insert("total_documents".to_string(), rows.0);
        metrics.insert("total_searches".to_string(), rows.1);
        metrics.insert("avg_storage_mb".to_string(), rows.2);

        Ok(metrics)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> SqlitePool {
        SqlitePool::connect(":memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_create_tenant() {
        let pool = setup_test_db().await;
        let manager = TenantManager::new(pool).await.unwrap();

        let tenant = manager
            .create_tenant("test-org", "Test Organization", TenantQuota::standard())
            .await
            .unwrap();

        assert_eq!(tenant.slug, "test-org");
        assert_eq!(tenant.name, "Test Organization");
        assert!(tenant.enabled);
    }

    #[tokio::test]
    async fn test_quota_check() {
        let pool = setup_test_db().await;
        let manager = TenantManager::new(pool).await.unwrap();

        let tenant = manager
            .create_tenant("test-org", "Test Org", TenantQuota::free())
            .await
            .unwrap();

        // Should pass initially
        manager
            .check_quota(&tenant.id, QuotaType::Documents)
            .await
            .unwrap();

        // Increment to limit
        for _ in 0..100 {
            manager
                .increment_usage(&tenant.id, UsageMetric::Documents(1))
                .await
                .unwrap();
        }

        // Should now fail
        let result = manager.check_quota(&tenant.id, QuotaType::Documents).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_usage_tracking() {
        let pool = setup_test_db().await;
        let manager = TenantManager::new(pool).await.unwrap();

        let tenant = manager
            .create_tenant("test-org", "Test Org", TenantQuota::standard())
            .await
            .unwrap();

        // Increment documents
        manager
            .increment_usage(&tenant.id, UsageMetric::Documents(5))
            .await
            .unwrap();

        // Increment searches
        manager
            .increment_usage(&tenant.id, UsageMetric::Searches(10))
            .await
            .unwrap();

        let usage = manager.get_usage(&tenant.id).await.unwrap();
        assert_eq!(usage.document_count, 5);
        assert_eq!(usage.searches_today, 10);

        // Decrement
        manager
            .decrement_usage(&tenant.id, UsageMetric::Documents(2))
            .await
            .unwrap();

        let usage = manager.get_usage(&tenant.id).await.unwrap();
        assert_eq!(usage.document_count, 3);
    }
}
