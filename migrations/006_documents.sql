-- Migration: 006_documents.sql
-- Description: Add documents, chunks, and embeddings for RAG system
-- Created: 2024-02-06
--
-- This migration adds support for document storage, chunking, and vector embeddings
-- for semantic search and RAG (Retrieval-Augmented Generation) functionality.

-- ============================================================================
-- Documents Table
-- ============================================================================

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

-- ============================================================================
-- Document Chunks Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS document_chunks (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    char_start INTEGER NOT NULL,
    char_end INTEGER NOT NULL,
    word_count INTEGER DEFAULT 0,
    heading TEXT, -- Markdown heading context if available
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
    UNIQUE(document_id, chunk_index)
);

-- ============================================================================
-- Document Embeddings Table
-- ============================================================================
-- Note: Using TEXT to store embeddings as JSON array for simplicity
-- Consider sqlite-vec extension or LanceDB for production scale

CREATE TABLE IF NOT EXISTS document_embeddings (
    id TEXT PRIMARY KEY,
    chunk_id TEXT NOT NULL UNIQUE,
    embedding TEXT NOT NULL, -- JSON array of floats
    model TEXT NOT NULL DEFAULT 'mxbai-embed-large-v1',
    dimension INTEGER NOT NULL DEFAULT 1024,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (chunk_id) REFERENCES document_chunks(id) ON DELETE CASCADE
);

-- ============================================================================
-- Document Tags Junction Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS document_tags (
    document_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (document_id, tag),
    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
    FOREIGN KEY (tag) REFERENCES tags(name) ON DELETE CASCADE
);

-- ============================================================================
-- Indexes
-- ============================================================================

-- Documents indexes
CREATE INDEX IF NOT EXISTS idx_documents_repo_id ON documents(repo_id)
    WHERE repo_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_documents_doc_type ON documents(doc_type);

