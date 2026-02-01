# Rustassistant Quick Start Guide

Get started with Rustassistant in under 5 minutes! 🚀

## What is Rustassistant?

Rustassistant is a developer workflow management system that helps you:
- **Capture ideas** quickly with a note-taking system
- **Track repositories** you're working on
- **Get recommendations** on what to work on next
- **Organize work** with tags and status tracking

## Installation

### Build from Source

```bash
cd rustassistant
cargo build --release --bin devflow
```

The binary will be at `target/release/devflow`. Optionally, add it to your PATH:

```bash
# Linux/macOS
sudo cp target/release/devflow /usr/local/bin/

# Or add to your shell config:
export PATH="$PATH:/path/to/rustassistant/target/release"
```

### Quick Test

```bash
./target/debug/devflow --help
```

## Your First 5 Minutes

### 1. Capture Your First Note

```bash
devflow note add "Implement user authentication" --tags backend,priority
```

Output:
```
✓ Note created with ID: 1
─────────────────────────────────────────
ID: 1
Status: inbox
Tags: backend, priority
Created: 2026-02-01 10:30:00
─────────────────────────────────────────
Implement user authentication
─────────────────────────────────────────
```

### 2. Add More Ideas

```bash
devflow note add "Design landing page" --tags frontend,design
devflow note add "Set up CI/CD pipeline" --tags devops
devflow note add "Write API documentation" --tags docs
```

### 3. See What You've Captured

```bash
devflow note list
```

Output shows all your notes with emoji status indicators:
- 📥 **inbox** - newly captured
- 🔥 **active** - currently working on
- ✅ **processed** - completed/converted to tasks
- 📦 **archived** - parked for later

### 4. Mark Something as Active

```bash
devflow note update 1 --status active
```

### 5. Ask What's Next

```bash
devflow next
```

Rustassistant will show you:
- Current active work
- Pending inbox items
- A recommendation on what to focus on

## Core Commands

### Note Management

```bash
# Add a note
devflow note add "Your idea here" --tags tag1,tag2

# List all notes
devflow note list

# Filter by tag
devflow note list --tag backend

# Filter by status
devflow note list --status active

# Search notes
devflow note search "authentication"

# Show specific note
devflow note show 1

# Update note
devflow note update 1 --status processed
devflow note update 1 --content "New content"

# Add/remove tags
devflow note tag 1 newtag
devflow note untag 1 oldtag

# Delete note
devflow note delete 1
```

### Repository Tracking

```bash
# Track current repository
devflow repo add . --name myproject

# Track another repository
devflow repo add /path/to/repo --name other-project

# List repositories
devflow repo list

# Show repository status
devflow repo status myproject

# Remove repository
devflow repo remove myproject
```

### Workflow Commands

```bash
# What should I work on next?
devflow next

# Show statistics
devflow stats
```

## Note Status Workflow

Rustassistant uses a simple 4-status workflow:

```
📥 inbox      → Newly captured ideas (default)
   ↓
🔥 active     → Currently working on this
   ↓
✅ processed  → Completed or converted to tasks
   ↓
📦 archived   → Parked for future consideration
```

### Status Transitions

```bash
# Start working on something
devflow note update 3 --status active

# Mark as done
devflow note update 3 --status processed

# Park for later
devflow note update 3 --status archived

# Bring back from archive
devflow note update 3 --status inbox
```

## Tagging Strategy

Organize your notes with tags:

### By Phase
```bash
--tags phase1,phase2,phase3
```

### By Type
```bash
--tags idea,bug,feature,research,decision
```

### By Technology
```bash
--tags rust,python,typescript,docker
```

### By Priority
```bash
--tags urgent,important,low-priority
```

### By Domain
```bash
--tags backend,frontend,database,api,ui,devops
```

## Daily Workflow

### Morning: Check What's Next
```bash
devflow next
```

### Throughout the Day: Capture Everything
```bash
# Quick ideas
devflow note add "idea: use connection pooling" --tags backend,performance

# Bugs found
devflow note add "bug: login fails on mobile Safari" --tags bug,frontend

# Research notes
devflow note add "research alternatives to Redux" --tags research,frontend

# Decisions
devflow note add "decision: use PostgreSQL over MySQL" --tags decision,database
```

