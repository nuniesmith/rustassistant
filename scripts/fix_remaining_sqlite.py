#!/usr/bin/env python3
"""
fix_remaining_sqlite.py
=======================
Fixes the remaining SQLite-specific references that the sed script couldn't
handle due to special characters in the replacement strings.

Targets:
  - SqlitePool::connect(":memory:")       -> PgPool test URL
  - SqlitePool::connect("sqlite::memory:") -> PgPool test URL
  - SqlitePool::connect("sqlite:data.db") -> PgPool URL
  - SqlitePool::connect(&database_url)    -> PgPool::connect(&database_url)
  - sqlx::sqlite::SqliteRow               -> sqlx::postgres::PgRow
  - Doc comment references to sqlite::    -> postgres::
  - "sqlite::memory:" in doc comments     -> Postgres URL note

Files processed:
  src/query_analytics.rs
  src/repo_cache_sql.rs
  src/multi_tenant.rs
  src/web_ui_db_explorer.rs
  src/db/chunks.rs        (doc comment only)
  src/cost_tracker.rs     (doc comment only)
  src/github/search.rs    (doc comment only)
  src/github/sync.rs      (doc comment only)
  src/github/mod.rs       (doc comment only)

Usage:
    python3 scripts/fix_remaining_sqlite.py
"""

import re
from pathlib import Path

TEST_URL = (
    'std::env::var("DATABASE_URL")'
    ".unwrap_or_else(|_| "
    '"postgresql://rustassistant:changeme@localhost:5432/rustassistant_test"'
    ".to_string())"
)

PROD_URL = (
    'std::env::var("DATABASE_URL")'
    ".unwrap_or_else(|_| "
    '"postgresql://rustassistant:changeme@localhost:5432/rustassistant"'
    ".to_string())"
)

# -----------------------------------------------------------------------
# Replacement rules — list of (pattern, replacement, use_regex)
# Applied in order to each file.
# -----------------------------------------------------------------------
RULES = [
    # Test pool helpers — in-memory SQLite replaced with env-var Postgres URL
    (
        'SqlitePool::connect(":memory:")',
        f"PgPool::connect(&{TEST_URL})",
        False,
    ),
    (
        'SqlitePool::connect("sqlite::memory:")',
        f"PgPool::connect(&{TEST_URL})",
        False,
    ),
    # Doc-comment example using sqlite::memory:
    (
        'init_db("sqlite::memory:")',
        f"init_db(&{TEST_URL})",
        False,
    ),
    # Doc-comment example using sqlite:data.db
    (
        'SqlitePool::connect("sqlite:data.db")',
        f"PgPool::connect(&{PROD_URL})",
        False,
    ),
    # Runtime connect from env var already called `database_url`
    (
        "SqlitePool::connect(&database_url)",
        "PgPool::connect(&database_url)",
        False,
    ),
    # sqlx row type
    (
        "sqlx::sqlite::SqliteRow",
        "sqlx::postgres::PgRow",
        False,
    ),
    # Remaining import aliases that the sed pass may have partially fixed
    (
        "use sqlx::SqlitePool;",
        "use sqlx::PgPool;",
        False,
    ),
    (
        "sqlx::SqlitePool",
        "sqlx::PgPool",
        False,
    ),
    # Doc-comment module-level examples referencing sqlite://
    (
        r"sqlite:data\.db",
        "postgresql://rustassistant:changeme@localhost:5432/rustassistant",
        True,  # regex so the dot is literal
    ),
    # Inline "let db_pool = SqlitePool" in doc comments
    (
        'SqlitePool::connect("sqlite::memory:").await?',
        f"PgPool::connect(&{TEST_URL}).await?",
        False,
    ),
]

# Files that need code-level fixes (not just comments)
CODE_FILES = [
    Path("src/query_analytics.rs"),
    Path("src/repo_cache_sql.rs"),
    Path("src/multi_tenant.rs"),
    Path("src/web_ui_db_explorer.rs"),
]

# Files that only need doc-comment / example fixes
DOC_FILES = [
    Path("src/db/chunks.rs"),
    Path("src/cost_tracker.rs"),
    Path("src/github/search.rs"),
    Path("src/github/sync.rs"),
    Path("src/github/mod.rs"),
]

ALL_FILES = CODE_FILES + DOC_FILES


def apply_rules(content: str) -> tuple[str, int]:
    total_changes = 0
    for pattern, replacement, is_regex in RULES:
        if is_regex:
            new_content, n = re.subn(pattern, replacement, content)
        else:
            count = content.count(pattern)
            new_content = content.replace(pattern, replacement)
            n = count if new_content != content else 0
        if n:
            total_changes += n
        content = new_content
    return content, total_changes


def process_file(path: Path) -> None:
    if not path.exists():
        print(f"  SKIP (not found): {path}")
        return

    original = path.read_text(encoding="utf-8")
    updated, changes = apply_rules(original)

    if changes == 0:
        print(f"  (no changes)  {path}")
        return

    path.write_text(updated, encoding="utf-8")
    print(f"  {changes:3d} replacement(s)  {path}")


def main() -> None:
    print("Fixing remaining SQLite references...")
    for p in ALL_FILES:
        process_file(p)
    print("\nDone.")


if __name__ == "__main__":
    main()
