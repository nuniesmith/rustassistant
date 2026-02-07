# RAG API Documentation

Complete REST API for semantic document search and management using Rust, Axum, and FastEmbed.

## 🚀 Quick Start

### Running the Server

```bash
# Development mode (no authentication)
cargo run --example rag_server

# Production mode with authentication
API_KEY="your-secret-key" REQUIRE_AUTH=true cargo run --example rag_server

# Custom port
PORT=8080 cargo run --example rag_server
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | SQLite database path | `sqlite:rustassistant.db` |
| `HOST` | Server bind address | `0.0.0.0` |
| `PORT` | Server port | `3000` |
| `API_KEY` | API key for authentication | None |
| `REQUIRE_AUTH` | Enable authentication | `false` |
| `RUST_LOG` | Log level | `info` |

---

## 📚 API Endpoints

### Health & Status

#### `GET /health`
Basic health check

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "services": {
    "database": true,
    "embeddings": true,
    "search": true
  }
}
```

#### `GET /api/stats`
System statistics

**Response:**
```json
{
  "documents": {
    "total": 150,
    "indexed": 145,
    "pending": 5,
    "by_type": [
      {"doc_type": "markdown", "count": 80},
      {"doc_type": "code", "count": 70}
    ]
  },
  "chunks": {
    "total": 1500,
    "avg_per_document": 10.0,
    "avg_size": 512.5
  },
  "search": {
    "total_searches": 250,
    "avg_results": 8.5,
    "avg_execution_time_ms": 45.2
  },
  "indexing": {
    "jobs_queued": 2,
    "jobs_processing": 1,
    "jobs_completed": 47,
    "jobs_failed": 0
  }
}
```

---

### Document Management

#### `POST /api/documents`
Upload a new document

**Request:**
```json
{
  "title": "Authentication Guide",
  "content": "# Authentication\n\nThis guide covers...",
  "doc_type": "markdown",
  "tags": ["auth", "security", "tutorial"],
  "metadata": {
    "author": "John Doe",
    "version": "1.0"
  },
  "repo_id": 1,
  "source_type": "github",
  "source_url": "https://github.com/user/repo"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": 123,
    "title": "Authentication Guide",
    "status": "queued_for_indexing",
    "created_at": "2024-01-15T10:30:00Z",
    "message": "Document uploaded successfully. Indexing job: abc-123"
  }
}
```

#### `GET /api/documents`
List documents with pagination

**Query Parameters:**
- `page` (default: 1)
- `limit` (default: 20, max: 100)
- `doc_type` (optional)
- `repo_id` (optional)
- `indexed_only` (optional)
- `tag` (optional)

**Response:**
```json
{
  "success": true,
  "data": {
    "items": [
      {
        "id": 123,
        "title": "Authentication Guide",
        "doc_type": "markdown",
        "is_indexed": true,
        "created_at": "2024-01-15T10:30:00Z"
      }
    ],
    "total": 150,
    "page": 1,
    "limit": 20,
    "total_pages": 8
  }
}
```

#### `GET /api/documents/:id`
Get document details

**Response:**
```json
{
  "success": true,
  "data": {
    "id": 123,
    "title": "Authentication Guide",
    "content": "# Authentication\n\nThis guide covers...",
    "doc_type": "markdown",
    "tags": ["auth", "security"],
    "metadata": {"author": "John Doe"},
    "repo_id": 1,
    "source_type": "github",
    "source_url": "https://github.com/user/repo",
    "is_indexed": true,
    "indexed_at": "2024-01-15T10:35:00Z",
    "created_at": "2024-01-15T10:30:00Z",
    "updated_at": "2024-01-15T10:30:00Z",
    "chunk_count": 12
  }
}
```

#### `PUT /api/documents/:id`
Update document metadata

**Request:**
```json
{
  "title": "Updated Title",
  "tags": ["new", "tags"],
  "metadata": {"version": "2.0"}
}
```

**Response:**
```json
{
  "success": true,
  "message": "Document updated successfully"
}
```

#### `DELETE /api/documents/:id`
Delete document and all associated chunks/embeddings

**Response:**
```json
{
  "success": true,
  "message": "Document deleted successfully"
}
```

