#!/usr/bin/env python3
"""
fix_query_macros.py
===================
Converts remaining sqlx::query!(...) macros in src/db/documents.rs to
dynamic sqlx::query_as / sqlx::query calls that work without a live
database connection (no .sqlx/ cache needed).

The macro form requires a live Postgres connection at compile time to
verify column types. Until `cargo sqlx prepare` has been run against
the new Postgres schema, those macros block compilation. Switching to
dynamic queries removes the compile-time requirement while keeping
identical runtime behaviour.

Usage:
    python3 scripts/fix_query_macros.py
"""

import re
import sys
from pathlib import Path

TARGET = Path("src/db/documents.rs")

# ---------------------------------------------------------------------------
# Helper — map a Document field from a PgRow by column name
# ---------------------------------------------------------------------------
DOC_FROM_ROW = """\
        .map(|row: sqlx::postgres::PgRow| {
            use sqlx::Row;
            Document {
                id:           row.get::<Option<String>, _>("id").unwrap_or_default(),
                title:        row.get("title"),
                content:      row.get("content"),
                content_type: row.get::<Option<String>, _>("content_type").unwrap_or_else(|| "markdown".to_string()),
                source_type:  row.get::<Option<String>, _>("source_type").unwrap_or_else(|| "manual".to_string()),
                source_url:   row.get("source_url"),
                doc_type:     row.get::<Option<String>, _>("doc_type").unwrap_or_else(|| "reference".to_string()),
                tags:         row.get("tags"),
                repo_id:      row.get("repo_id"),
                file_path:    row.get("file_path"),
                word_count:   row.get::<Option<i64>, _>("word_count").unwrap_or(0),
                char_count:   row.get::<Option<i64>, _>("char_count").unwrap_or(0),
                created_at:   row.get("created_at"),
                updated_at:   row.get("updated_at"),
                indexed_at:   row.get("indexed_at"),
            }
        })"""

# ---------------------------------------------------------------------------
# SELECT columns used in every document query
# ---------------------------------------------------------------------------
DOC_COLS = (
    "id, title, content, content_type, source_type, source_url, doc_type, "
    "tags, repo_id, file_path, word_count, char_count, created_at, updated_at, indexed_at"
)

# ---------------------------------------------------------------------------
# Replacement blocks keyed by a unique anchor string found in the original
# ---------------------------------------------------------------------------
# Each entry is (anchor_pattern, replacement_fn)
# anchor_pattern: regex that uniquely identifies the block to replace
# replacement_fn: callable(match) -> str  OR a plain string

