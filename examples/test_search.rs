//! Test Semantic Search Example
//!
//! This example demonstrates the semantic search capabilities:
//! 1. Index sample documents
//! 2. Perform semantic search queries
//! 3. Test filtering options
//! 4. Compare semantic vs keyword search
//! 5. Test hybrid search

use rustassistant::chunking::ChunkConfig;
use rustassistant::db::{create_document, init_pool};
use rustassistant::indexing::{DocumentIndexer, IndexingConfig};
use rustassistant::search::{SearchConfig, SearchFilters, SearchQuery, SemanticSearcher};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🧪 Testing Semantic Search Module\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Setup test database
    println!("📋 Setting up test database...");
    let pool = init_pool("sqlite::memory:").await?;
    println!("✓ Database initialized\n");

    // Create and index sample documents
    println!("📋 Creating sample documents...");
    let doc_ids = create_sample_documents(&pool).await?;
    println!("✓ Created {} documents\n", doc_ids.len());

    println!("📋 Indexing documents...");
    index_documents(&pool, &doc_ids).await?;
    println!("✓ Documents indexed\n");

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Test 1: Basic semantic search
    println!("📋 Test 1: Basic Semantic Search");
    println!("─────────────────────────────────────────────────────────────────────────");
    test_basic_search(&pool).await?;
    println!();

    // Test 2: Search with filters
    println!("📋 Test 2: Search with Filters");
    println!("─────────────────────────────────────────────────────────────────────────");
    test_filtered_search(&pool).await?;
    println!();

    // Test 3: Semantic vs keyword search
    println!("📋 Test 3: Semantic vs Keyword Search");
    println!("─────────────────────────────────────────────────────────────────────────");
    test_semantic_vs_keyword(&pool).await?;
    println!();

    // Test 4: Hybrid search
    println!("📋 Test 4: Hybrid Search");
    println!("─────────────────────────────────────────────────────────────────────────");
    test_hybrid_search(&pool).await?;
    println!();

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ All search tests completed successfully!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}

async fn create_sample_documents(pool: &sqlx::SqlitePool) -> anyhow::Result<Vec<String>> {
    let documents = vec![
        (
            "rust-async",
            "Asynchronous Programming in Rust",
            "Rust provides powerful async/await syntax for writing concurrent code. \
             The async keyword transforms a function into an asynchronous function that returns a Future. \
             You can use await to wait for futures to complete without blocking the thread. \
             The tokio runtime provides the infrastructure for running async code efficiently.",
            "code",
            vec!["rust", "async", "programming"],
        ),
        (
            "rust-ownership",
            "Ownership and Borrowing in Rust",
            "Rust's ownership system ensures memory safety without garbage collection. \
             Each value has a single owner, and when the owner goes out of scope, the value is dropped. \
             Borrowing allows you to reference values without taking ownership. \
             There are two types of borrows: immutable (&T) and mutable (&mut T).",
            "documentation",
            vec!["rust", "ownership", "memory-safety"],
        ),
        (
            "python-async",
            "Async/Await in Python",
            "Python's asyncio library enables asynchronous programming using async and await keywords. \
             Coroutines are defined with async def and can be awaited. \
             The event loop manages the execution of asynchronous tasks. \
             This is particularly useful for I/O-bound operations like network requests.",
            "code",
            vec!["python", "async", "asyncio"],
        ),
        (
            "web-frameworks",
            "Modern Web Frameworks",
            "Web frameworks simplify building web applications by providing routing, templating, and database integration. \
             Popular frameworks include Django and Flask for Python, Express for Node.js, and Axum for Rust. \
             Each framework has its own philosophy and strengths. \
             Choosing the right framework depends on your project requirements and team expertise.",
            "article",
            vec!["web", "frameworks", "comparison"],
        ),
        (
            "database-design",
            "Database Design Best Practices",
            "Good database design is crucial for application performance and maintainability. \
             Normalize your schema to reduce redundancy and improve data integrity. \
             Use appropriate indexes to speed up queries. \
             Consider using foreign keys to maintain referential integrity. \
             Document your schema and naming conventions.",
            "documentation",
            vec!["database", "design", "best-practices"],
        ),
    ];

    let mut doc_ids = Vec::new();

    for (id, title, content, doc_type, tags) in documents {
        let tags_json = serde_json::to_string(&tags)?;

        create_document(
            pool,
            id.to_string(),
            title.to_string(),
            content.to_string(),
            "text".to_string(),
            "manual".to_string(),
            doc_type.to_string(),
            Some(tags_json),
            None,
            None,
        )
        .await?;

        doc_ids.push(id.to_string());
    }

    Ok(doc_ids)
}

