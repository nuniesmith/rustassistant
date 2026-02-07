# 📁 RustAssistant Project Structure

This document provides a comprehensive overview of the project's directory structure and file organization.

## 📂 Top-Level Directory Layout

```
rustassistant/
├── src/                      # Source code
├── docs/                     # Documentation
├── examples/                 # Usage examples
├── tests/                    # Integration tests
├── migrations/               # Database migrations
├── scripts/                  # Utility scripts
├── config/                   # Configuration files
├── static/                   # Static web assets
├── docker/                   # Docker configurations
├── deployment/               # Deployment configs
├── data/                     # Local data directory
├── Cargo.toml                # Rust project manifest
├── Cargo.lock                # Dependency lock file
├── README.md                 # Project overview
├── CONTRIBUTING.md           # Contribution guidelines
├── PROJECT_STATUS.md         # Current status and roadmap
├── LICENSE                   # MIT License
└── docker-compose*.yml       # Docker Compose configurations
```

## 🔍 Detailed Structure

### `/src` - Source Code

```
src/
├── api/                      # REST API implementation
│   ├── mod.rs               # API router and configuration
│   ├── admin.rs             # Admin dashboard endpoints
│   ├── auth.rs              # Authentication middleware
│   ├── handlers.rs          # API request handlers
│   ├── jobs.rs              # Background job queue
│   ├── rate_limit.rs        # Rate limiting middleware
│   └── types.rs             # API request/response types
│
├── templates/               # Web UI templates (Askama)
│   ├── layouts/             # Layout templates
│   │   └── base.html        # Base HTML template
│   └── pages/               # Page templates
│       ├── admin.html       # Admin dashboard
│       ├── documents.html   # Document management
│       └── search.html      # Search interface
│
├── bin/                     # Binary executables
│   ├── cli.rs               # CLI tool
│   ├── server.rs            # API server
│   └── github-sync-daemon.rs # GitHub sync daemon
│
├── cache_layer.rs           # Redis + memory caching (650 lines)
├── telemetry.rs             # OpenTelemetry tracing (360 lines)
├── query_analytics.rs       # Search analytics (770 lines)
├── multi_tenant.rs          # Multi-tenancy support (730 lines)
├── vector_index.rs          # HNSW vector index
├── webhooks.rs              # Webhook system
├── metrics.rs               # Prometheus metrics
├── chunking.rs              # Document chunking
├── embeddings.rs            # Vector embeddings
├── indexing.rs              # Document indexing
├── search.rs                # Semantic search
├── db.rs                    # Database operations
├── web_ui.rs                # Web UI routing
├── config.rs                # Configuration management
├── error.rs                 # Error types
└── lib.rs                   # Library exports
```

### `/docs` - Documentation

```
docs/
├── guides/                           # User guides
│   ├── QUICK_START.md               # Getting started (5 min)
│   └── ADVANCED_FEATURES_GUIDE.md   # Advanced features guide
│
├── archive/                          # Historical documentation
│   ├── PHASE2_COMPLETE.md           # Phase 2 implementation
│   ├── PHASE3_COMPLETE.md           # Phase 3 implementation
│   ├── PHASE4_COMPLETE.md           # Phase 4 implementation
│   ├── PHASE5.md                    # Phase 5 implementation
│   ├── ADVANCED_FEATURES.md         # Initial advanced features
│   └── DEPLOYMENT_COMPLETE.md       # Deployment documentation
│
├── RAG_API.md                       # API reference documentation
├── ADVANCED_FEATURES_COMPLETE.md    # Advanced features technical guide
├── IMPLEMENTATION_COMPLETE.md       # Implementation summary
└── FEATURES_SUMMARY.md              # Feature overview
```

### `/examples` - Usage Examples

```
examples/
├── rag_server.rs            # Complete RAG server example
├── chunking_example.rs      # Document chunking demo
├── embedding_example.rs     # Embedding generation demo
├── search_example.rs        # Semantic search demo
└── indexing_example.rs      # Document indexing demo
```

### `/tests` - Integration Tests

```
tests/
└── api_integration_tests.rs # Comprehensive API tests
```

### `/migrations` - Database Migrations

```
migrations/
├── 20240101000000_initial_schema.sql
├── 20240102000000_add_webhooks.sql
├── 20240103000000_add_analytics.sql
└── 20240104000000_add_tenants.sql
```

### `/scripts` - Utility Scripts

```
scripts/
├── deployment/              # Deployment scripts
│   └── deploy-migrations.sh
├── testing/                 # Test scripts
│   ├── test-deployment.sh
│   └── test-phase1-documents.sh
└── run.sh                   # Development startup script
```

### `/config` - Configuration Files

```
config/
├── haproxy.cfg              # HAProxy load balancer config
├── prometheus.yml           # Prometheus metrics config
├── tempo.yaml               # Grafana Tempo tracing config
├── redis-sentinel.conf      # Redis Sentinel config
└── grafana/                 # Grafana configurations
    ├── provisioning/
    └── dashboards/
```

### `/docker` - Docker Configurations

```
docker/
└── Dockerfile               # Production Docker image
```

### `/static` - Static Web Assets

```
static/
├── css/                     # Stylesheets
├── js/                      # JavaScript files
└── images/                  # Image assets
```

## 📋 Key Files

### Root Level

