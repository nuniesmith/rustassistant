# RustAssistant Cache Directory

This directory serves two purposes:

## 1. CLI Analysis Cache (`cache/`)

The Rust CLI (`rustassistant`) stores cached analysis results here to avoid
redundant LLM calls on unchanged files.

- `cache/analysis/` — General code analysis results
- `cache/docs/`     — Generated documentation
- `cache/refactor/` — Refactoring suggestions and code smell detection
- `cache/todos/`    — TODO/FIXME scan results

Cache entries are automatically invalidated when file contents change.
Each entry stores a SHA-256 hash of the analyzed file content.

To disable caching, set `cache.enabled = false` in your config.

## 2. LLM Audit Workflow State (`cache.json`, `batches/`)

The GitHub Actions workflow (`llm-audit.yml` in `nuniesmith/actions`) uses
this directory as persistent cross-run state so the repo is self-describing.

- `cache.json`           — File hashes, run history, batch completion state
- `batches/`             — Per-batch state files (status, PR number, diff)
- `todo_state.json`      — Snapshot of todo.md item completion (created on demand)
- `todo-plan.json`       — Cached gameplan from `todo-plan` mode (created on demand)
- `GAMEPLAN.md`          — Human-readable gameplan (created on demand)

The workflow reads `cache.json` on startup to know exactly where it left off,
then updates it before committing back. This means any clone of the repo has
full context about prior audit runs without needing a central state store.

### Workflow Modes

| Mode            | Reads                        | Writes                              |
|-----------------|------------------------------|-------------------------------------|
| `regular`/`full`| `cache.json`                 | `cache.json`                        |
| `todo-analyze`  | `cache.json`                 | `cache.json`                        |
| `todo-plan`     | `cache.json`                 | `cache.json`, `todo-plan.json`, `GAMEPLAN.md` |
| `todo-work`     | `cache.json`, `todo-plan.json` | `cache.json`, `batches/batch-N.json` |
| `todo-review`   | `cache.json`, `batches/*`    | `cache.json`                        |

## Managing the Cache

You can safely delete everything under `cache/` (the CLI cache) to clear
analysis results. RustAssistant will regenerate entries as needed.

**Do NOT delete `cache.json` or `batches/`** unless you want the audit
workflow to lose its run history and start fresh.

## Git Tracking

This directory is tracked in version control so that:
- The audit workflow's state persists across runs and clones
- Teams can optionally share CLI analysis results to save API costs

If you only want to track the workflow state (not CLI cache), add this
to `.gitignore`:
```
.rustassistant/cache/
```