CREATE INDEX IF NOT EXISTS idx_documents_created ON documents(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_documents_updated ON documents(updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_documents_indexed ON documents(indexed_at)
    WHERE indexed_at IS NOT NULL;

-- Document chunks indexes
CREATE INDEX IF NOT EXISTS idx_document_chunks_doc_id ON document_chunks(document_id);

CREATE INDEX IF NOT EXISTS idx_document_chunks_chunk_index ON document_chunks(document_id, chunk_index);

-- Document embeddings indexes
CREATE INDEX IF NOT EXISTS idx_document_embeddings_chunk_id ON document_embeddings(chunk_id);

CREATE INDEX IF NOT EXISTS idx_document_embeddings_model ON document_embeddings(model);

-- Document tags indexes
CREATE INDEX IF NOT EXISTS idx_document_tags_tag ON document_tags(tag);

CREATE INDEX IF NOT EXISTS idx_document_tags_doc ON document_tags(document_id);

-- ============================================================================
-- Views
-- ============================================================================

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
    d.char_count,
    d.created_at,
    d.updated_at,
    d.indexed_at,
    GROUP_CONCAT(dt.tag, ',') as tag_list,
    COUNT(DISTINCT dc.id) as chunk_count,
    COUNT(DISTINCT de.id) as embedding_count
FROM documents d
LEFT JOIN document_tags dt ON d.id = dt.document_id
LEFT JOIN document_chunks dc ON d.id = dc.document_id
LEFT JOIN document_embeddings de ON dc.id = de.chunk_id
GROUP BY d.id;

-- document_stats: Statistics about documents
CREATE VIEW IF NOT EXISTS document_stats AS
SELECT
    doc_type,
    COUNT(*) as count,
    SUM(word_count) as total_words,
    AVG(word_count) as avg_words,
    MAX(word_count) as max_words,
    MIN(word_count) as min_words
FROM documents
GROUP BY doc_type;

-- indexed_documents: Documents with their indexing status
CREATE VIEW IF NOT EXISTS indexed_documents AS
SELECT
    d.id,
    d.title,
    d.doc_type,
    d.word_count,
    COUNT(DISTINCT dc.id) as chunk_count,
    COUNT(DISTINCT de.id) as embedding_count,
    d.updated_at,
    d.indexed_at,
    CASE
        WHEN d.indexed_at IS NULL THEN 'not_indexed'
        WHEN d.updated_at > d.indexed_at THEN 'needs_reindex'
        ELSE 'indexed'
    END as index_status,
    datetime(d.updated_at, 'unixepoch') as updated_time,
    datetime(d.indexed_at, 'unixepoch') as indexed_time
FROM documents d
LEFT JOIN document_chunks dc ON d.id = dc.document_id
LEFT JOIN document_embeddings de ON dc.id = de.chunk_id
GROUP BY d.id;

-- unindexed_documents: Documents needing indexing
CREATE VIEW IF NOT EXISTS unindexed_documents AS
SELECT
    id,
    title,
    doc_type,
    word_count,
    updated_at,
    indexed_at
FROM documents
WHERE indexed_at IS NULL OR updated_at > indexed_at
ORDER BY updated_at DESC;

-- recent_documents: Recently created or updated documents
CREATE VIEW IF NOT EXISTS recent_documents AS
SELECT
    d.id,
    d.title,
    d.doc_type,
    d.word_count,
    d.created_at,
    d.updated_at,
    GROUP_CONCAT(dt.tag, ',') as tags,
    CASE
        WHEN d.created_at = d.updated_at THEN 'created'
        ELSE 'updated'
    END as activity_type,
    datetime(d.updated_at, 'unixepoch') as activity_time
FROM documents d
LEFT JOIN document_tags dt ON d.id = dt.document_id
GROUP BY d.id
ORDER BY d.updated_at DESC
LIMIT 50;

-- document_repo_summary: Documents grouped by repository
CREATE VIEW IF NOT EXISTS document_repo_summary AS
SELECT
    r.id as repo_id,
    r.name as repo_name,
    COUNT(d.id) as document_count,
    SUM(d.word_count) as total_words,
    MAX(d.updated_at) as last_updated
FROM repositories r
LEFT JOIN documents d ON r.id = d.repo_id
GROUP BY r.id;

-- ============================================================================
-- Triggers
-- ============================================================================

-- Update document updated_at timestamp
CREATE TRIGGER IF NOT EXISTS update_document_timestamp
AFTER UPDATE ON documents
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE documents
    SET updated_at = strftime('%s', 'now')
    WHERE id = NEW.id;
END;

-- Update tag usage count when document_tags changes
CREATE TRIGGER IF NOT EXISTS update_tag_count_on_doc_tag_insert
AFTER INSERT ON document_tags
FOR EACH ROW
BEGIN
    UPDATE tags
    SET usage_count = usage_count + 1
    WHERE name = NEW.tag;
END;

CREATE TRIGGER IF NOT EXISTS update_tag_count_on_doc_tag_delete
AFTER DELETE ON document_tags
FOR EACH ROW
BEGIN
    UPDATE tags
    SET usage_count = MAX(0, usage_count - 1)
    WHERE name = OLD.tag;
END;

-- ============================================================================
-- Initial Data / Sample Documents
-- ============================================================================

-- Optional: Add a welcome document
INSERT OR IGNORE INTO documents (
    id,
    title,
    content,
    content_type,
    source_type,
    doc_type,
    word_count,
    char_count
) VALUES (
    'welcome-doc',
    'Welcome to RustAssistant RAG System',
    '# Welcome to RustAssistant RAG

This is your knowledge base system powered by semantic search and vector embeddings.

## Features

- **Document Storage**: Store markdown documents, code snippets, research notes
- **Semantic Search**: Find relevant content using natural language queries
- **Context Retrieval**: Automatically retrieve relevant context for LLM queries
- **Tag Organization**: Organize documents with tags for easy filtering

## Getting Started

1. Upload documents via the web UI
2. Documents are automatically chunked and indexed
3. Use the search page to find relevant content
4. Context is automatically stuffed into LLM prompts

## Document Types

- **Reference**: API docs, language references, technical specs
- **Research**: Research papers, articles, blog posts
- **Tutorial**: How-to guides, walkthroughs, examples
- **Architecture**: Design docs, system architecture, diagrams
- **Note**: Personal notes, ideas, observations
- **Snippet**: Code snippets, templates, examples

Happy searching! 🦀',
    'markdown',
    'manual',
    'tutorial',
    120,
    800
);

-- ============================================================================
-- Migration Complete
-- ============================================================================

-- Summary:
-- - Created documents table for storing document metadata and content
-- - Created document_chunks table for storing chunked text
-- - Created document_embeddings table for vector embeddings
-- - Created document_tags junction table for many-to-many tags
-- - Added indexes for performance
-- - Created views for common queries
-- - Added triggers for timestamp updates and tag counts
-- - Added welcome document as example

-- Next steps:
-- 1. Implement chunking logic in Rust
-- 2. Integrate fastembed-rs for embedding generation
-- 3. Implement semantic search with cosine similarity
-- 4. Build web UI for document management
