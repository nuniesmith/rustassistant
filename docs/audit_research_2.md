# Building a Rust RAG system with Grok 4.1 for Rustassistant

**Grok 4.1's 2-million-token context window and $0.20/M input pricing fundamentally changes RAG architecture decisions.** For Rustassistant tracking GitHub repos, notes, and project files, you can often skip complex chunking entirely and load entire file trees directly into context. The optimal MVP stack: LanceDB (embedded, no Docker needed) + fastembed-rs for local embeddings + async-openai configured for xAI. Total time to working prototype: a weekend.

## xAI has no embedding API—use fastembed-rs locally

A critical discovery: **xAI does not offer an embedding API**. The official docs at docs.x.ai list chat completions, image generation, and tool calling, but no embeddings endpoint. This means you need a separate embedding solution.

**fastembed-rs** emerges as the clear winner for a solo Rust developer:

```rust
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

let model = TextEmbedding::try_new(
    InitOptions::new(EmbeddingModel::MxbaiEmbedLargeV1)
        .with_show_download_progress(true)
)?;
let embeddings = model.embed(vec!["Your text here"], None)?;
```

The crate uses ONNX Runtime under the hood—no PyTorch installation, no Python dependencies, pure Rust convenience. The `mxbai-embed-large-v1` model produces **1024-dimensional** embeddings with quality rivaling OpenAI's offerings. For code-heavy content, consider also using **Voyage AI's voyage-code-3** (200M tokens free) via reqwest for the initial indexing pass, as it benchmarks **13.8% better** than OpenAI on code retrieval tasks.

| Embedding Approach | Cost per 1M tokens | Quality | Setup Complexity |
|-------------------|-------------------|---------|------------------|
| fastembed-rs (local) | $0 | Good | `cargo add fastembed` |
| OpenAI text-embedding-3-small | $0.02 | Excellent | API key + reqwest |
| Voyage AI voyage-code-3 | $0.18 (200M free) | Best for code | API key + reqwest |

## LanceDB eliminates Docker for your vector store

For a solo developer building an MVP, **LanceDB** provides the simplest path: an embedded, file-based vector database written in Rust that requires zero infrastructure.

```toml
[dependencies]
lancedb = "0.23.1"
```

```rust
use lancedb::connect;
let db = connect("data/devflow-vectors").execute().await?;
```

Your vectors live in a local directory (`data/devflow-vectors/`) using Lance columnar format with **built-in versioning**—each insert creates a version, similar to git commits. While not human-readable (binary format), this is far more git-friendly than a running database server. For true version control, use Git LFS for the Lance files.

**Qdrant** remains the production upgrade path when you outgrow embedded storage. It's written entirely in Rust and deploys as a single Docker container:

```yaml
services:
  qdrant:
    image: qdrant/qdrant
    ports: ["6333:6333", "6334:6334"]
    volumes: ["./qdrant_storage:/qdrant/storage"]
```

The migration is straightforward: both support the same embedding dimensions, and LanceDB data can be exported to Qdrant via simple iteration.

## Grok 4.1's context window changes everything

The **2-million-token context** (verified in official docs) at **$0.20/M input** means radically different RAG architecture. Consider what fits in 2M tokens:

- Your entire codebase directory tree with file contents: likely fits
- All your notes and ideas: almost certainly fits  
- House building project documents: probably fits

For Rustassistant's scale, you may not need sophisticated chunking at all. A "context stuffing" approach becomes viable:

```rust
async fn query_with_full_context(
    user_query: &str,
    repo_contents: &str,
    notes: &str,
) -> Result<String> {
    let system_prompt = format!(
        "You are Rustassistant, a personal assistant with access to:\n\n\
        ## Repository Contents\n{}\n\n\
        ## Notes and Ideas\n{}\n\n\
        Answer questions using this context.",
        repo_contents, notes
    );
    
    call_grok(&system_prompt, user_query).await
}
```

For larger datasets where 2M tokens isn't enough, hybrid retrieval becomes essential: vector search for semantic similarity, combined with tag/keyword filtering for your categorized notes.

## The async-openai crate works directly with xAI

xAI's API is **fully OpenAI-compatible**, meaning the mature `async-openai` crate works out of the box with a simple config change:

```rust
use async_openai::{Client, config::OpenAIConfig};

let config = OpenAIConfig::new()
    .with_api_key(std::env::var("XAI_API_KEY")?)
    .with_api_base("https://api.x.ai/v1");
let client = Client::with_config(config);

let request = CreateChatCompletionRequestArgs::default()
    .model("grok-4-1-fast-reasoning")
    .messages(vec![
        ChatCompletionRequestMessage::System(system_content),
        ChatCompletionRequestMessage::User(user_query.into()),
    ])
    .build()?;

let response = client.chat().create(request).await?;
```

Streaming works identically via `client.chat().create_stream(request)`. For rate limit handling, xAI returns standard 429 errors with `x-rate-limit-reset` headers—use the `backoff` crate for exponential retry.

