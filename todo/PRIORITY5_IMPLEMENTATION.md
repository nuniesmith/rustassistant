# Priority 5: RAG/Document Integration - Implementation Plan

**Status:** 🚧 In Progress  
**Priority:** High  
**Estimated Time:** 15-20 hours  
**Started:** February 6, 2026

---

## 📋 Overview

Implement a RAG (Retrieval-Augmented Generation) system with document storage, semantic search, and intelligent context retrieval for LLM workflows.

### Goals

1. **Document Storage** - Store markdown docs, research, references, and notes
2. **Semantic Search** - Find relevant documents using vector embeddings
3. **Context Stuffing** - Automatically retrieve relevant context for LLM queries
4. **Web UI** - Upload, manage, and search documents
5. **Integration** - Connect with existing notes and repository systems

---

## 🏗️ Architecture

### Technology Stack

- **Database:** SQLite for metadata, document content
- **Embeddings:** fastembed-rs with `mxbai-embed-large-v1` model (already in use)
- **Vector Storage:** SQLite with vector extension OR LanceDB
- **Chunking:** Simple paragraph/sentence-based chunking
- **Search:** Cosine similarity for semantic search

### Data Flow

```
Document Upload → Parse & Chunk → Generate Embeddings → Store in DB
                                                              ↓
User Search Query → Generate Query Embedding → Similarity Search → Return Results
                                                                         ↓
LLM Context Retrieval → Find Relevant Chunks → Stuff into Prompt → Send to LLM
```

---

## 🗄️ Database Schema

### Migration 006: Documents & Embeddings

```sql
-- documents table: Stores document metadata and content
CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    content_type TEXT DEFAULT 'markdown' CHECK(content_type IN ('markdown', 'text', 'code', 'html')),
    source_type TEXT DEFAULT 'manual' CHECK(source_type IN ('manual', 'url', 'file', 'repo')),
    source_url TEXT,
    doc_type TEXT DEFAULT 'reference' CHECK(doc_type IN ('reference', 'research', 'tutorial', 'architecture', 'note', 'snippet')),
    tags TEXT, -- Comma-separated tags for backward compatibility
    repo_id TEXT, -- Link to repository if applicable
    file_path TEXT, -- Original file path if from repo
    word_count INTEGER DEFAULT 0,
    char_count INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    indexed_at INTEGER, -- When embeddings were last generated
    FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE SET NULL
);

-- document_chunks table: Stores document chunks for embedding
CREATE TABLE IF NOT EXISTS document_chunks (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    char_start INTEGER NOT NULL,
    char_end INTEGER NOT NULL,
    word_count INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
    UNIQUE(document_id, chunk_index)
);

-- document_embeddings table: Stores vector embeddings
-- Note: Using TEXT to store as JSON array for simplicity
-- Consider sqlite-vec extension or LanceDB for production
CREATE TABLE IF NOT EXISTS document_embeddings (
    id TEXT PRIMARY KEY,
    chunk_id TEXT NOT NULL UNIQUE,
    embedding TEXT NOT NULL, -- JSON array of floats
    model TEXT NOT NULL DEFAULT 'mxbai-embed-large-v1',
    dimension INTEGER NOT NULL DEFAULT 1024,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (chunk_id) REFERENCES document_chunks(id) ON DELETE CASCADE
);

-- document_tags junction table
CREATE TABLE IF NOT EXISTS document_tags (
    document_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (document_id, tag),
    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
    FOREIGN KEY (tag) REFERENCES tags(name) ON DELETE CASCADE
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_documents_repo_id ON documents(repo_id);
CREATE INDEX IF NOT EXISTS idx_documents_doc_type ON documents(doc_type);
CREATE INDEX IF NOT EXISTS idx_documents_created ON documents(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_document_chunks_doc_id ON document_chunks(document_id);
CREATE INDEX IF NOT EXISTS idx_document_embeddings_chunk_id ON document_embeddings(chunk_id);
CREATE INDEX IF NOT EXISTS idx_document_tags_tag ON document_tags(tag);
CREATE INDEX IF NOT EXISTS idx_document_tags_doc ON document_tags(document_id);

-- Views

-- documents_with_tags: Join documents with their tags
CREATE VIEW IF NOT EXISTS documents_with_tags AS
SELECT
    d.id,
    d.title,
    d.content,
    d.content_type,
    d.source_type,
    d.source_url,
    d.doc_type,
    d.repo_id,
    d.word_count,
    d.created_at,
    d.updated_at,
    d.indexed_at,
    GROUP_CONCAT(dt.tag, ',') as tag_list,
    COUNT(DISTINCT dc.id) as chunk_count
FROM documents d
LEFT JOIN document_tags dt ON d.id = dt.document_id
LEFT JOIN document_chunks dc ON d.id = dc.document_id
GROUP BY d.id;

-- document_stats: Statistics about documents
CREATE VIEW IF NOT EXISTS document_stats AS
SELECT
    doc_type,
    COUNT(*) as count,
    SUM(word_count) as total_words,
    AVG(word_count) as avg_words,
    MAX(word_count) as max_words
FROM documents
GROUP BY doc_type;

-- indexed_documents: Documents with embeddings
CREATE VIEW IF NOT EXISTS indexed_documents AS
SELECT
    d.id,
    d.title,
    d.doc_type,
    COUNT(DISTINCT dc.id) as chunk_count,
    COUNT(DISTINCT de.id) as embedding_count,
    d.indexed_at,
    CASE
        WHEN d.indexed_at IS NULL THEN 'not_indexed'
        WHEN d.updated_at > d.indexed_at THEN 'needs_reindex'
        ELSE 'indexed'
    END as index_status
FROM documents d
LEFT JOIN document_chunks dc ON d.id = dc.document_id
LEFT JOIN document_embeddings de ON dc.id = de.chunk_id
GROUP BY d.id;
```

