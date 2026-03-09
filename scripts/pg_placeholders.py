#!/usr/bin/env python3
"""
pg_placeholders.py
==================
Converts SQLite-style `?` parameter placeholders to PostgreSQL-style `$1`, `$2`, ...
in Rust source files that use sqlx raw query strings.

Works on:
  - r#"..."# raw string literals
  - "..." regular string literals

Only rewrites strings that contain at least one SQL keyword so that non-SQL
strings (error messages, format strings, etc.) are left alone.

Usage:
    python3 scripts/pg_placeholders.py src/db/core.rs src/db/queue.rs ...
    python3 scripts/pg_placeholders.py src/db/*.rs src/auto_scanner.rs
"""

import re
import sys
from pathlib import Path

SQL_KEYWORDS = {
    "SELECT",
    "INSERT",
    "UPDATE",
    "DELETE",
    "CREATE",
    "DROP",
    "ALTER",
    "WITH",
}


def looks_like_sql(s: str) -> bool:
    upper = s.upper()
    return any(kw in upper for kw in SQL_KEYWORDS)


def convert_placeholders(sql: str) -> str:
    """Replace each bare `?` with $1, $2, ... in order."""
    counter = [0]

    def repl(_m):
        counter[0] += 1
        return f"${counter[0]}"

    # Only replace standalone `?` — not `?` inside comments or string-like contexts.
    # Simple approach: replace every literal `?` character in order.
    return re.sub(r"\?", repl, sql)


def process_raw_string(m: re.Match) -> str:
    """Callback for r#"..."# matches."""
    inner = m.group(1)
    if looks_like_sql(inner):
        return 'r#"' + convert_placeholders(inner) + '"#'
    return m.group(0)


def process_regular_string(m: re.Match) -> str:
    """Callback for "..." matches (non-raw, non-escaped-quote strings)."""
    inner = m.group(1)
    if looks_like_sql(inner):
        return '"' + convert_placeholders(inner) + '"'
    return m.group(0)


# Matches r#"..."# (single-hash raw strings — the most common in sqlx code)
RAW_STRING_RE = re.compile(r'r#"(.*?)"#', re.DOTALL)

# Matches ordinary "..." strings, avoiding escaped quotes inside.
# This is intentionally conservative: it won't handle strings with \" inside,
# but those are rare in SQL literals.
REGULAR_STRING_RE = re.compile(r'"((?:[^"\\]|\\.)*?)"')


def process_file(path: Path) -> None:
    original = path.read_text(encoding="utf-8")
    content = original

    # Process raw strings first (they take priority and don't nest)
    content = RAW_STRING_RE.sub(process_raw_string, content)

    # Then process regular strings — but skip lines that are pure comments
    lines = content.splitlines(keepends=True)
    result_lines = []
    for line in lines:
        stripped = line.lstrip()
        if stripped.startswith("//") or stripped.startswith("*"):
            result_lines.append(line)
        else:
            result_lines.append(REGULAR_STRING_RE.sub(process_regular_string, line))
    content = "".join(result_lines)

    if content == original:
        print(f"  (no changes) {path}")
        return

    path.write_text(content, encoding="utf-8")

    # Count replacements made
    before_q = original.count("?")
    after_q = content.count("?")
    replaced = before_q - after_q
    print(f"  {path}  — replaced {replaced} placeholder(s)")


def main() -> None:
    if len(sys.argv) < 2:
        print("Usage: python3 scripts/pg_placeholders.py <file> [<file> ...]")
        sys.exit(1)

    files = [Path(p) for p in sys.argv[1:]]
    missing = [p for p in files if not p.exists()]
    if missing:
        for p in missing:
            print(f"ERROR: file not found: {p}", file=sys.stderr)
        sys.exit(1)

    print(f"Converting ? → $N in {len(files)} file(s):")
    for p in files:
        process_file(p)
    print("Done.")


if __name__ == "__main__":
    main()