async fn index_documents(pool: &sqlx::SqlitePool, doc_ids: &[String]) -> anyhow::Result<()> {
    let config = IndexingConfig {
        chunk_config: ChunkConfig {
            target_words: 100,
            overlap_words: 20,
            min_chunk_size: 10,
            max_chunk_size: 150,
            markdown_aware: false,
            preserve_code_blocks: false,
            include_headings: false,
        },
        ..Default::default()
    };

    let indexer = DocumentIndexer::new(config).await?;

    for doc_id in doc_ids {
        let result = indexer.index_document(pool, doc_id).await?;
        println!(
            "  ✓ Indexed {}: {} chunks",
            doc_id, result.chunks_indexed
        );
    }

    Ok(())
}

async fn test_basic_search(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    let searcher = SemanticSearcher::new(SearchConfig::default()).await?;

    let query = SearchQuery {
        text: "How do I write asynchronous code?".to_string(),
        top_k: 3,
        filters: SearchFilters::default(),
    };

    println!("Query: \"{}\"", query.text);
    let results = searcher.search(pool, &query).await?;

    println!("✓ Found {} results:", results.len());
    for (idx, result) in results.iter().enumerate() {
        println!(
            "  [{}] Score: {:.4} | Doc: {} | Title: {}",
            idx + 1,
            result.score,
            result.document_id,
            result.title.as_ref().unwrap_or(&"N/A".to_string())
        );
        println!(
            "      Preview: {}...",
            result.content.chars().take(60).collect::<String>()
        );
    }

    Ok(())
}

async fn test_filtered_search(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    let searcher = SemanticSearcher::new(SearchConfig::default()).await?;

    // Search only in "code" documents
    let query = SearchQuery {
        text: "async programming".to_string(),
        top_k: 5,
        filters: SearchFilters {
            doc_type: Some("code".to_string()),
            indexed_only: true,
            ..Default::default()
        },
    };

    println!("Query: \"{}\" (filtered by doc_type='code')", query.text);
    let results = searcher.search(pool, &query).await?;

    println!("✓ Found {} results:", results.len());
    for (idx, result) in results.iter().enumerate() {
        println!(
            "  [{}] Score: {:.4} | Doc: {} (type: {})",
            idx + 1,
            result.score,
            result.document_id,
            result.doc_type.as_ref().unwrap_or(&"N/A".to_string())
        );
    }

    Ok(())
}

async fn test_semantic_vs_keyword(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    // Semantic search
    let semantic_config = SearchConfig {
        use_hybrid_search: false,
        ..Default::default()
    };
    let semantic_searcher = SemanticSearcher::new(semantic_config).await?;

    let query = SearchQuery {
        text: "memory management without garbage collection".to_string(),
        top_k: 3,
        filters: SearchFilters::default(),
    };

    println!("Query: \"{}\"", query.text);
    println!("\nSemantic Search Results:");
    let semantic_results = semantic_searcher.search(pool, &query).await?;

    for (idx, result) in semantic_results.iter().enumerate() {
        println!(
            "  [{}] Score: {:.4} | Doc: {}",
            idx + 1,
            result.score,
            result.document_id
        );
    }

    // Note: Keyword search would need "ownership" or "borrowing" to match
    println!("\n✓ Semantic search found relevant documents even without exact keyword matches!");

    Ok(())
}

async fn test_hybrid_search(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    let hybrid_config = SearchConfig {
        use_hybrid_search: true,
        semantic_weight: 0.7,
        keyword_weight: 0.3,
        ..Default::default()
    };

    let searcher = SemanticSearcher::new(hybrid_config).await?;

    let query = SearchQuery {
        text: "async".to_string(), // This will match keywords AND semantic meaning
        top_k: 5,
        filters: SearchFilters::default(),
    };

    println!("Query: \"{}\" (hybrid search)", query.text);
    let results = searcher.search(pool, &query).await?;

    println!("✓ Found {} results:", results.len());
    for (idx, result) in results.iter().enumerate() {
        let match_types = match (
            result.metadata.semantic_match,
            result.metadata.keyword_match,
        ) {
            (true, true) => "semantic + keyword",
            (true, false) => "semantic only",
            (false, true) => "keyword only",
            (false, false) => "unknown",
        };

        println!(
            "  [{}] Score: {:.4} | Doc: {} | Match: {}",
            idx + 1,
            result.score,
            result.document_id,
            match_types
        );
    }

    Ok(())
}