---

## 🔧 Implementation Steps

### Phase 1: Database & Core Models (2-3 hours)

**Tasks:**
1. ✅ Create migration `006_documents.sql`
2. ✅ Add Rust models in `src/db/models.rs`:
   - `Document`
   - `DocumentChunk`
   - `DocumentEmbedding`
   - `DocumentTag`
3. ✅ Add database functions in `src/db/documents.rs`:
   - `create_document()`
   - `get_document()`
   - `list_documents()`
   - `update_document()`
   - `delete_document()`
   - `search_documents_by_tag()`
   - `search_documents_by_title()`

**Files to Create:**
- `migrations/006_documents.sql`
- `src/db/documents.rs`

**Files to Modify:**
- `src/db/models.rs`
- `src/db/mod.rs` (add `pub mod documents;`)

---

### Phase 2: Chunking & Text Processing (2-3 hours)

**Tasks:**
1. ✅ Create text chunking module `src/chunking.rs`
2. ✅ Implement chunking strategies:
   - Fixed-size chunks (512 words)
   - Paragraph-based chunks
   - Markdown-aware chunks (preserve code blocks, headers)
3. ✅ Add chunk overlap (20% overlap for context continuity)
4. ✅ Extract metadata from chunks (headings, code language)

**Chunking Logic:**

```rust
pub struct ChunkConfig {
    pub max_words: usize,      // 512
    pub overlap_words: usize,  // 100
    pub respect_paragraphs: bool,
    pub respect_code_blocks: bool,
}

pub fn chunk_document(content: &str, config: ChunkConfig) -> Vec<Chunk> {
    // 1. Parse markdown
    // 2. Split on paragraphs/sections
    // 3. Combine into chunks of ~max_words
    // 4. Add overlap between chunks
    // 5. Return chunks with metadata
}
```

**Files to Create:**
- `src/chunking.rs`

**Files to Modify:**
- `src/lib.rs` (add `pub mod chunking;`)

---

### Phase 3: Embedding Generation (3-4 hours)

**Tasks:**
1. ✅ Create embedding module `src/embeddings.rs`
2. ✅ Integrate fastembed-rs (already in Cargo.toml?)
3. ✅ Implement embedding generation:
   - Load model on startup (cache in memory)
   - Generate embeddings for chunks
   - Batch processing for efficiency
4. ✅ Add database functions for embeddings:
   - `store_embedding()`
   - `get_embeddings_for_document()`
   - `delete_embeddings_for_document()`
5. ✅ Implement background indexing job

**Embedding Flow:**

```rust
pub struct EmbeddingService {
    model: Arc<TextEmbedding>,
}

impl EmbeddingService {
    pub fn new() -> Result<Self>;
    
    pub async fn embed_text(&self, text: &str) -> Result<Vec<f32>>;
    
    pub async fn embed_batch(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>>;
    
    pub async fn index_document(&self, doc_id: &str, db: &DbPool) -> Result<()> {
        // 1. Chunk document
        // 2. Generate embeddings for each chunk
        // 3. Store in database
        // 4. Update indexed_at timestamp
    }
}
```

**Files to Create:**
- `src/embeddings.rs`

**Files to Modify:**
- `Cargo.toml` (add fastembed-rs if not present)
- `src/lib.rs`

---

### Phase 4: Semantic Search (3-4 hours)

**Tasks:**
1. ✅ Implement cosine similarity function
2. ✅ Create search service `src/search.rs`
3. ✅ Implement semantic search:
   - Generate query embedding
   - Calculate similarity with all stored embeddings
   - Return top-k results with scores