### End of Day: Review and Prioritize
```bash
# See what you captured
devflow note list --status inbox

# Mark important items as active
devflow note update 5 --status active
devflow note update 7 --status active

# Archive things that aren't relevant anymore
devflow note update 9 --status archived
```

### Weekly: Review Stats
```bash
devflow stats
```

## Tips & Tricks

### 1. Use Short Tag Names
```bash
# Good
--tags db,perf,api

# Works but verbose
--tags database,performance,application-programming-interface
```

### 2. Keep Notes Atomic
Each note should be one clear thought or task:

✅ Good:
```bash
devflow note add "Add pagination to user list endpoint" --tags api
devflow note add "Add sorting to user list endpoint" --tags api
```

❌ Too broad:
```bash
devflow note add "Improve user list endpoint with pagination, sorting, filtering, and caching" --tags api
```

### 3. Search for Context
```bash
# Find all authentication-related notes
devflow note search "auth"

# Find all database notes
devflow note list --tag database
```

### 4. Use Status Filters
```bash
# What am I actively working on?
devflow note list --status active

# What have I completed recently?
devflow note list --status processed

# What's still in my inbox?
devflow note list --status inbox
```

### 5. Limit Output for Focus
```bash
# Just show top 3 items
devflow note list --limit 3
```

## Database Location

By default, Rustassistant stores data in `devflow.db` in your current directory.

### Use a Global Database
```bash
# Set a custom location
devflow --database ~/.devflow/main.db note add "Global note"

# Or create an alias
alias devflow='devflow --database ~/.devflow/main.db'
```

### Use Per-Project Databases
```bash
# Each project has its own database
cd ~/projects/webapp
devflow note add "Project-specific note"

cd ~/projects/api
devflow note add "Different project note"
```

## What's Next?

You've mastered the basics! Here's what's coming in future phases:

### Phase 2 (Coming Soon)
- 🤖 **LLM Analysis** - Grok-powered code insights
- 🔍 **Repository Analysis** - Automatic file scoring and issue detection
- 📊 **Cost Tracking** - Monitor LLM API usage

### Phase 3 (Planned)
- 🌐 **Web UI** - Visual dashboard with HTMX
- 🔗 **RAG System** - Semantic search across repos and notes
- 📋 **Task Generation** - Automatic task creation from analysis
- 🎯 **Smart Recommendations** - LLM-powered "next action" suggestions

## Troubleshooting

### Database Errors

```bash
# If you get database errors, check permissions
ls -la devflow.db

# Recreate database
rm devflow.db
devflow note add "Test note"
```

### Build Issues

```bash
# Clean rebuild
cargo clean
cargo build --release --bin devflow
```

## Getting Help

```bash
# General help
devflow --help

# Command-specific help
devflow note --help
devflow note add --help
```

## Examples: Real-World Usage

### Solo Developer Building a SaaS

```bash
# Capture product ideas
devflow note add "Add dark mode toggle" --tags feature,ui
devflow note add "Implement team collaboration features" --tags feature,roadmap

# Track bugs
devflow note add "Memory leak in background job processor" --tags bug,urgent

# Document decisions
devflow note add "Chose Stripe over PayPal for payments" --tags decision,payments

# Research notes
devflow note add "Alternative: Consider Paddle for merchant of record" --tags research,payments
```

### Working on Open Source

```bash
# Track repositories
devflow repo add ~/code/my-project --name myproject
devflow repo add ~/code/dependency --name dep

# Capture contribution ideas
devflow note add "PR: Add TypeScript definitions" --tags contribution,typescript
devflow note add "Issue: Improve error messages in parser" --tags issue,dx
```

### Learning New Technology

```bash
# Research notes as you learn
devflow note add "Rust ownership: moved values can't be used again" --tags rust,learning
devflow note add "Async traits require Box<dyn Future>" --tags rust,async
devflow note add "Consider using tokio::spawn for concurrent tasks" --tags rust,concurrency

# Track practice projects
devflow note add "Build a CLI tool to practice clap" --tags project,practice
```

## Success Metrics

Track these to measure your workflow improvement:

- ✅ Capturing 10+ notes per week
- ✅ Inbox zero at end of each week
- ✅ Clear active work (2-5 items)
- ✅ Using tags consistently
- ✅ Running `devflow next` daily

---

**You're all set!** Start capturing your ideas and let Rustassistant help you stay organized. 🎯