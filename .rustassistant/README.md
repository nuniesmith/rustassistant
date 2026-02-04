# RustAssistant Cache Directory

This directory contains cached analysis results from RustAssistant.

## Structure

- `cache/analysis/` - General code analysis results
- `cache/docs/` - Generated documentation
- `cache/refactor/` - Refactoring suggestions and code smell detection
- `cache/todos/` - TODO/FIXME scan results

## Cache Invalidation

Cache entries are automatically invalidated when file contents change.
Each entry stores a SHA-256 hash of the analyzed file content.

## Managing the Cache

You can safely delete this directory to clear all cached results.
RustAssistant will regenerate cache entries as needed.

To disable caching, set `cache.enabled = false` in your config.

## Committing to Git

You can optionally commit this directory to version control to share
analysis results with your team. This can save API costs and speed up
analysis for unchanged files.

Add to `.gitignore` if you prefer not to track cache files:
```
.rustassistant/cache/
```