## Practical RAG architecture for Rustassistant

Given Rustassistant's use case (GitHub repos, tagged notes, project documents), here's the recommended architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                     Rustassistant RAG System                       │
├─────────────────────────────────────────────────────────────┤
│  Content Sources                                             │
│  ├── GitHub repos (file trees, cached locally)              │
│  ├── Notes (markdown with YAML frontmatter for tags)        │
│  └── Project docs (house planning PDFs, images)             │
├─────────────────────────────────────────────────────────────┤
│  Indexing Pipeline (batch, on content change)               │
│  ├── Chunker: tree-sitter for code, markdown-aware for notes│
│  ├── Embeddings: fastembed-rs MxbaiEmbedLargeV1 (1024d)     │
│  └── Storage: LanceDB (file-based, versioned)               │
├─────────────────────────────────────────────────────────────┤
│  Query Pipeline (real-time)                                  │
│  ├── Embed query with same model                            │
│  ├── Hybrid search: vector similarity + tag filtering       │
│  ├── Context assembly (respect 2M token budget)             │
│  └── LLM: Grok 4.1 Fast via async-openai                    │
└─────────────────────────────────────────────────────────────┘
```

**Chunking strategy by content type:**

| Content | Strategy | Implementation |
|---------|----------|----------------|
| Code files | Function/class boundaries | tree-sitter parsing via Swiftide |
| Markdown notes | Header-based sections | Split on `##` with 512-token max |
| Project docs | Paragraph + semantic | embed_anything crate |

For hybrid search with tag filtering:

```rust
struct RustassistantSearcher {
    vector_index: lancedb::Table,
    tag_index: HashMap<String, HashSet<DocumentId>>,
}

impl RustassistantSearcher {
    async fn search(&self, query: &str, tags: &[String], k: usize) -> Vec<Chunk> {
        let query_embedding = self.embed(query)?;
        
        // Vector search returns candidates
        let candidates = self.vector_index
            .search(&query_embedding)
            .limit(k * 3)  // Over-fetch for filtering
            .execute().await?;
        
        // Filter by tags if specified
        candidates
            .filter(|c| tags.is_empty() || c.tags.iter().any(|t| tags.contains(t)))
            .take(k)
            .collect()
    }
}
```

## Git-friendly vector storage is partially achievable

True git tracking of vectors faces challenges: embedding files are binary, large, and change frequently. However, several approaches work for Rustassistant's scale:

**Option 1: LanceDB + Git LFS** (Recommended)
Lance files version naturally. Add to `.gitattributes`:
```
*.lance filter=lfs diff=lfs merge=lfs -text
data/vectors/** filter=lfs diff=lfs merge=lfs -text
```

**Option 2: sqlite-vec for small datasets**
A single SQLite file containing vectors is easier to track:
```sql
CREATE VIRTUAL TABLE note_embeddings USING vec0(embedding float[1024]);
```

**Option 3: Export to JSON periodically**
For truly git-readable history, serialize embeddings to JSON—not space-efficient but diff-able:
```rust
#[derive(Serialize)]
struct EmbeddingRecord {
    id: String,
    source_hash: String,  // Hash of source content
    embedding: Vec<f32>,
}
```

## Recommended MVP implementation path

**Week 1: Core infrastructure**
```toml
[dependencies]
lancedb = "0.23.1"
fastembed = "5"
async-openai = "0.27"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

Build: Document loader → Chunker → Embedding generator → LanceDB storage

**Week 2: Query pipeline**
- Implement vector search with tag filtering
- Add Grok 4.1 integration via async-openai
- Build simple CLI or API interface

**Week 3: Rustassistant-specific features**
- GitHub repo crawler with file caching
- Note ingestion with frontmatter tag parsing  
- Incremental indexing (only re-embed changed files)

## Cost analysis for personal use

At Rustassistant's scale, costs are negligible:

| Operation | Estimated Volume | Cost |
|-----------|-----------------|------|
| Initial indexing (embeddings) | 100K tokens | $0 (local) |
| Daily queries to Grok 4.1 | 50 queries × 10K tokens | $0.10/day |
| Monthly embedding updates | 200K tokens | $0 (local) |
| **Monthly total** | — | **~$3** |

Using fastembed-rs locally eliminates embedding API costs entirely. Grok 4.1's pricing makes even aggressive usage affordable.

## Conclusion

The Rust RAG ecosystem has matured significantly. **LanceDB + fastembed-rs + async-openai** provides a complete, production-viable stack with minimal dependencies and zero required infrastructure. Grok 4.1's massive context window and aggressive pricing make it ideal for personal developer tools—you can often bypass complex retrieval entirely by loading full context.

For Rustassistant specifically: start with the "context stuffing" approach (load everything into Grok's 2M window), then add vector retrieval only when your content exceeds that limit. The frameworks exist (Swiftide for production, Rig for simplicity), but for a solo developer, hand-rolling the pipeline with the recommended crates offers more control and understanding of your system.