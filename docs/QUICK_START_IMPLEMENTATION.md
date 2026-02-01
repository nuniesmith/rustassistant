# Quick Start Implementation Guide

> **Goal**: Get Rustassistant working in the next 2-4 hours with a minimal but functional version.

---

## 🎯 Minimal Viable Product (MVP)

Build these features first:
1. ✅ Basic Axum server with health check
2. ✅ SQLite database with notes table
3. ✅ REST API to create/list notes
4. ✅ Simple CLI to add notes
5. ✅ Basic XAI Grok integration test

Everything else comes later.

---

## 📋 Pre-flight Checklist

```bash
# 1. Verify Rust installation
rustc --version  # Should be 1.75+

# 2. Verify cargo is working
cargo --version

# 3. Test compilation
cd audit
cargo check

# 4. Get XAI API key
# Go to https://console.x.ai and get your key
export XAI_API_KEY="xai-your-key-here"

# 5. Create data directory
mkdir -p data/{repos,vectors,notes}
```

---

## 🚀 Step-by-Step Implementation

### Step 1: Update Environment (5 minutes)

```bash
# Copy example env
cp .env.example .env

# Edit .env and add your real XAI_API_KEY
nano .env
```

### Step 2: Create Database Module (15 minutes)

Create `src/db.rs`:

```rust
use anyhow::Result;
use sqlx::{sqlite::SqlitePool, Row};

pub async fn init_db(database_url: &str) -> Result<SqlitePool> {
    let pool = SqlitePool::connect(database_url).await?;
    
    // Create notes table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            tags TEXT,
            project TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;
    
    Ok(pool)
}

pub struct Note {
    pub id: String,
    pub content: String,
    pub tags: Option<String>,
    pub project: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub async fn create_note(
    pool: &SqlitePool,
    content: &str,
    tags: Option<&str>,
    project: Option<&str>,
) -> Result<Note> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    
    sqlx::query(
        "INSERT INTO notes (id, content, tags, project, created_at, updated_at) 
         VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(content)
    .bind(tags)
    .bind(project)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    
    Ok(Note {
        id,
        content: content.to_string(),
        tags: tags.map(|s| s.to_string()),
        project: project.map(|s| s.to_string()),
        created_at: now,
        updated_at: now,
    })
}

pub async fn list_notes(pool: &SqlitePool, limit: i64) -> Result<Vec<Note>> {
    let notes = sqlx::query(
        "SELECT id, content, tags, project, created_at, updated_at 
         FROM notes 
         ORDER BY created_at DESC 
         LIMIT ?"
    )
    .bind(limit)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| Note {
        id: row.get("id"),
        content: row.get("content"),
        tags: row.get("tags"),
        project: row.get("project"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
    .collect();
    
    Ok(notes)
}
```

Add to `src/lib.rs`:
```rust
pub mod db;
```

Update `Cargo.toml` to add sqlx:
```toml
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite"] }
```

### Step 3: Update Server with Notes API (20 minutes)

Update `src/bin/server.rs`:

```rust
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use devflow::db::{create_note, init_db, list_notes};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load env
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();
    
    // Init database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:data/devflow.db".to_string());
    let db = init_db(&database_url).await?;
    
    let state = AppState { db };
    
    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/notes", post(create_note_handler))
        .route("/api/notes", get(list_notes_handler))
        .with_state(state);
    
    // Start server
    let addr = "127.0.0.1:3000";
    println!("🚀 Rustassistant server starting on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "devflow"
    }))
}

#[derive(Deserialize)]
struct CreateNoteRequest {
    content: String,
    tags: Option<String>,
    project: Option<String>,
}

#[derive(Serialize)]
struct NoteResponse {
    id: String,
    content: String,
    tags: Option<String>,
    project: Option<String>,
    created_at: i64,
}

async fn create_note_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<NoteResponse>), StatusCode> {
    let note = create_note(
        &state.db,
        &req.content,
        req.tags.as_deref(),
        req.project.as_deref(),
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok((
        StatusCode::CREATED,
        Json(NoteResponse {
            id: note.id,
            content: note.content,
            tags: note.tags,
            project: note.project,
            created_at: note.created_at,
        }),
    ))
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    10
}

async fn list_notes_handler(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<NoteResponse>>, StatusCode> {
    let notes = list_notes(&state.db, query.limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let response: Vec<NoteResponse> = notes
        .into_iter()
        .map(|n| NoteResponse {
            id: n.id,
            content: n.content,
            tags: n.tags,
            project: n.project,
            created_at: n.created_at,
        })
        .collect();
    
    Ok(Json(response))
}
```

