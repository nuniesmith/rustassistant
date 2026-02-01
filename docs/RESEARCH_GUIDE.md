# Research Guide: Building a Rust Web App with RAG System

> **Purpose**: Guide research and architectural decisions for Rustassistant's Rust-based web application with LLM RAG capabilities.

---

## 🎯 Project Context

**Goal**: Build a lightweight, self-hostable Rust web app for solo developer workflow management with:
- Note-taking and thought capture
- Repository tracking and caching
- LLM-powered code analysis (Grok 4.1)
- RAG system for semantic search
- Task generation and prioritization
- Git-friendly vector storage

**Constraints**:
- Must run on home server initially, then Linode
- Budget-conscious LLM usage
- Simple deployment (Docker Compose)
- Solo developer focused (team features optional later)

---

## 📚 Research Topics

### 1. Rust Web Frameworks

#### Axum (Current Choice)
**Pros**:
- Modern, built on tokio and tower
- Type-safe, excellent error handling
- Good ecosystem (tower middleware)
- Fast compilation (compared to actix)
- WebSocket support
- Extract pattern is elegant

**Cons**:
- Smaller community than actix-web
- Fewer examples and tutorials
- Some middleware may need custom impl

**Research Questions**:
- [ ] How to structure a medium-sized Axum app? (routing, state, middleware)
- [ ] Best practices for error handling in Axum?
- [ ] Session management strategies?
- [ ] Static file serving patterns?
- [ ] WebSocket integration for real-time updates?

**Resources**:
- Official examples: https://github.com/tokio-rs/axum/tree/main/examples
- Axum book (community): https://github.com/programatik29/axum-tutorial
- Tower middleware: https://docs.rs/tower/latest/tower/

#### Alternative: actix-web
**When to consider**:
- If you need proven production patterns
- Larger community and more examples
- Actor model fits your use case

**Research**: Compare Axum vs actix-web for this specific use case

#### Alternative: Rocket
**When to consider**:
- If you prefer macro-heavy, Rails-like framework
- Want batteries-included approach

**Research**: Does Rocket's async support match Axum's maturity now?

### 2. Database Strategy

#### Phase 1: SQLite (Recommended)
**Pros**:
- Zero configuration
- Single file, easy backup
- Perfect for local/home server
- Fast for read-heavy workloads
- Built-in full-text search (FTS5)

**Cons**:
- Limited concurrency (writes)
- Not ideal for team features later
- Size limits (though unlikely to hit)

**Research Questions**:
- [ ] SQLite connection pooling with sqlx?
- [ ] Migration strategy (sqlx-cli vs diesel)?
- [ ] FTS5 for note search performance?
- [ ] WAL mode for better concurrency?
- [ ] When to migrate to PostgreSQL?

**Schema Design**:
```sql
-- Notes table
CREATE TABLE notes (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    tags TEXT, -- JSON array or separate table?
    project_id TEXT,
    created_at TIMESTAMP,
    updated_at TIMESTAMP
);

-- Repositories table
CREATE TABLE repositories (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT,
    last_analyzed TIMESTAMP,
    metadata TEXT -- JSON
);

-- Tasks table
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    priority INTEGER,
    status TEXT,
    repo_id TEXT,
    created_at TIMESTAMP
);
```

#### Phase 2: PostgreSQL
**When to migrate**:
- Team features needed
- Concurrency becomes an issue
- Need advanced features (pgvector, partitioning)

**Research Questions**:
- [ ] Use pgvector for embeddings instead of JSON files?
- [ ] Connection pooling strategy (r2d2, deadpool)?
- [ ] Migration path from SQLite?

### 3. ORM/Query Builder

#### Option A: sqlx (Recommended)
**Pros**:
- Compile-time query checking
- Async/await native
- No heavy macros
- Works with SQLite and PostgreSQL

**Cons**:
- More verbose than diesel
- Manual migrations

**Example**:
```rust
let note = sqlx::query_as!(
    Note,
    "SELECT * FROM notes WHERE id = ?",
    id
)
.fetch_one(&pool)
.await?;
```

