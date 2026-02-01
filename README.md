# RustAssistant - Developer Workflow Management System

> 🚀 **A Rust-based workflow manager for solo developers to track repos, capture ideas, and leverage LLM-powered insights**

RustAssistant helps you manage the entire development lifecycle from idea capture to production deployment. Built with Rust, powered by Grok AI, designed for developers who manage multiple GitHub repositories.

## 🎯 Core Features

### 📝 Note & Thought Capture
- Quick note input with tag-based categorization
- Personal notes for random thoughts
- Project-specific notes linked to repos
- Forward notes to specific projects for future work

### 🗂️ Repository Management
- Track all your GitHub repositories
- Cache directory trees and file contents
- Monitor changes across repos
- Standardize tooling and patterns

### 🤖 LLM-Powered Analysis
- Grok AI API integration with large context window
- Score files for quality, security, and complexity
- Find issues and suggest improvements
- Identify common patterns and shared logic
- Generate actionable tasks from analysis
- **Batch analysis** - Analyze multiple files efficiently

### 🎯 Solo Developer Workflow
- **Research**: Validate and expand research areas
- **Planning**: Break down complex features
- **Prototype**: Track experimental code
- **Production**: Monitor production-ready systems
- **Next Actions**: Always know what to work on next

### 💰 Cost Optimization
- Response caching (70%+ cost savings)
- Smart TTL-based expiration
- Batch operations for efficiency
- Cost tracking and monitoring

### 🏗️ Tech Stack Support
Built to work with your stack:
- **Languages**: Rust, Kotlin Multiplatform, JavaScript, TypeScript, Python
- **Infrastructure**: Docker Compose, Nginx, Prometheus, Alertmanager, Grafana, Loki
- **Databases**: PostgreSQL, Redis, QuestDB, SQLite
- **CI/CD**: GitHub Actions (test → build-push → deploy)

## ⚡ Quick Start

### Prerequisites