---

### Search

#### `POST /api/search`
Search documents using semantic, keyword, or hybrid search

**Request:**
```json
{
  "query": "How to implement JWT authentication in Rust",
  "limit": 10,
  "search_type": "hybrid",
  "filters": {
    "doc_type": "documentation",
    "tags": ["rust", "auth"],
    "repo_id": 1,
    "source_type": "github",
    "indexed_only": true,
    "date_from": "2024-01-01T00:00:00Z",
    "date_to": "2024-12-31T23:59:59Z"
  }
}
```

**Search Types:**
- `semantic` - Vector similarity search only
- `keyword` - Traditional keyword matching
- `hybrid` - Combines both using Reciprocal Rank Fusion (default)

**Response:**
```json
{
  "success": true,
  "data": {
    "results": [
      {
        "document_id": 123,
        "chunk_id": 456,
        "title": "Authentication Guide",
        "content": "JWT authentication in Rust can be implemented using...",
        "doc_type": "markdown",
        "score": 0.89,
        "tags": ["rust", "auth", "jwt"],
        "metadata": {
          "repo_id": 1,
          "source_type": "github"
        },
        "source_url": "https://github.com/user/repo",
        "created_at": "2024-01-15T10:30:00Z"
      }
    ],
    "total_results": 8,
    "search_type": "hybrid",
    "query": "How to implement JWT authentication in Rust",
    "execution_time_ms": 42
  }
}
```

---

### Indexing

#### `POST /api/index`
Index a single document

**Request:**
```json
{
  "document_id": 123,
  "force_reindex": false
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "job_id": "abc-123",
    "document_ids": [123],
    "status": "queued",
    "queued_at": "2024-01-15T10:30:00Z"
  }
}
```

#### `POST /api/index/batch`
Batch index multiple documents

**Request:**
```json
{
  "document_ids": [1, 2, 3, 4, 5],
  "force_reindex": false
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "job_id": "def-456",
    "document_ids": [1, 2, 3, 4, 5],
    "status": "queued",
    "queued_at": "2024-01-15T10:30:00Z"
  }
}
```

#### `GET /api/index/jobs`
List all indexing jobs

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "job_id": "abc-123",
      "status": "completed",
      "documents_total": 5,
      "documents_completed": 5,
      "created_at": "2024-01-15T10:30:00Z",
      "completed_at": "2024-01-15T10:32:00Z"
    }
  ]
}
```

#### `GET /api/index/jobs/:job_id`
Get indexing job status

**Response:**
```json
{
  "success": true,
  "data": {
    "job_id": "abc-123",
    "status": "processing",
    "documents_total": 5,
    "documents_completed": 3,
    "documents_failed": 0,
    "started_at": "2024-01-15T10:30:00Z",
    "completed_at": null,
    "error": null
  }
}
```

**Job Statuses:**
- `queued` - Waiting to be processed
- `processing` - Currently being processed
- `completed` - Successfully completed
- `failed` - Failed with errors

#### `POST /api/index/jobs/:job_id/cancel`
Cancel a queued indexing job

**Response:**
```json
{
  "success": true,
  "message": "Job cancelled successfully"
}
```

---

## 🔐 Authentication

### API Key Authentication

Include your API key in the `X-API-Key` header or `Authorization: Bearer <key>` header.

```bash
curl -X POST http://localhost:3000/api/search \
  -H "X-API-Key: your-secret-key" \
  -H "Content-Type: application/json" \
  -d '{"query": "rust tutorials", "limit": 10}'
```

### Anonymous Read Access

When `allow_anonymous_read` is enabled, GET requests don't require authentication:

```bash
# No API key needed for reads
curl http://localhost:3000/api/documents
curl http://localhost:3000/api/stats
```

### Generating API Keys

```rust
use rustassistant::generate_api_key;

let api_key = generate_api_key();
println!("New API key: {}", api_key);
```

---

## ⚡ Rate Limiting

The API includes built-in rate limiting using a token bucket algorithm.

### Default Limits
- **100 requests per minute** per client
- Identified by API key or IP address

### Rate Limit Headers

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Window: 60
```

### Rate Limit Exceeded