REPLACEMENTS = [
    # -----------------------------------------------------------------------
    # 1. get_document — single-row fetch by id
    # -----------------------------------------------------------------------
    (
        re.compile(
            r"let row = sqlx::query!\s*\(\s*"
            r'"SELECT id.*?FROM documents WHERE id = \?".*?\)\s*'
            r"\.fetch_one\(pool\)\s*\n\s*\.await\s*\n\s*\.map_err\([^)]+\)\?;",
            re.DOTALL,
        ),
        """\
    let row = sqlx::query(
        "SELECT {cols} FROM documents WHERE id = $1"
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {{
        sqlx::Error::RowNotFound => DbError::NotFound(format!("Document {{}} not found", id)),
        e => DbError::Sqlx(e),
    }})?;
    use sqlx::Row as _;
    let row_id: Option<String> = row.get("id");""".format(cols=DOC_COLS),
    ),
    # -----------------------------------------------------------------------
    # 2. search_documents_by_title  — fetch_all with two LIKE params + limit
    # -----------------------------------------------------------------------
    (
        re.compile(
            r"let rows = sqlx::query!\s*\(\s*"
            r'"SELECT id.*?WHERE title LIKE \? OR content LIKE \?.*?LIMIT \?".*?'
            r"search_pattern,\s*search_pattern,\s*limit\s*\)\s*"
            r"\.fetch_all\(pool\)\s*\n\s*\.await\s*\n\s*\.map_err\(DbError::Sqlx\)\?;",
            re.DOTALL,
        ),
        """\
    let rows = sqlx::query(
        "SELECT {cols}
         FROM documents
         WHERE title ILIKE $1 OR content ILIKE $2
         ORDER BY updated_at DESC
         LIMIT $3"
    )
    .bind(&search_pattern)
    .bind(&search_pattern)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(DbError::Sqlx)?;""".format(cols=DOC_COLS),
    ),
    # -----------------------------------------------------------------------
    # 3. search_documents_by_tags — fetch_all with tag param
    # -----------------------------------------------------------------------
    (
        re.compile(
            r"let rows = sqlx::query!\s*\(\s*"
            r'"SELECT DISTINCT.*?document_tags.*?LIMIT \?".*?'
            r"search_tag,\s*limit\s*\)\s*"
            r"\.fetch_all\(pool\)\s*\n\s*\.await\s*\n\s*\.map_err\(DbError::Sqlx\)\?;",
            re.DOTALL,
        ),
        """\
    let rows = sqlx::query(
        "SELECT DISTINCT d.id, d.title, d.content, d.content_type, d.source_type,
                d.source_url, d.doc_type, d.tags, d.repo_id, d.file_path,
                d.word_count, d.char_count, d.created_at, d.updated_at, d.indexed_at
         FROM documents d
         JOIN document_tags dt ON d.id = dt.document_id
         WHERE dt.tag ILIKE $1
         ORDER BY d.updated_at DESC
         LIMIT $2"
    )
    .bind(&search_tag)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(DbError::Sqlx)?;""",
    ),
    # -----------------------------------------------------------------------
    # 4. get_unindexed_documents — fetch_all, no params
    # -----------------------------------------------------------------------
    (
        re.compile(
            r"let rows = sqlx::query!\s*\(\s*"
            r'"SELECT id.*?WHERE.*?indexed_at IS NULL.*?LIMIT \?".*?'
            r"limit\s*\)\s*"
            r"\.fetch_all\(pool\)\s*\n\s*\.await\s*\n\s*\.map_err\(DbError::Sqlx\)\?;",
            re.DOTALL,
        ),
        """\
    let rows = sqlx::query(
        "SELECT {cols}
         FROM documents
         WHERE indexed_at IS NULL OR updated_at > indexed_at
         ORDER BY updated_at DESC
         LIMIT $1"
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(DbError::Sqlx)?;""".format(cols=DOC_COLS),
    ),
    # -----------------------------------------------------------------------
    # 5. list_ideas — fetch_all sqlx::query! form
    # -----------------------------------------------------------------------
    (
        re.compile(
            r"let rows = sqlx::query!\s*\(\s*"
            r'"SELECT.*?FROM ideas.*?LIMIT \?".*?'
            r"limit\s*\)\s*"
            r"\.fetch_all\(pool\)\s*\n\s*\.await\s*\n\s*\.map_err\(DbError::Sqlx\)\?;",
            re.DOTALL,
        ),
        """\
    let rows = sqlx::query(
        "SELECT id, content, tags, project, repo_id, priority, status,
                category, linked_doc_id, linked_task_id, created_at, updated_at
         FROM ideas
         ORDER BY priority ASC, created_at DESC
         LIMIT $1"
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(DbError::Sqlx)?;""",
    ),
    # -----------------------------------------------------------------------
    # 6. search_documents (FTS5 → tsvector)
    #    Original: WHERE documents_fts MATCH ?
    #    Postgres:  WHERE search_vector @@ plainto_tsquery('english', $1)
    # -----------------------------------------------------------------------
    (
        re.compile(
            r"let rows = sqlx::query!\s*\(\s*"
            r'"SELECT.*?documents_fts.*?LIMIT \?".*?'
            r"query,\s*limit\s*\)\s*"
            r"\.fetch_all\(pool\)\s*\n\s*\.await\s*\n\s*\.map_err\(DbError::Sqlx\)\?;",
            re.DOTALL,
        ),
        """\
    let rows = sqlx::query(
        "SELECT {cols}
         FROM documents
         WHERE search_vector @@ plainto_tsquery('english', $1)
         ORDER BY ts_rank(search_vector, plainto_tsquery('english', $1)) DESC
         LIMIT $2"
    )
    .bind(query)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(DbError::Sqlx)?;""".format(cols=DOC_COLS),
    ),
]