**Research**:
- [ ] sqlx-cli for migrations
- [ ] Macro vs runtime query checking
- [ ] Transaction handling patterns

#### Option B: diesel
**Pros**:
- Mature, battle-tested
- Type-safe query builder
- Good migration tooling

**Cons**:
- Async support still maturing (diesel-async)
- Heavier compile times
- More magic (can be confusing)

**Research**: Is diesel-async production ready?

#### Option C: SeaORM
**Pros**:
- Modern, async-first
- Active development
- Nice migration tools

**Cons**:
- Smaller community
- Less mature

**Research**: Community adoption and stability?

### 4. Vector Storage for RAG

#### Option A: Git-Friendly JSON Files (Phase 1)
**Structure**:
```
data/vectors/
├── repo-name/
│   ├── src/
│   │   ├── main.rs.vec.json
│   │   └── lib.rs.vec.json
│   └── metadata.json
```

**Format**:
```json
{
  "file": "src/main.rs",
  "chunks": [
    {
      "id": "chunk-1",
      "content": "...",
      "embedding": [0.123, 0.456, ...],
      "start_line": 1,
      "end_line": 50
    }
  ],
  "version": "1.0",
  "generated_at": "2025-01-31T..."
}
```

**Pros**:
- Version controlled with git
- Human-inspectable
- No external dependencies
- Easy incremental updates

**Cons**:
- Slower search than vector DB
- Not scalable to millions of vectors
- Manual similarity search implementation

**Research Questions**:
- [ ] Efficient cosine similarity in Rust?
- [ ] Lazy loading strategy for large repos?
- [ ] Compression options (gzip, zstd)?
- [ ] Index structure for faster search?

#### Option B: Qdrant (Phase 2)
**When to consider**:
- Search becomes slow (>10k vectors)
- Need advanced filtering
- Production deployment

**Pros**:
- Purpose-built vector database
- Fast similarity search
- Filtering and metadata support
- Self-hostable

**Cons**:
- Additional infrastructure
- More complex deployment

**Research**:
- [ ] Qdrant Rust client quality
- [ ] Docker Compose integration
- [ ] Backup/restore strategies
- [ ] Migration from JSON files

#### Option C: pgvector (PostgreSQL extension)
**When to consider**:
- Already using PostgreSQL
- Want single database solution

**Pros**:
- No separate vector DB
- SQL-based querying
- Mature and stable

**Cons**:
- Slower than dedicated vector DBs
- PostgreSQL dependency

**Research**: Performance benchmarks vs Qdrant

### 5. Embedding Generation

#### Option A: OpenAI API
**Model**: `text-embedding-3-small` (1536 dimensions)
**Cost**: ~$0.02 per 1M tokens
**Pros**: High quality, well-tested
**Cons**: External API, privacy concerns

#### Option B: Local Models (Rust)
**Libraries to research**:
- [ ] `rust-bert` - BERT models in Rust
- [ ] `candle` - ML framework by Hugging Face
- [ ] `ort` (ONNX Runtime) - Run exported models

**Trade-offs**:
- Slower but private
- Need GPU for reasonable speed
- Model size (~400MB+)

**Research**:
- [ ] Performance benchmarks
- [ ] Memory usage
- [ ] CPU vs GPU inference time

#### Hybrid Approach
Use OpenAI for initial embedding, cache results, run local for incremental updates.

### 6. LLM Integration Architecture

#### Current: Direct HTTP Client
```rust
pub struct GrokClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl GrokClient {
    pub async fn chat(&self, messages: Vec<Message>) -> Result<Response> {
        // Direct API call
    }
}
```

**Research Questions**:
- [ ] Should we use a library like `async-openai`?
- [ ] How to handle streaming responses?
- [ ] Retry strategy for rate limits?
- [ ] Circuit breaker pattern for cost control?

#### Caching Strategy
```rust
// Cache key: hash(prompt + model + temperature)
pub struct LlmCache {
    store: HashMap<String, CachedResponse>,
    ttl: Duration,
}
```

