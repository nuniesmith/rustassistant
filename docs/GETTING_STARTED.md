# Getting Started with Rustassistant

Welcome to Rustassistant! This guide will help you set up and start using Rustassistant to manage your developer workflow.

## What is Rustassistant?

Rustassistant is a Rust-based workflow management system designed for solo developers who:
- Manage multiple GitHub repositories
- Want to track ideas, notes, and tasks
- Need help prioritizing what to work on next
- Want to leverage LLM-powered code analysis affordably

## Prerequisites

Before you start, make sure you have:

- **Rust 1.75+** - Install from [rustup.rs](https://rustup.rs)
- **Git** - For repository operations
- **XAI API Key** - Get from [console.x.ai](https://console.x.ai) (free credits available)
- **Docker** (optional) - For containerized deployment

## Installation

### 1. Clone and Build

```bash
# Clone the repository
git clone https://github.com/your-username/devflow.git
cd devflow

# Build the project
cargo build --release

# The binaries will be in target/release/
```

### 2. Set Up Environment

Create a `.env` file in the project root:

```bash
# Copy the example
cp .env.example .env

# Edit with your favorite editor
nano .env
```

Add your configuration:

```env
# XAI API Configuration
XAI_API_KEY=xai-your-api-key-here
XAI_BASE_URL=https://api.x.ai/v1

# Server Configuration
HOST=127.0.0.1
PORT=3000

# Logging
RUST_LOG=info,devflow=debug
```

### 3. Initialize Database

```bash
# Create data directories
mkdir -p data/{repos,vectors,notes}

# Initialize the database
cargo run --bin devflow -- init
```

### 4. Verify Installation

```bash
# Test the CLI
cargo run --bin devflow -- --version

# Test the API connection
cargo run --bin devflow -- test-api

# Expected output:
# ✅ XAI API: Connected
# ✅ Model: grok-4-1-fast-reasoning
# ✅ Rate limit: 10000 RPM
```

## Quick Start Tutorial

### 1. Capture Your First Note

```bash
# Add a quick thought
cargo run --bin devflow -- note add "Build a code analysis dashboard" --tags idea,ui

# Add a project-specific note
cargo run --bin devflow -- note add "Refactor authentication module" --project myapp --tags refactor

# List all notes
cargo run --bin devflow -- note list
```

### 2. Track a Repository

```bash
# Add a repository to track
cargo run --bin devflow -- repo add ~/github/myproject

# Rustassistant will:
# - Scan the directory tree
# - Detect languages and frameworks
# - Cache file metadata
# - Calculate initial statistics

# View repository status
cargo run --bin devflow -- repo status myproject
```

### 3. Run Your First Analysis

```bash
# Analyze a repository with static checks
cargo run --bin devflow -- repo analyze myproject

# Run deep analysis with LLM
cargo run --bin devflow -- repo analyze myproject --deep

# Score individual files
cargo run --bin devflow -- repo score myproject/src/main.rs
```

### 4. Generate Tasks

```bash
# Generate tasks from analysis
cargo run --bin devflow -- tasks generate myproject

# View tasks
cargo run --bin devflow -- tasks list --priority high

# Mark a task complete
cargo run --bin devflow -- tasks done TASK-123
```

### 5. Find What to Work On Next

```bash
# Show recommended next actions
cargo run --bin devflow -- next

# Filter by category
cargo run --bin devflow -- next --category prototype

# Show only high priority
cargo run --bin devflow -- next --priority high
```

## Starting the Web Server

If you prefer a web interface:

```bash
# Start the server
cargo run --release --bin devflow-server

# Server starts at http://localhost:3000
# Open in your browser
```

The web interface provides:
- Dashboard with overview of all repos and tasks
- Note-taking interface
- Repository browser
- Task management
- Analysis results and insights

## Common Workflows

### Daily Standup Workflow

```bash
# Morning routine
devflow next --today                    # What should I work on?
devflow note list --recent 1d           # What did I note yesterday?
devflow tasks list --status in-progress # What's in progress?

# Evening routine
devflow note add "Completed auth refactor, need to add tests tomorrow" --tags done
devflow tasks update TASK-123 --status done
```

### Research to Implementation

```bash
# 1. Capture research
devflow note add "Found great article on WASM plugins" --tags research --url https://...

# 2. Expand with expensive LLM (manual, using Claude)
# Use Claude Opus to deeply analyze and create detailed plan

# 3. Break down with cheap LLM
devflow research process research-notes.md --generate-tasks

# 4. Review and prioritize tasks
devflow tasks list --category research

# 5. Start implementation
devflow tasks start TASK-456
```

### Repository Health Check

```bash
# Run on all tracked repos
devflow repo check --all

# Output shows:
# - Files that need attention
# - Quality score trends
# - New TODOs
# - Stale branches
# - Missing documentation

# Deep dive on one repo
devflow repo analyze myproject --full-report
```

### Cross-Repository Pattern Analysis

```bash
# Find common patterns across all repos
devflow patterns analyze --all

# Rustassistant uses LLM to identify:
# - Repeated code patterns
# - Common configurations
# - Shared utilities that should be extracted
# - Inconsistent approaches to same problem
```

## Configuration

### Repository Profiles

Create profiles for different types of projects:

```bash
# Create a profile
devflow profile create rust-service --template rust

# Edit the profile
nano config/profiles/rust-service.toml

# Apply to a repo
devflow repo set-profile myproject rust-service
```

Example profile:

```toml
[profile]
name = "rust-service"
description = "Standard Rust microservice"

[structure]
required_dirs = ["src", "tests", "docker"]
required_files = ["Cargo.toml", "Dockerfile"]

[quality]
min_doc_coverage = 0.8
min_test_coverage = 0.7
max_complexity = 15
```

### Analysis Presets

Configure different analysis depths:

```bash
# Quick check (free, static only)
devflow repo analyze myproject --preset quick

# Standard (cheap LLM, good balance)
devflow repo analyze myproject --preset standard

# Deep dive (expensive LLM, comprehensive)
devflow repo analyze myproject --preset deep
```

## Cost Management

Rustassistant helps you manage LLM costs:

```bash
# Check current usage
devflow costs today

# Set a daily budget
devflow config set budget.daily 5.00

# Rustassistant will:
# - Warn when approaching 80% of budget
# - Block requests when budget exceeded
# - Suggest using cheaper models
# - Show cost breakdown by operation
```

### Model Selection Strategy

- **grok-4-1-fast-reasoning** (default, ~$0.20/M input tokens)
  - Daily analyses
  - Task generation
  - File scoring
  - Pattern detection

- **claude-opus-4-5** (expensive, manual trigger)
  - Deep research validation
  - Architecture review
  - Complex refactoring plans
  - Critical security audits

## Troubleshooting

### API Key Issues

```bash
# Test your API key
devflow test-api

# Common issues:
# ❌ Invalid API key format
#    Solution: Check that it starts with "xai-"

# ❌ Rate limit exceeded
#    Solution: Wait a minute, or upgrade plan

# ❌ Network error
#    Solution: Check firewall, proxy settings
```

### Cache Issues

```bash
# Clear cache for a repo
devflow repo clear-cache myproject

# Rebuild cache
devflow repo rebuild-cache myproject

# Clear all caches
devflow cache clear --all
```

### Performance Issues

```bash
# Check database size
devflow stats disk-usage

# Optimize database
devflow optimize

# Limit analysis scope
devflow repo analyze myproject --exclude tests --exclude vendor
```

## Next Steps

Now that you're set up, explore these features:

1. **[CLI Reference](CLI_REFERENCE.md)** - Complete command documentation
2. **[API Guide](../api/REST_API.md)** - REST API endpoints
3. **[Configuration](CONFIGURATION.md)** - Advanced configuration options
4. **[Docker Deployment](DEPLOYMENT.md)** - Deploy with Docker
5. **[Examples](../examples/)** - Real-world usage examples

## Getting Help

- **Documentation**: https://devflow.dev/docs
- **GitHub Issues**: https://github.com/your-username/devflow/issues
- **Discord**: https://discord.gg/devflow

## Quick Reference

```bash
# Notes
devflow note add "text" --tags tag1,tag2
devflow note list
devflow note search "keyword"

# Repositories
devflow repo add /path/to/repo
devflow repo analyze <name> [--deep]
devflow repo list

# Tasks
devflow tasks generate <repo>
devflow tasks list [--priority high]
devflow tasks done <id>

# Analysis
devflow repo score <repo>
devflow patterns analyze --all
devflow next [--category research]

# Server
devflow-server
```

Happy coding! 🚀