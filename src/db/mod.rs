//! Database module
//!
//! Provides database operations for notes, repositories, tasks, and queue system.

pub mod config;
pub mod core;
pub mod documents;
pub mod queue;

// Re-export configuration types and functions
pub use config::{
    backup_database, ensure_data_dir, get_backup_path, get_data_dir, health_check, init_pool,
    print_env_help, DatabaseConfig, DatabaseHealth,
};

// Re-export core database types and functions
pub use core::*;

// Re-export queue types and functions
pub use queue::{
    create_queue_tables, FileAnalysis, QueueItem, QueuePriority, QueueSource, QueueStage,
    RepoCache, TodoItem, GITHUB_USERNAME,
};

// Re-export document types and functions
pub use documents::{
    count_documents, count_documents_by_type, create_chunks, create_document, delete_document,
    delete_document_chunks, delete_document_embeddings, get_all_embeddings, get_document,
    get_document_chunks, get_document_embeddings, get_document_tags, get_unindexed_documents,
    list_documents, mark_document_indexed, search_documents_by_tags, search_documents_by_title,
    store_embedding, update_document,
};
