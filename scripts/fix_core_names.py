#!/usr/bin/env python3
"""
fix_core_names.py
=================
Fixes duplicate function names in src/db/core.rs that were introduced
by a bad edit-tool run which renamed functions instead of just changing
type signatures.

Corrections:
  Line 410:  get_note        -> list_notes        (the multi-row overload)
  Line 522:  get_tag         -> upsert_tag        (the insert-or-update overload)
  Line 673:  update_note_status -> update_note_repo  (the repo_id update overload)
  Line 1193: list_notes      -> list_tasks        (the task listing function)

Usage:
    python3 scripts/fix_core_names.py
"""

import sys
from pathlib import Path

TARGET = Path("src/db/core.rs")

# Map of (1-based line number) -> correct replacement line
# These are the lines that start with `pub async fn <wrong_name>(`
FIXES = {
    410: "pub async fn list_notes(\n",
    522: "pub async fn upsert_tag(\n",
    673: "pub async fn update_note_repo(\n",
    1193: "pub async fn list_tasks(\n",
}


def main() -> None:
    if not TARGET.exists():
        print(f"ERROR: {TARGET} not found. Run from the repo root.", file=sys.stderr)
        sys.exit(1)

    lines = TARGET.read_text(encoding="utf-8").splitlines(keepends=True)

    for lineno, replacement in sorted(FIXES.items()):
        idx = lineno - 1  # convert to 0-based
        if idx >= len(lines):
            print(f"  SKIP line {lineno}: file is only {len(lines)} lines long")
            continue

        original = lines[idx].rstrip("\n")
        lines[idx] = replacement
        print(f"  Line {lineno}: {original!r}")
        print(f"          -> {replacement.rstrip()!r}")

    TARGET.write_text("".join(lines), encoding="utf-8")
    print(f"\nDone. Wrote {TARGET}")


if __name__ == "__main__":
    main()