4. ✅ Add filtering (by doc_type, tags, repo)
5. ✅ Implement hybrid search (combine semantic + keyword)

**Search Logic:**

```rust
pub struct SearchService {
    embedding_service: Arc<EmbeddingService>,
}

pub struct SearchQuery {
    pub query: String,
    pub limit: usize,
    pub threshold: f32,  // Minimum similarity score
    pub doc_types: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub repo_id: Option<String>,
}

pub struct SearchResult {
    pub chunk_id: String,
    pub document_id: String,
    pub document_title: String,
    pub chunk_content: String,
    pub similarity_score: f32,
    pub chunk_index: i32,
}

impl SearchService {
    pub async fn search(&self, query: SearchQuery, db: &DbPool) -> Result<Vec<SearchResult>> {
        // 1. Generate query embedding
        // 2. Load all embeddings from DB (with filters)
        // 3. Calculate cosine similarity
        // 4. Sort by score, return top-k
    }
    
    pub async fn find_relevant_context(
        &self,
        query: &str,
        max_tokens: usize,
        db: &DbPool
    ) -> Result<String> {
        // For LLM context stuffing
        // 1. Search for relevant chunks
        // 2. Combine chunks up to max_tokens
        // 3. Return formatted context
    }
}
```

**Files to Create:**
- `src/search.rs`

**Files to Modify:**
- `src/lib.rs`

---

### Phase 5: Web UI - Backend (2-3 hours)

**Tasks:**
1. ✅ Add document handlers in `src/web_ui.rs`:
   - `GET /documents` - List all documents
   - `GET /documents/new` - Upload form
   - `POST /documents` - Create document
   - `GET /documents/:id` - View document
   - `GET /documents/:id/edit` - Edit form
   - `POST /documents/:id` - Update document
   - `DELETE /documents/:id` - Delete document
2. ✅ Add search handlers:
   - `GET /search` - Search page
   - `POST /api/search` - Search API endpoint
3. ✅ Add background indexing trigger
4. ✅ Add import handlers:
   - `POST /documents/import/url` - Import from URL
   - `POST /documents/import/repo` - Import from repo file

**Request/Response Types:**

```rust
pub struct CreateDocumentRequest {
    pub title: String,
    pub content: String,
    pub doc_type: String,
    pub tags: Vec<String>,
    pub repo_id: Option<String>,
    pub auto_index: bool, // Generate embeddings immediately
}

pub struct SearchRequest {
    pub query: String,
    pub limit: Option<usize>,
    pub doc_types: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}

pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total: usize,
    pub query_time_ms: u64,
}
```

**Files to Modify:**
- `src/web_ui.rs`

---

### Phase 6: Web UI - Frontend (3-4 hours)

**Tasks:**
1. ✅ Create document templates in `src/templates/pages/`:
   - `documents.html` - List view
   - `document_new.html` - Upload/create form
   - `document_view.html` - View single document
   - `document_edit.html` - Edit form
2. ✅ Create search template:
   - `search.html` - Search interface with results
3. ✅ Add HTMX interactions:
   - Live search with debouncing
   - Infinite scroll for document list
   - Inline tag adding
4. ✅ Add syntax highlighting for code blocks
5. ✅ Add markdown preview

**UI Components:**

```html
<!-- Document List Item -->
<div class="document-card">
    <h3>{{ document.title }}</h3>
    <div class="meta">
        <span class="doc-type">{{ document.doc_type }}</span>
        <span class="word-count">{{ document.word_count }} words</span>
        <span class="tags">
            {% for tag in document.tags %}
            <span class="tag">#{{ tag }}</span>
            {% endfor %}
        </span>
    </div>
    <p class="excerpt">{{ document.content | truncate(200) }}</p>
    <div class="actions">
        <a href="/documents/{{ document.id }}">View</a>
        <a href="/documents/{{ document.id }}/edit">Edit</a>
    </div>
</div>

<!-- Search Box -->
<input type="text" 
       name="q" 
       placeholder="Search documents..."
       hx-post="/api/search"
       hx-trigger="keyup changed delay:500ms"
       hx-target="#search-results" />
       
<div id="search-results">
    <!-- Results loaded here via HTMX -->
</div>
```

**Files to Create:**
- `src/templates/pages/documents.html`
- `src/templates/pages/document_new.html`
- `src/templates/pages/document_view.html`
- `src/templates/pages/document_edit.html`
- `src/templates/pages/search.html`

**Files to Modify:**
- Navigation in other templates to add "Documents" and "Search" links

---

### Phase 7: LLM Context Integration (2-3 hours)

**Tasks:**
1. ✅ Add context retrieval to LLM service
2. ✅ Implement "smart context" for queue items:
   - When analyzing a task, search for relevant docs
   - Stuff context into LLM prompt