- Rust 1.70+ (`rustup` recommended)
- Git
- Grok API key from [x.ai](https://x.ai)

### Installation

```bash
# Clone the repository
git clone https://github.com/jordanistan/rustassistant.git
cd rustassistant

# Set up environment
cp .env.example .env
# Edit .env and add your XAI_API_KEY

# Build the project
cargo build --release

# Install CLI globally (optional)
cargo install --path . --bin rustassistant
```

### Quick Test

```bash
# Run via script
./run.sh check

# Or use cargo directly
cargo run --bin rustassistant -- --help
```

## 🌐 Web UI (NEW!)

RustAssistant now includes a modern web-based dashboard for managing your development workflow.

### Features

- 📊 **Dashboard**: Real-time stats, recent notes, activity feed, cost insights
- 📝 **Notes Management**: View, filter, and organize your notes with tags
- 📦 **Repository Tracking**: Monitor all tracked repositories
- 💰 **Cost Tracking**: Detailed LLM API cost breakdown and savings visualization
- 🔍 **Code Analysis**: Run AI-powered code analysis from the browser
- 🌙 **Dark Mode**: Beautiful dark theme by default, easy on the eyes
- ⚡ **HTMX-Powered**: Fast, modern UI with minimal JavaScript

### Start the Web UI

```bash
# Build the web server
cargo build --release --bin webui-server

# Start the server (default: http://127.0.0.1:3001)
./target/release/webui-server

# Or with custom configuration
PORT=8080 DATABASE_PATH=custom.db ./target/release/webui-server
```

### Access the Dashboard

- **Dashboard**: http://127.0.0.1:3001/
- **Notes**: http://127.0.0.1:3001/notes
- **Repositories**: http://127.0.0.1:3001/repos
- **Costs**: http://127.0.0.1:3001/costs
- **Analyze**: http://127.0.0.1:3001/analyze

### Documentation

See [Web UI Guide](docs/WEB_UI_GUIDE.md) for detailed documentation on:
- Architecture and technology stack
- Development guide
- Adding pages and features
- HTMX integration
- Deployment options

## 🚀 Full System (Production Ready!)

### Environment Setup

Create a `.env` file:

```env
# Grok API Configuration
XAI_API_KEY=xai-your-api-key-here
XAI_BASE_URL=https://api.x.ai/v1

# Server Configuration
HOST=127.0.0.1
PORT=3000

# Database (defaults to data/rustassistant.db)
DATABASE_PATH=data/rustassistant.db

# Cache Configuration
CACHE_DB_PATH=data/rustassistant_cache.db
```

### Usage Examples

#### 1. Note Management

```bash
# Add a quick note
rustassistant note add "Implement batch analysis for code reviews" --tags feature,phase2

# List all notes
rustassistant note list

# Filter by tag
rustassistant note list --tag rust --status inbox

# Search notes
rustassistant note search "batch"

# View a specific note
rustassistant note show 5

# Update note status
rustassistant note update 5 --status active
```

#### 2. Repository Tracking

```bash
# Add a repository to track
rustassistant repo add ~/projects/myapp

# List tracked repositories
rustassistant repo list

# Analyze repository structure
rustassistant repo analyze myapp

# View directory tree
rustassistant repo tree myapp --depth 3

# List files by language
rustassistant repo files myapp --language rust
```

#### 3. AI-Powered Analysis

```bash
# Score a single file
rustassistant analyze file src/main.rs

# Quick analysis
rustassistant analyze quick "fn add(a: i32, b: i32) -> i32 { a + b }"

# Ask Grok a question
rustassistant analyze ask "What are best practices for error handling in Rust?"

# Analyze entire repository with context
rustassistant analyze repo myapp --language rust

# Query with full codebase context
rustassistant analyze query "Where is authentication handled?" --repo myapp

# Find patterns across codebase
rustassistant analyze patterns "TODO" --repo myapp

# Batch analyze multiple files (efficient!)
rustassistant analyze batch src/ --output quality-report.md
rustassistant analyze batch src/*.rs --batch-size 20
rustassistant analyze batch src/ tests/ --output full-audit.md
```

#### 4. Cost & Cache Management

```bash
# View LLM costs
rustassistant costs

# Cache statistics
rustassistant cache stats

# Clear expired cache entries
rustassistant cache prune

# View most frequently accessed cache entries
rustassistant cache hot --limit 10
```

#### 5. Next Actions

```bash
# See what to work on next
rustassistant next

# View overall statistics
rustassistant stats
```

## 📊 Batch Analysis

Efficiently analyze multiple files with aggregate statistics:

```bash
# Analyze entire directory
rustassistant analyze batch src/ --output report.md

# Code review for PR
git diff --name-only main | xargs rustassistant analyze batch

# Custom batch size
rustassistant analyze batch src/ --batch-size 15

# Multiple directories
rustassistant analyze batch src/ tests/ examples/
```

**Benefits:**
- 50-70% faster than sequential analysis
- Aggregate statistics and insights
- Markdown report generation
- Full cache integration (re-runs are instant!)
- Cross-file pattern detection

See [docs/BATCH_OPERATIONS.md](docs/BATCH_OPERATIONS.md) for complete guide.

## 🏗️ Project Structure

```
rustassistant/
├── src/
│   ├── bin/
│   │   ├── server.rs              # Web server entry point
│   │   ├── devflow_cli.rs         # Main CLI tool
│   │   └── cli.rs                 # Legacy audit CLI
│   ├── cache.rs                   # File system cache
│   ├── config.rs                  # Configuration management
│   ├── context_builder.rs         # Build analysis context
│   ├── db.rs                      # SQLite database layer
│   ├── directory_tree.rs          # Directory tree analysis
│   ├── git.rs                     # Git operations
│   ├── grok_client.rs             # Grok API client
│   ├── grok_reasoning.rs          # Advanced Grok reasoning
│   ├── llm.rs                     # LLM integration layer
│   ├── query_templates.rs         # Pre-built query templates
│   ├── repo_analysis.rs           # Repository analysis
│   ├── response_cache.rs          # Response caching system
│   ├── scanner.rs                 # Code scanner
│   ├── tags.rs                    # Tag management
│   ├── tasks.rs                   # Task generation
│   └── ...
├── data/                          # Database files (gitignored)
│   ├── rustassistant.db           # Main database
│   └── rustassistant_cache.db     # Response cache
├── docs/                          # Documentation
│   ├── BATCH_OPERATIONS.md        # Batch analysis guide
│   ├── NEXT_PRIORITIES.md         # Roadmap and next steps
│   ├── QUICK_DECISION_GUIDE.md    # Decision matrix
│   └── ...
├── scripts/                       # Utility scripts
├── config/                        # Configuration files
├── static/                        # Static web assets
├── Cargo.toml                     # Rust dependencies
├── run.sh                         # Quick start script
└── README.md                      # This file
```

## 💡 Workflow Examples

### Daily Workflow

```bash
# Morning: Check what's next
rustassistant next

# Add thoughts as they come
rustassistant note add "Consider using Arc instead of Rc for thread safety" --tags idea,rust

# Analyze code you're working on
rustassistant analyze file src/api/handler.rs

# End of day: Review costs
rustassistant costs
rustassistant cache stats
```

### Code Review Workflow

```bash
# Get changed files
git diff --name-only main > changed_files.txt

# Batch analyze all changes
rustassistant analyze batch $(cat changed_files.txt) --output pr-review.md

# Review the report
cat pr-review.md

# Re-run after fixes (cached = instant!)
rustassistant analyze batch $(cat changed_files.txt)
```

### Project Audit

```bash
# Full repository analysis
rustassistant repo add ~/projects/myapp
rustassistant analyze batch ~/projects/myapp/src/ --output audit-$(date +%Y%m%d).md

# Track improvement over time
git add audit-*.md
git commit -m "Weekly code quality audit"
```

## 🎓 Key Concepts

### LLM Cost Management

**Grok Fast Reasoning** (cost-effective):
- Input: ~$0.20 per 1M tokens (estimated)
- Output: ~$0.50 per 1M tokens (estimated)
- Cached: 90% savings on repeated content

**Cost Optimization Strategies:**
1. **Response Caching** - 70%+ savings on repeated queries
2. **Batch Operations** - Efficient multi-file analysis
3. **Smart Context Building** - Only include relevant files
4. **Cost Tracking** - Monitor and control spending

**Typical Costs:**
- File analysis: ~$0.003-0.005 per file
- Batch analysis (20 files): ~$0.08-0.10 first run, ~$0 cached
- Daily usage: <$2/day with caching

### Response Caching

All LLM responses are automatically cached:
- **Content-based hashing** - SHA-256 for deduplication
- **TTL expiration** - Configurable cache lifetime
- **Hit rate tracking** - Monitor cache efficiency
- **Cost savings** - 70%+ reduction in API costs

### Context Building

Smart context assembly for better LLM responses:
- Load entire repo structure (up to 100K tokens)
- Filter by language, recency, or custom criteria
- Include relevant notes and documentation
- Optimize token usage while maximizing insight

## 🔧 Configuration

### CLI Configuration

Most commands accept options:
- `--verbose` - Enable debug logging
- `--database <path>` - Custom database path (default: data/rustassistant.db)

### Environment Variables

```env
XAI_API_KEY=<your-key>          # Required for AI features
XAI_BASE_URL=<api-endpoint>      # Default: https://api.x.ai/v1
DATABASE_PATH=<db-path>          # Default: data/rustassistant.db
CACHE_DB_PATH=<cache-path>       # Default: data/rustassistant_cache.db
RUST_LOG=info                    # Logging level
```

## 🗺️ Roadmap

### ✅ Phase 1: Core Foundation (COMPLETE)
- ✅ Note system with tags and search
- ✅ Repository tracking with directory trees
- ✅ Grok API integration
- ✅ File scoring and analysis
- ✅ Response caching (70%+ cost savings)
- ✅ Cost tracking and monitoring
- ✅ Batch analysis operations
- ✅ CLI with 50+ commands

### 🔄 Phase 2: Advanced Features (In Progress)
- [ ] Code review automation
- [ ] Test generation from code
- [ ] Refactoring assistant
- [ ] Documentation generator
- [ ] Dependency analysis

### 🌐 Phase 3: Web UI (Planned)
- [ ] HTMX + Askama web dashboard
- [ ] Real-time cost tracking
- [ ] Interactive analysis interface
- [ ] Repository browser
- [ ] Team collaboration features

### 🚀 Phase 4: Production & Scale (Future)
- [ ] Multi-user support
- [ ] Team permissions
- [ ] Advanced RAG with vector search
- [ ] CI/CD integrations
- [ ] Monitoring and alerts

## 📚 Documentation

### Core Documentation
- **[Getting Started Guide](docs/GETTING_STARTED.md)** - Detailed setup instructions
- **[Batch Operations](docs/BATCH_OPERATIONS.md)** - Complete batch analysis guide
- **[Advanced Features Guide](docs/ADVANCED_FEATURES_GUIDE.md)** - Code review, testing, refactoring
- **[CLI Cheat Sheet](docs/CLI_CHEATSHEET.md)** - All commands reference

### Web UI Documentation
- **[Web UI Guide](docs/WEB_UI_GUIDE.md)** - Complete web interface documentation
- **[Web UI Completion Report](docs/WEB_UI_COMPLETION.md)** - Implementation status and next steps

### Planning & Progress
- **[Next Priorities](docs/NEXT_PRIORITIES.md)** - Roadmap and implementation plans
- **[Decision Guide](docs/QUICK_DECISION_GUIDE.md)** - Choose your next feature
- **[Session Summaries](SESSION*.md)** - Development progress logs

## 🤝 Contributing

This is currently a personal project, but feedback and suggestions are welcome!

## 📄 License

MIT License - See [LICENSE](LICENSE) file for details

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- LLM powered by [Grok (xAI)](https://x.ai)
- Inspired by the needs of solo developers managing multiple projects

## 💬 Support

- **Issues**: [GitHub Issues](https://github.com/jordanistan/rustassistant/issues)
- **Discussions**: [GitHub Discussions](https://github.com/jordanistan/rustassistant/discussions)

---

**Status: Production Ready** 🚀  
**Last Updated: February 1, 2026**  
**Phase: 1 Complete, Phase 2 In Progress**

Built with ❤️ for solo developers who need an AI-powered workflow assistant.