**Status:** `429 Too Many Requests`

```json
{
  "success": false,
  "error": "Rate limit exceeded. Retry after 15 seconds"
}
```

**Headers:**
```
Retry-After: 15
```

### Configuration

```rust
use rustassistant::api::{ApiConfig, RateLimitConfig};

let config = ApiConfig::development()
    .with_rate_limit(1000, 60); // 1000 requests per 60 seconds
```

---

## 📊 Usage Examples

### Complete Workflow

```bash
# 1. Upload a document
curl -X POST http://localhost:3000/api/documents \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Rust Error Handling",
    "content": "# Error Handling in Rust\n\nRust uses Result<T, E> for error handling...",
    "doc_type": "markdown",
    "tags": ["rust", "errors", "tutorial"]
  }'

# Response: {"success":true,"data":{"id":1,...,"message":"Indexing job: abc-123"}}

# 2. Check indexing status
curl http://localhost:3000/api/index/jobs/abc-123

# Response: {"success":true,"data":{"status":"completed",...}}

# 3. Search the document
curl -X POST http://localhost:3000/api/search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "how to handle errors in rust",
    "limit": 5,
    "search_type": "hybrid"
  }'

# Response: {"success":true,"data":{"results":[...],"total_results":1}}

# 4. Get document details
curl http://localhost:3000/api/documents/1

# 5. Re-index if needed
curl -X POST http://localhost:3000/api/index \
  -H "Content-Type: application/json" \
  -d '{"document_id": 1, "force_reindex": true}'

# 6. Delete document
curl -X DELETE http://localhost:3000/api/documents/1
```

### Batch Upload Script

```bash
#!/bin/bash

# Upload multiple documents
for file in docs/*.md; do
  title=$(basename "$file" .md)
  content=$(cat "$file")
  
  curl -X POST http://localhost:3000/api/documents \
    -H "Content-Type: application/json" \
    -d "{
      \"title\": \"$title\",
      \"content\": $(echo "$content" | jq -Rs .),
      \"doc_type\": \"markdown\",
      \"tags\": [\"docs\"]
    }"
  
  echo "Uploaded: $title"
  sleep 0.5
done
```

### Python Client Example

```python
import requests

class RAGClient:
    def __init__(self, base_url="http://localhost:3000", api_key=None):
        self.base_url = base_url
        self.headers = {"Content-Type": "application/json"}
        if api_key:
            self.headers["X-API-Key"] = api_key
    
    def upload_document(self, title, content, doc_type="markdown", tags=None):
        data = {
            "title": title,
            "content": content,
            "doc_type": doc_type,
            "tags": tags or []
        }
        response = requests.post(
            f"{self.base_url}/api/documents",
            json=data,
            headers=self.headers
        )
        return response.json()
    
    def search(self, query, limit=10, search_type="hybrid", filters=None):
        data = {
            "query": query,
            "limit": limit,
            "search_type": search_type,
            "filters": filters or {}
        }
        response = requests.post(
            f"{self.base_url}/api/search",
            json=data,
            headers=self.headers
        )
        return response.json()
    
    def get_stats(self):
        response = requests.get(f"{self.base_url}/api/stats")
        return response.json()

# Usage
client = RAGClient()

# Upload
result = client.upload_document(
    title="My Document",
    content="Content here...",
    tags=["python", "api"]
)
print(f"Uploaded: {result['data']['id']}")

# Search
results = client.search("python api examples", limit=5)
for hit in results['data']['results']:
    print(f"Score: {hit['score']:.2f} - {hit['title']}")
```

---

## 🏗️ Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────┐
│                      API Server                         │
├─────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Rate       │  │     Auth     │  │    CORS      │  │
│  │   Limiter    │→ │  Middleware  │→ │   Layer      │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
├─────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────┐  │
│  │              API Handlers                        │  │
│  │  • Documents  • Search  • Indexing  • Stats      │  │
│  └──────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────┤
│  ┌───────────┐  ┌──────────┐  ┌──────────────────┐   │
│  │  Semantic │  │ Embedding│  │  Background      │   │
│  │  Searcher │  │ Generator│  │  Job Queue       │   │
│  └───────────┘  └──────────┘  └──────────────────┘   │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────┐   │
│  │            SQLite Database                      │   │
│  │  • Documents  • Chunks  • Embeddings  • Jobs    │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### Data Flow