3. ✅ Add context preview in UI (show what context was used)
4. ✅ Add manual context selection (user picks docs to include)

**Integration Points:**

```rust
// In LLM service, before calling API:
pub async fn analyze_with_context(
    &self,
    task: &Task,
    search_service: &SearchService,
    db: &DbPool
) -> Result<String> {
    // 1. Extract keywords from task
    // 2. Search for relevant documents
    // 3. Build context string
    // 4. Prepend to prompt
    // 5. Call LLM
    // 6. Return response
}
```

**Files to Modify:**
- LLM service module
- Queue processing logic

---

## 📊 Success Metrics

- ✅ Documents can be created, viewed, edited, deleted via UI
- ✅ Documents are automatically chunked and indexed
- ✅ Semantic search returns relevant results
- ✅ Search response time < 500ms for 1000+ documents
- ✅ Embeddings generate in < 2s per document
- ✅ Context stuffing improves LLM response quality
- ✅ UI is responsive and intuitive

---

## 🧪 Testing Plan

### Unit Tests
- Chunking algorithm (various document sizes, formats)
- Embedding generation (model loading, batch processing)
- Cosine similarity calculation
- Search ranking accuracy

### Integration Tests
- Full document lifecycle (create → chunk → embed → search)
- Context retrieval for LLM queries
- Tag filtering in search

### Manual Tests
1. Upload a markdown document with code blocks
2. Verify chunks preserve code blocks
3. Search for a concept, verify relevant results
4. Test with various document types (tutorial, reference, snippet)
5. Verify LLM uses retrieved context

---

## 🚧 Implementation Order

**Day 1 (4-5 hours):**
- Phase 1: Database schema and models
- Phase 2: Chunking module

**Day 2 (4-5 hours):**
- Phase 3: Embedding generation
- Start Phase 4: Semantic search

**Day 3 (4-5 hours):**
- Complete Phase 4: Semantic search
- Phase 5: Backend handlers

**Day 4 (3-4 hours):**
- Phase 6: Frontend templates
- Phase 7: LLM integration

---

## 📦 Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
# Embeddings
fastembed = "3.0"  # Or latest version

# Text processing
unicode-segmentation = "1.10"
pulldown-cmark = "0.9"  # Markdown parsing

# Vector operations
ndarray = "0.15"  # For similarity calculations
```

---

## 🔄 Future Enhancements

**Phase 2 (Post-MVP):**
- SQLite vec extension for native vector search
- Document versioning (track changes)
- Collaborative editing
- Document templates
- Auto-categorization using LLM
- Citation tracking (which docs were used for which LLM responses)

**Phase 3 (Advanced):**
- LanceDB integration for better vector search
- Multi-modal embeddings (images, diagrams)
- Automatic document extraction from repos
- Knowledge graph visualization
- Question answering over documents

---

## 📝 Notes

- Start simple: Use JSON arrays for embeddings initially
- Consider sqlite-vec extension later for performance
- Keep chunks small enough to fit in context window
- Use mxbai-embed-large-v1 (1024 dimensions) for consistency
- Implement pagination for document lists (100 items per page)
- Cache embeddings model in memory (don't reload per request)

---

## ✅ Checklist

### Phase 1: Database & Models
- [ ] Create migration 006_documents.sql
- [ ] Add Document model
- [ ] Add DocumentChunk model
- [ ] Add DocumentEmbedding model
- [ ] Add database functions (CRUD)
- [ ] Test migrations

### Phase 2: Chunking
- [ ] Create chunking module
- [ ] Implement fixed-size chunking
- [ ] Implement markdown-aware chunking
- [ ] Add chunk overlap
- [ ] Unit tests for chunking

### Phase 3: Embeddings
- [ ] Create embeddings module
- [ ] Integrate fastembed-rs
- [ ] Implement batch embedding
- [ ] Add background indexing
- [ ] Store embeddings in DB

### Phase 4: Search
- [ ] Implement cosine similarity
- [ ] Create search service
- [ ] Add semantic search
- [ ] Add filtering (tags, type, repo)
- [ ] Optimize search performance

### Phase 5: Backend
- [ ] Document CRUD handlers
- [ ] Search API endpoint
- [ ] Import from URL handler
- [ ] Import from repo handler
- [ ] Background job triggers

### Phase 6: Frontend
- [ ] Documents list page
- [ ] Create/upload form
- [ ] View document page
- [ ] Edit document page
- [ ] Search page with live results
- [ ] Navigation updates

### Phase 7: LLM Integration
- [ ] Context retrieval function
- [ ] Integrate with queue processing
- [ ] Context preview in UI
- [ ] Manual context selection

---

**Status:** Ready to begin implementation  
**Next Step:** Create migration 006_documents.sql  
**Estimated Completion:** 4-5 days of focused work