### Step 4: Create Simple CLI (20 minutes)

Update `src/bin/cli.rs`:

```rust
use clap::{Parser, Subcommand};
use devflow::db::{create_note, init_db, list_notes};

#[derive(Parser)]
#[command(name = "devflow")]
#[command(about = "Developer workflow management tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new note
    Note {
        #[command(subcommand)]
        action: NoteAction,
    },
    /// Test API connection
    TestApi,
}

#[derive(Subcommand)]
enum NoteAction {
    /// Add a new note
    Add {
        /// Note content
        content: String,
        /// Tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
        /// Project name
        #[arg(short, long)]
        project: Option<String>,
    },
    /// List notes
    List {
        /// Number of notes to show
        #[arg(short, long, default_value = "10")]
        limit: i64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Note { action } => handle_note_action(action).await?,
        Commands::TestApi => test_api().await?,
    }
    
    Ok(())
}

async fn handle_note_action(action: NoteAction) -> anyhow::Result<()> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:data/devflow.db".to_string());
    let db = init_db(&database_url).await?;
    
    match action {
        NoteAction::Add { content, tags, project } => {
            let note = create_note(
                &db,
                &content,
                tags.as_deref(),
                project.as_deref(),
            )
            .await?;
            
            println!("✅ Note created: {}", note.id);
            println!("   Content: {}", note.content);
            if let Some(t) = note.tags {
                println!("   Tags: {}", t);
            }
        }
        NoteAction::List { limit } => {
            let notes = list_notes(&db, limit).await?;
            
            if notes.is_empty() {
                println!("📝 No notes yet. Add one with: devflow note add \"Your note\"");
            } else {
                println!("📝 Notes ({}):\n", notes.len());
                for note in notes {
                    println!("  [{}]", note.id);
                    println!("  {}", note.content);
                    if let Some(tags) = note.tags {
                        println!("  Tags: {}", tags);
                    }
                    println!();
                }
            }
        }
    }
    
    Ok(())
}

async fn test_api() -> anyhow::Result<()> {
    let api_key = std::env::var("XAI_API_KEY")
        .expect("XAI_API_KEY not set");
    
    println!("🧪 Testing XAI API connection...");
    
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.x.ai/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": "grok-beta",
            "messages": [
                {"role": "user", "content": "Say 'API test successful' and nothing else."}
            ],
            "max_tokens": 10
        }))
        .send()
        .await?;
    
    if response.status().is_success() {
        println!("✅ XAI API: Connected");
        let body: serde_json::Value = response.json().await?;
        if let Some(content) = body["choices"][0]["message"]["content"].as_str() {
            println!("✅ Response: {}", content.trim());
        }
    } else {
        println!("❌ API Error: {}", response.status());
        println!("Response: {}", response.text().await?);
    }
    
    Ok(())
}
```

### Step 5: Test Everything (10 minutes)

```bash
# 1. Build
cargo build --release

# 2. Test CLI - add a note
./target/release/devflow note add "First note" --tags "test,idea"

# 3. Test CLI - list notes
./target/release/devflow note list

# 4. Test API connection
./target/release/devflow test-api

# 5. Start server in one terminal
./target/release/devflow-server

# 6. Test API in another terminal
# Health check
curl http://localhost:3000/health

# Create note via API
curl -X POST http://localhost:3000/api/notes \
  -H "Content-Type: application/json" \
  -d '{"content": "Note via API", "tags": "api,test"}'

# List notes via API
curl http://localhost:3000/api/notes
```

---

## ✅ Success Criteria

You've successfully set up Rustassistant MVP when:

- [x] Server starts without errors
- [x] Health endpoint returns 200 OK
- [x] Can create notes via CLI
- [x] Can list notes via CLI
- [x] Can create notes via API
- [x] Can list notes via API
- [x] XAI API test passes
- [x] Database persists data (check `data/devflow.db`)

---

## 🐛 Troubleshooting

### Error: "no such table: notes"
```bash
# Delete the db and restart server (it will recreate)
rm data/devflow.db
./target/release/devflow-server
```

### Error: "XAI_API_KEY not set"
```bash
# Make sure .env file exists and has your key
cat .env | grep XAI_API_KEY

# Or export directly
export XAI_API_KEY="xai-your-key"
```

### Error: "address already in use"
```bash
# Kill existing server
pkill devflow-server

# Or change port in .env
PORT=3001
```

### Compilation errors with sqlx
```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features sqlite

# Or use offline mode (add to .cargo/config.toml)
[env]
SQLX_OFFLINE = "true"
```

---

## 📈 Next Steps (Priority Order)

### Phase 1A: Core Features (Next 4-8 hours)
1. **Repository tracking**
   - Add `repositories` table
   - Create repo scanner
   - Cache directory tree
   - API endpoints for repos

2. **Basic file analysis**
   - Scan for TODOs
   - Count lines/files
   - Detect languages
   - Store metrics

3. **Simple task system**
   - Add `tasks` table
   - Generate tasks from TODOs
   - Priority sorting
   - Status tracking

### Phase 1B: LLM Integration (Next 8 hours)
1. **File scoring**
   - Send file to Grok
   - Get quality score
   - Extract issues
   - Cache results

2. **Cost tracking**
   - Track tokens used
   - Calculate costs
   - Enforce budget
   - Show usage stats

### Phase 1C: Web UI (Next 8-12 hours)
1. **Basic HTML interface**
   - Notes page
   - Repos page
   - Tasks page
   - Dashboard

2. **HTMX integration**
   - Live updates
   - Form submissions
   - Partial refreshes

---

## 🎯 Daily Goals

### Day 1: Foundation ✅
- [x] Server + database working
- [x] Notes CRUD complete
- [x] CLI functional
- [x] API tested

### Day 2: Repository Tracking
- [ ] Repo scanner working
- [ ] Directory tree cached
- [ ] Basic metrics collected
- [ ] API endpoints ready

### Day 3: LLM Integration
- [ ] File scoring implemented
- [ ] Cost tracking working
- [ ] Results cached
- [ ] Budget enforced

### Day 4: Task System
- [ ] Tasks generated from analysis
- [ ] Priority algorithm working
- [ ] Next action recommendation
- [ ] CLI commands complete

### Week 2: Web UI + Polish
- [ ] Basic web interface
- [ ] Real-time updates
- [ ] Documentation complete
- [ ] Docker deployment tested

---

## 💡 Development Tips

### Use `cargo watch` for faster iteration
```bash
cargo install cargo-watch
cargo watch -x 'run --bin devflow-server'
```

### Enable better error messages
```bash
export RUST_BACKTRACE=1
export RUST_LOG=debug,devflow=trace
```

### Test with example repos
```bash
# Clone some small repos for testing
mkdir test-repos
cd test-repos
git clone https://github.com/rust-lang/rustlings.git
```

### Keep notes of decisions
```bash
# Create a dev journal
echo "$(date): Decided to use SQLite for Phase 1" >> dev-journal.md
```

---

## 📚 Helpful Commands

```bash
# Format code
cargo fmt

# Check for issues
cargo clippy

# Run tests
cargo test

# Build optimized
cargo build --release

# Check without building
cargo check

# Update dependencies
cargo update

# Generate docs
cargo doc --open
```

---

## 🎉 You're Ready!

You now have:
- ✅ Working server
- ✅ Database with notes
- ✅ REST API
- ✅ CLI tool
- ✅ Foundation for everything else

**Next**: Pick a feature from Phase 1A and start building!

**Remember**: Ship small, working increments. Don't try to build everything at once.

---

**Questions?** Check docs/RESEARCH_GUIDE.md or docs/ROADMAP.md

**Happy coding!** 🚀