| File | Purpose |
|------|---------|
| `Cargo.toml` | Rust project configuration and dependencies |
| `Cargo.lock` | Locked dependency versions |
| `README.md` | Project overview and quick start |
| `CONTRIBUTING.md` | Contribution guidelines |
| `PROJECT_STATUS.md` | Current status and roadmap |
| `PROJECT_STRUCTURE.md` | This file - project navigation |
| `LICENSE` | MIT License |
| `askama.toml` | Askama template configuration |
| `.gitignore` | Git ignore rules |
| `.dockerignore` | Docker ignore rules |

### Docker Compose Files

| File | Purpose |
|------|---------|
| `docker-compose.yml` | Simple development setup (PostgreSQL + Redis) |
| `docker-compose.advanced.yml` | Full stack with HA (PostgreSQL, Redis, Jaeger, Grafana, Prometheus, HAProxy, 3 app instances) |
| `docker-compose.prod.yml` | Production configuration |

## 🎯 Module Responsibilities

### Core Modules (RAG Functionality)

- **`chunking`** - Split documents into searchable chunks
- **`embeddings`** - Generate vector embeddings using FastEmbed
- **`indexing`** - Index documents with chunks and embeddings
- **`search`** - Semantic, keyword, and hybrid search
- **`vector_index`** - HNSW-based vector similarity search
- **`db`** - Database operations and queries

### Enterprise Modules (Advanced Features)

- **`cache_layer`** - Redis + memory distributed caching
- **`telemetry`** - OpenTelemetry distributed tracing
- **`query_analytics`** - Search pattern analysis and insights
- **`multi_tenant`** - Organization isolation and quotas
- **`webhooks`** - Event notification system
- **`metrics`** - Prometheus metrics collection

### API & Web Modules

- **`api`** - REST API endpoints and middleware
- **`web_ui`** - Web interface routing
- **`templates`** - HTML templates for admin dashboard

### Support Modules

- **`config`** - Configuration management
- **`error`** - Error types and handling
- **`types`** - Common type definitions

## 📊 Code Statistics

| Component | Files | Lines of Code | Tests |
|-----------|-------|---------------|-------|
| Core RAG | 6 | ~2,500 | 15+ |
| Enterprise Features | 5 | ~3,500 | 16+ |
| API & Web | 8 | ~2,000 | 10+ |
| Templates | 3 | ~1,200 | - |
| Examples | 5 | ~500 | - |
| Tests | 1 | ~800 | - |
| **Total** | **28+** | **~10,500** | **41+** |

## 🗺️ Navigation Guide

### New Contributors

1. Start with [`README.md`](README.md) - Project overview
2. Read [`CONTRIBUTING.md`](CONTRIBUTING.md) - How to contribute
3. Check [`docs/guides/QUICK_START.md`](docs/guides/QUICK_START.md) - Setup guide
4. Review this file for project layout

### Users

1. [`README.md`](README.md) - Quick start and features
2. [`docs/guides/QUICK_START.md`](docs/guides/QUICK_START.md) - Detailed setup
3. [`docs/RAG_API.md`](docs/RAG_API.md) - API reference
4. [`docs/guides/ADVANCED_FEATURES_GUIDE.md`](docs/guides/ADVANCED_FEATURES_GUIDE.md) - Advanced usage

### Developers

1. [`CONTRIBUTING.md`](CONTRIBUTING.md) - Development guidelines
2. [`src/lib.rs`](src/lib.rs) - Module exports and structure
3. [`examples/`](examples/) - Code examples
4. [`tests/`](tests/) - Integration tests

### DevOps

1. [`docker-compose.advanced.yml`](docker-compose.advanced.yml) - Full stack setup
2. [`config/`](config/) - Service configurations
3. [`scripts/deployment/`](scripts/deployment/) - Deployment scripts
4. [`docs/guides/ADVANCED_FEATURES_GUIDE.md`](docs/guides/ADVANCED_FEATURES_GUIDE.md#deployment) - Deployment guide

## 🔄 Build Artifacts

Generated during build (not in git):

```
target/                      # Build output
├── debug/                   # Debug builds
├── release/                 # Release builds
└── doc/                     # Generated documentation

data/                        # Runtime data (gitignored)
├── rustassistant.db         # SQLite database
├── cache/                   # Cache files
└── logs/                    # Log files
```

## 📦 Dependencies

### Main Dependencies

- **axum** - Web framework
- **sqlx** - Database operations
- **tokio** - Async runtime
- **fastembed** - Embeddings
- **redis** - Distributed cache
- **opentelemetry** - Tracing
- **askama** - Templates
- **serde** - Serialization

See [`Cargo.toml`](Cargo.toml) for complete list.

## 🎯 Finding What You Need

| I want to... | Look in... |
|--------------|------------|
| Add a new API endpoint | `src/api/handlers.rs` |
| Modify caching logic | `src/cache_layer.rs` |
| Change search algorithm | `src/search.rs` |
| Add tracing to a function | `src/telemetry.rs` |
| Update admin dashboard | `src/templates/pages/admin.html` |
| Add a new module | `src/` + update `src/lib.rs` |
| Write integration tests | `tests/` |
| Add documentation | `docs/` |
| Modify Docker setup | `docker-compose*.yml` |
| Add deployment config | `config/` |

## 📞 Questions?

- Check [`README.md`](README.md) first
- See [`CONTRIBUTING.md`](CONTRIBUTING.md) for dev questions
- Browse [`docs/`](docs/) for detailed guides
- Open an issue on GitHub for support

---

**Last Updated:** January 2024  
**Maintained by:** RustAssistant Contributors