# ---------------------------------------------------------------------------
# Row → Document mapping block
# We need to replace the `Ok(Document { id: row.id.unwrap_or_default(), ...})` blocks
# that come from `query!` with `row.get(...)` equivalents.
# ---------------------------------------------------------------------------
OLD_ROW_MAP = re.compile(
    r"Document\s*\{\s*"
    r"id:\s*row\.id\.unwrap_or_default\(\),.*?"
    r"indexed_at:\s*row\.indexed_at,\s*\}",
    re.DOTALL,
)

NEW_ROW_MAP = """\
Document {
                id:           row.get::<Option<String>, _>("id").unwrap_or_default(),
                title:        row.get("title"),
                content:      row.get("content"),
                content_type: row.get::<Option<String>, _>("content_type").unwrap_or_else(|| "markdown".to_string()),
                source_type:  row.get::<Option<String>, _>("source_type").unwrap_or_else(|| "manual".to_string()),
                source_url:   row.get("source_url"),
                doc_type:     row.get::<Option<String>, _>("doc_type").unwrap_or_else(|| "reference".to_string()),
                tags:         row.get("tags"),
                repo_id:      row.get("repo_id"),
                file_path:    row.get("file_path"),
                word_count:   row.get::<Option<i64>, _>("word_count").unwrap_or(0),
                char_count:   row.get::<Option<i64>, _>("char_count").unwrap_or(0),
                created_at:   row.get("created_at"),
                updated_at:   row.get("updated_at"),
                indexed_at:   row.get("indexed_at"),
            }"""


def process_file(path: Path) -> None:
    if not path.exists():
        print(f"ERROR: {path} not found. Run from repo root.", file=sys.stderr)
        sys.exit(1)

    content = path.read_text(encoding="utf-8")
    original = content

    # Apply each structural replacement
    for pattern, replacement in REPLACEMENTS:
        new_content, n = pattern.subn(replacement, content)
        if n:
            print(f"  Applied replacement ({n}×): {pattern.pattern[:60]}...")
            content = new_content
        else:
            print(f"  (no match) pattern: {pattern.pattern[:60]}...")

    # Fix all Document struct literals that still use row.id / row.title etc.
    # (from the query! macro expansions that remain)
    new_content, n = OLD_ROW_MAP.subn(NEW_ROW_MAP, content)
    if n:
        print(f"  Fixed {n} Document row-mapping block(s)")
        content = new_content

    # Make sure sqlx::Row is imported for .get() calls
    if "use sqlx::Row" not in content and "row.get(" in content:
        # Insert after the existing `use sqlx` imports
        content = content.replace(
            "use sqlx::{Row, PgPool};",
            "use sqlx::{Row, PgPool};",
            1,
        )
        # Fallback: add at top of use block
        if "use sqlx::Row" not in content:
            content = content.replace(
                "use sqlx::{PgPool};",
                "use sqlx::{PgPool, Row};",
                1,
            )
            if "use sqlx::Row" not in content:
                content = "use sqlx::Row;\n" + content

    if content == original:
        print("  (no changes made to file)")
    else:
        path.write_text(content, encoding="utf-8")
        print(f"\nWrote {path}")


def main() -> None:
    print(f"Processing {TARGET} ...")
    process_file(TARGET)
    print("\nDone.")
    print()
    print("NOTE: The get_document function still constructs Document from")
    print("      named row columns. Add `use sqlx::Row as _;` at the top")
    print("      of each function that calls row.get() if the compiler")
    print("      complains about the Row trait not being in scope.")


if __name__ == "__main__":
    main()