1. **Document Upload** → Chunking → Embedding → Storage
2. **Search Query** → Embedding → Vector Search → Ranking → Results
3. **Background Jobs** → Queue → Process → Update Status

---

## 🔧 Configuration

### Programmatic Configuration

```rust
use rustassistant::api::{ApiConfig, JobQueueConfig, RateLimitConfig};
use rustassistant::indexing::IndexingConfig;

let config = ApiConfig {
    auth: AuthConfig::new(vec!["key1".to_string(), "key2".to_string()]),
    
    rate_limit: RateLimitConfig {
        max_requests: 100,
        window_seconds: 60,
        enabled: true,
    },
    
    indexing: IndexingConfig {
        chunk_size: 512,
        chunk_overlap: 50,
        batch_size: 32,
    },
    
    job_queue: JobQueueConfig {
        max_concurrent_jobs: 2,
        retry_enabled: true,
        max_retries: 3,
    },
};

let router = config.build_router(db_pool);
```

### Environment-Based Configuration

```bash
# .env file
DATABASE_URL=sqlite:production.db
HOST=0.0.0.0
PORT=8080
API_KEY=super-secret-key
REQUIRE_AUTH=true
RUST_LOG=info,rustassistant=debug
```

---

## 🚢 Deployment

### Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --example rag_server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates
COPY --from=builder /app/target/release/examples/rag_server /usr/local/bin/
EXPOSE 3000
CMD ["rag_server"]
```

```bash
docker build -t rag-api .
docker run -p 3000:3000 -e API_KEY=secret rag-api
```

### Docker Compose

```yaml
version: '3.8'

services:
  rag-api:
    build: .
    ports:
      - "3000:3000"
    environment:
      - DATABASE_URL=sqlite:/data/rustassistant.db
      - API_KEY=${API_KEY}
      - REQUIRE_AUTH=true
    volumes:
      - ./data:/data
    restart: unless-stopped
```

### Systemd Service

```ini
# /etc/systemd/system/rag-api.service
[Unit]
Description=RAG API Server
After=network.target

[Service]
Type=simple
User=rag
WorkingDirectory=/opt/rag-api
Environment="DATABASE_URL=sqlite:/var/lib/rag-api/db.sqlite"
Environment="PORT=3000"
ExecStart=/opt/rag-api/rag_server
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

### Nginx Reverse Proxy

```nginx
upstream rag_api {
    server 127.0.0.1:3000;
}

server {
    listen 80;
    server_name api.example.com;

    location / {
        proxy_pass http://rag_api;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

---

## 📈 Performance

### Benchmarks (Approximate)

- **Document Upload**: ~50ms
- **Chunk & Embed (512 tokens)**: ~100ms
- **Semantic Search (10k docs)**: ~50-100ms
- **Hybrid Search (10k docs)**: ~80-150ms
- **Throughput**: ~500 req/s (single instance)

### Optimization Tips

1. **Batch Operations**: Use batch indexing for multiple documents
2. **Connection Pooling**: Configure SQLx pool size
3. **Caching**: Enable response caching for frequent queries
4. **Load Balancing**: Run multiple instances behind a load balancer
5. **Database**: Use SQLite WAL mode or migrate to PostgreSQL

---

## 🐛 Troubleshooting

### Common Issues

**Issue: "Failed to load embedding model"**
```bash
# Download model manually
mkdir -p ~/.cache/fastembed
# Model downloads automatically on first use
```

**Issue: "Database locked"**
```bash
# Enable WAL mode
sqlite3 rustassistant.db "PRAGMA journal_mode=WAL;"
```

**Issue: "Rate limit too strict"**
```rust
// Adjust in code
RateLimitConfig::new(1000, 60) // 1000 req/min
```

---

## 📝 License

MIT License - See LICENSE file for details

---

## 🤝 Contributing

Contributions welcome! Please see CONTRIBUTING.md

---

## 📞 Support

- Documentation: `/docs`
- Issues: GitHub Issues
- Discussions: GitHub Discussions