**Research**:
- [ ] Cache invalidation strategy?
- [ ] Disk vs memory cache?
- [ ] Redis for distributed caching (future)?

#### Cost Control
```rust
pub struct CostTracker {
    daily_limit: f64,
    current_usage: f64,
    warnings_sent: Vec<DateTime>,
}

impl CostTracker {
    pub fn should_allow_request(&self, estimated_cost: f64) -> Result<()> {
        if self.current_usage + estimated_cost > self.daily_limit {
            Err(BudgetExceeded)
        } else {
            Ok(())
        }
    }
}
```

### 7. Web Frontend Strategy

#### Option A: HTMX + Server-Side Rendering
**Pros**:
- Minimal JavaScript
- Fast development
- Good UX with little complexity
- SEO-friendly

**Cons**:
- Less interactive than SPA
- Limited offline capability

**Example**:
```html
<button hx-post="/notes" hx-target="#notes-list">
    Add Note
</button>
```

**Research**:
- [ ] HTMX patterns for forms
- [ ] Real-time updates with SSE
- [ ] Template engine (askama, tera, maud)

#### Option B: Minimal JavaScript + REST API
**Pros**:
- More interactive
- Can build mobile app later
- Clear API boundary

**Cons**:
- More code to write
- CORS considerations

#### Option C: Full SPA (React/Svelte)
**Skip for Phase 1**: Too complex for initial version

**Research for Future**: Leptos (Rust WASM framework)

### 8. Session Management

#### Options to Research
1. **JWT tokens** - Stateless but larger
2. **Session cookies** - Server-side state
3. **tower-sessions** - Middleware for Axum

**Questions**:
- [ ] Solo dev = do we even need auth?
- [ ] API key auth for programmatic access?
- [ ] OAuth for GitHub integration?

### 9. Real-Time Features

#### Server-Sent Events (SSE)
For real-time updates without WebSocket complexity:
```rust
async fn analysis_stream(
    State(app_state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event>>> {
    // Stream analysis progress
}
```

**Research**:
- [ ] SSE vs WebSockets for this use case
- [ ] Browser compatibility
- [ ] Reconnection handling

### 10. Background Jobs

#### Option A: tokio::spawn
Simple for Phase 1:
```rust
tokio::spawn(async move {
    analyze_repository(repo_id).await
});
```

**Pros**: Simple, no dependencies
**Cons**: No persistence, lost on restart

#### Option B: Job Queue (Future)
- [ ] Research: tokio-cron-scheduler
- [ ] Research: apalis (job processing)
- [ ] Research: Redis-based queue

**Use cases**:
- Scheduled repo analysis
- Batch embedding generation
- Daily summaries

### 11. Deployment & Operations

#### Docker Compose Strategy
```yaml
services:
  devflow:
    build: .
    volumes:
      - ./data:/app/data  # Persist SQLite + vectors
    environment:
      - XAI_API_KEY=${XAI_API_KEY}
```

**Research Questions**:
- [ ] Multi-stage builds for smaller images?
- [ ] Health check implementations?
- [ ] Log aggregation (Loki)?
- [ ] Metrics (Prometheus)?

#### Reverse Proxy
```nginx
server {
    listen 80;
    server_name devflow.home;
    
    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
    }
}
```

