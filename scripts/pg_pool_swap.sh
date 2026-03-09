#!/bin/sh
# pg_pool_swap.sh
# Swaps SqlitePool → PgPool (and related SQLite-specific sqlx identifiers)
# across all Rust source files that reference them.
#
# Usage:
#   chmod +x scripts/pg_pool_swap.sh
#   cd /path/to/rustassistant
#   ./scripts/pg_pool_swap.sh

set -e

FILES="
src/api/handlers.rs
src/api/jobs.rs
src/api/mod.rs
src/auto_scanner.rs
src/bin/cli.rs
src/bin/server.rs
src/cli/github_commands.rs
src/cli/queue_commands.rs
src/cli/research_backup_commands.rs
src/cli/task_commands.rs
src/cost_tracker.rs
src/db/chunks.rs
src/db/config.rs
src/db/documents.rs
src/db/queue.rs
src/db/scan_events.rs
src/github/background_sync.rs
src/github/mod.rs
src/github/search.rs
src/github/sync.rs
src/indexing.rs
src/multi_tenant.rs
src/query_analytics.rs
src/query_router.rs
src/queue/processor.rs
src/repo_cache_sql.rs
src/repo_sync.rs
src/research/mod.rs
src/research/worker.rs
src/response_cache.rs
src/scanner/github.rs
src/search.rs
src/server.rs
src/task/models.rs
src/web_api.rs
src/web_ui_db_explorer.rs
src/web_ui_scan_progress.rs
"

echo "Swapping SqlitePool -> PgPool in Rust source files..."

for f in $FILES; do
    if [ ! -f "$f" ]; then
        echo "  SKIP (not found): $f"
        continue
    fi

    sed \
        -e 's/use sqlx::SqlitePool;/use sqlx::PgPool;/g' \
        -e 's/use sqlx::{SqlitePool}/use sqlx::{PgPool}/g' \
        -e 's/use sqlx::{SqlitePool,/use sqlx::{PgPool,/g' \
        -e 's/, SqlitePool}/, PgPool}/g' \
        -e 's/, SqlitePool,/, PgPool,/g' \
        -e 's/sqlx::SqlitePool/sqlx::PgPool/g' \
        -e 's/: SqlitePool/: PgPool/g' \
        -e 's/<SqlitePool>/<PgPool>/g' \
        -e 's/SqlitePool,/PgPool,/g' \
        -e 's/SqlitePool)/PgPool)/g' \
        -e 's/SqlitePool;/PgPool;/g' \
        -e 's/SqlitePool {/PgPool {/g' \
        -e 's/SqlitePool$/PgPool/g' \
        -e 's/sqlite::SqlitePoolOptions/postgres::PgPoolOptions/g' \
        -e 's/sqlite::SqliteConnectOptions/postgres::PgConnectOptions/g' \
        -e 's/sqlite::SqliteJournalMode/postgres::PgJournalMode/g' \
        -e 's/SqlitePoolOptions/PgPoolOptions/g' \
        -e 's/SqliteConnectOptions/PgConnectOptions/g' \
        -e 's/SqliteSynchronous/PgSynchronous/g' \
        -e 's/sqlite::SqliteQueryResult/postgres::PgQueryResult/g' \
        -e 's/sqlx::sqlite::SqliteQueryResult/sqlx::postgres::PgQueryResult/g' \
        -e 's/Result<sqlx::sqlite::SqliteQueryResult/Result<sqlx::postgres::PgQueryResult/g' \
        -e 's/Result<sqlx::sqlite::/Result<sqlx::postgres::/g' \
        -e 's/sqlite:\/\//postgresql:\/\//g' \
        -e 's|sqlite:data/|postgresql://rustassistant:changeme@localhost:5432/|g' \
        -e 's/SQLX_OFFLINE=true //' \
        "$f" > "${f}.pg_tmp" && mv "${f}.pg_tmp" "$f"

    echo "  OK: $f"
done

echo ""
echo "Also patching db/core.rs test helper for Postgres URL..."
sed \
    -e 's/init_db("sqlite::memory:")/init_db(\&std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:\/\/rustassistant:changeme@localhost:5432\/rustassistant_test".to_string()))/g' \
    -e 's/SqlitePool::connect("sqlite::memory:")/PgPool::connect(\&std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:\/\/rustassistant:changeme@localhost:5432\/rustassistant_test".to_string()))/g' \
    src/db/core.rs > src/db/core.rs.pg_tmp && mv src/db/core.rs.pg_tmp src/db/core.rs

echo ""
echo "Patching db/chunks.rs test pool helper..."
sed \
    -e 's/SqlitePool::connect("sqlite::memory:")/PgPool::connect(\&std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:\/\/rustassistant:changeme@localhost:5432\/rustassistant_test".to_string()))/g' \
    src/db/chunks.rs > src/db/chunks.rs.pg_tmp && mv src/db/chunks.rs.pg_tmp src/db/chunks.rs

echo ""
echo "Patching cost_tracker.rs test pool helper..."
sed \
    -e 's/crate::db::core::init_db("sqlite::memory:")/crate::db::core::init_db(\&std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:\/\/rustassistant:changeme@localhost:5432\/rustassistant_test".to_string()))/g' \
    src/cost_tracker.rs > src/cost_tracker.rs.pg_tmp && mv src/cost_tracker.rs.pg_tmp src/cost_tracker.rs

echo ""
echo "All done. Next steps:"
echo "  1. Start Postgres:  docker compose up -d postgres"
echo "  2. Run migrations:  cargo sqlx database create && cargo sqlx migrate run"
echo "  3. Regen cache:     cargo sqlx prepare"
echo "  4. Build:           cargo build"