**Research**:
- [ ] Nginx vs Caddy for simple setup
- [ ] SSL certificate (Let's Encrypt)
- [ ] Rate limiting

### 12. Testing Strategy

#### Unit Tests
```rust
#[tokio::test]
async fn test_note_creation() {
    let pool = setup_test_db().await;
    let note = create_note(&pool, "Test", &["tag1"]).await?;
    assert_eq!(note.content, "Test");
}
```

#### Integration Tests
```rust
#[tokio::test]
async fn test_api_note_endpoint() {
    let app = create_test_app().await;
    let response = app
        .oneshot(Request::builder()
            .method("POST")
            .uri("/api/notes")
            .body(Body::from(r#"{"content":"test"}"#))
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::CREATED);
}
```

**Research**:
- [ ] Test database setup/teardown
- [ ] Mocking LLM API calls
- [ ] Load testing tools (wrk, bombardier)

---

## 🗺️ Recommended Research Order

### Week 1: Foundation
1. [ ] Axum project structure patterns (medium-sized apps)
2. [ ] sqlx setup and migrations
3. [ ] Error handling patterns in async Rust
4. [ ] Basic REST API design

### Week 2: Storage
1. [ ] SQLite FTS5 for note search
2. [ ] JSON vector file format design
3. [ ] Cosine similarity implementation
4. [ ] File watching for incremental updates

### Week 3: LLM Integration
1. [ ] XAI API client (study existing code)
2. [ ] Prompt engineering for code analysis
3. [ ] Cost tracking implementation
4. [ ] Response caching strategy

### Week 4: Frontend
1. [ ] HTMX basics and patterns
2. [ ] Template engine comparison
3. [ ] Static asset serving
4. [ ] Real-time updates with SSE

---

## 📖 Learning Resources

### Rust Web Development
- **Book**: "Zero to Production in Rust" by Luca Palmieri
- **Course**: Rust Web Development (Manning)
- **Repo**: Axum examples - https://github.com/tokio-rs/axum/tree/main/examples
- **Blog**: Shuttle.rs blog (Rust web tutorials)

### Databases
- **Docs**: sqlx book - https://github.com/launchbadge/sqlx
- **Tutorial**: SQLite FTS5 guide
- **Article**: "Choosing a Rust ORM" comparisons

### Vector Search
- **Paper**: "Efficient and robust approximate nearest neighbor search"
- **Library**: HNSW implementations in Rust
- **Article**: "Building a RAG system from scratch"

### LLM Integration
- **Docs**: OpenAI API docs (similar to XAI)
- **Blog**: Anthropic's prompt engineering guide
- **Tutorial**: Building a code analysis agent

### Frontend
- **Docs**: HTMX documentation
- **Course**: HTMX + Axum tutorial
- **Repo**: Real-world HTMX examples

---

## 💻 Prototyping Experiments

### Experiment 1: Minimal Axum App
```bash
cargo new axum-test
cd axum-test
# Add dependencies
# Build hello world API
# Test with curl
```

### Experiment 2: SQLite + sqlx
```bash
# Create migrations
sqlx migrate add create_notes
# Test CRUD operations
# Benchmark FTS5 search
```

### Experiment 3: Vector Search
```rust
// Implement naive cosine similarity
// Test with 1000 vectors
// Measure search time
// Optimize if needed
```

### Experiment 4: LLM Cost Tracking
```rust
// Track tokens per request
// Calculate costs
// Test budget enforcement
// Verify caching saves money
```

---

## 🎯 Decision Framework

When researching, answer these questions:

1. **Complexity**: Is this the simplest solution that works?
2. **Dependencies**: Does this add many new dependencies?
3. **Performance**: Is this fast enough for solo dev use case?
4. **Scalability**: Can we optimize later if needed?
5. **Maintainability**: Can I understand this in 6 months?
6. **Cost**: Does this impact LLM or hosting costs?

**Default to**: Simple, boring, proven solutions first.

---

## 📝 Notes & Findings

### Research Log Template
```markdown
## [Date] - [Topic]

**Question**: What I'm trying to figure out

**Findings**:
- Point 1
- Point 2

**Decision**: What I chose and why

**References**:
- Link 1
- Link 2

**Next Steps**:
- [ ] Action item 1
```

---

## ✅ Pre-Implementation Checklist

Before starting Phase 1 implementation:

- [ ] Understand Axum routing and state
- [ ] Know how to use sqlx with SQLite
- [ ] Have a clear database schema
- [ ] Understand vector storage strategy
- [ ] Know LLM API cost structure
- [ ] Have Docker deployment plan
- [ ] Understand error handling patterns
- [ ] Know how to write tests

---

## 🚀 Ready to Build?

Once research is complete, start with:

1. Create minimal Axum server
2. Add SQLite database
3. Implement one feature end-to-end (e.g., notes)
4. Add tests
5. Iterate

**Remember**: Ship something working quickly, then improve.

---

**Last Updated**: January 31, 2025