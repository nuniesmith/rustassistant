//! # RustAssistant - Developer Workflow Management System
//!
//! A Rust-based workflow manager for solo developers to track repos, capture ideas,
//! and leverage LLM-powered insights.
//!
//! ## Features
//!
//! - **Note & Thought Capture**: Quick note input with tag-based categorization
//! - **Repository Management**: Track GitHub repos with cached directory trees
//! - **LLM-Powered Analysis**: Grok 4.1 API integration for code insights
//! - **File Scoring**: Quality, security, and complexity assessment
//! - **Task Generation**: Automatically generate actionable tasks
//! - **Solo Dev Workflow**: Research → Planning → Prototype → Production
//! - **RAG System**: Git-friendly vector storage for semantic search
//! - **Cost Management**: Efficient LLM usage with budget controls
//!
//! ## Architecture
//!
//! - Static analysis for fast pattern detection
//! - LLM integration for deep code insights
//! - Git operations for repository tracking
//! - Vector storage for RAG system
//! - RESTful API and CLI interface

pub mod cache;
pub mod code_review;
pub mod config;
pub mod context;
pub mod context_builder;
pub mod db;
pub mod directory_tree;
pub mod enhanced_scanner;
pub mod error;
pub mod formatter;
pub mod git;
pub mod grok_client;
pub mod grok_reasoning;
pub mod refactor_assistant;
pub mod test_generator;
pub mod web_ui;

pub mod llm;
pub mod llm_audit;
pub mod llm_config;
pub mod parser;
pub mod query_templates;
pub mod repo_analysis;
pub mod research;
pub mod response_cache;
pub mod scanner;
pub mod scoring;
pub mod server;
pub mod tag_schema;
pub mod tags;
pub mod tasks;
pub mod tests_runner;
pub mod todo_scanner;
pub mod tree_state;
pub mod types;

pub use cache::{AuditCache, CacheEntry, CacheStats};
pub use code_review::{
    CodeReview, CodeReviewer, FileReview, IssueSeverity, ReviewIssue, ReviewStats,
};
pub use config::Config;
pub use context::{ContextBuilder as OldContextBuilder, GlobalContextBundle};
pub use context_builder::{Context, ContextBuilder, ContextFile, QueryBuilder};
pub use db::{Database, DatabaseStats, LlmCost, LlmCostStats, Note, NoteStatus, Repository};
pub use directory_tree::{DirectoryTreeBuilder, Hotspot, TreeSummary};
pub use enhanced_scanner::EnhancedScanner;
pub use error::{AuditError, Result};
pub use formatter::{BatchFormatResult, CodeFormatter, FormatMode, FormatResult, Formatter};
pub use git::GitManager;
pub use grok_client::{FileScoreResult, GrokClient, QuickAnalysisResult};
pub use grok_reasoning::{
    analyze_all_batches, BatchAnalysisResult, FileAnalysisResult, FileBatch, FileForAnalysis,
    GrokReasoningClient, IdentifiedIssue, Improvement, RetryConfig, TokenUsage,
};
pub use llm_audit::{
    ArchitectureInsights, AuditMode, FileAnalysis, FileLlmAnalysis, FileRelationships,
    FullAuditResult, LlmAuditor, MasterReview, Recommendation, RegularAuditResult, SecurityConcern,
    TechDebtArea,
};
pub use llm_config::{
    claude_models, BudgetStatus, CacheConfig, FileSelectionConfig, LimitsConfig, LlmConfig,
    ProviderConfig, LLM_CONFIG_FILE,
};
pub use query_templates::{QueryTemplate, TemplateCategory, TemplateRegistry};
pub use refactor_assistant::{
    CodeLocation, CodeSmell, CodeSmellType, EffortEstimate, PlanStep, RefactorAssistant,
    RefactoringAnalysis, RefactoringExample, RefactoringPlan, RefactoringPriority,
    RefactoringSuggestion, RefactoringType, Risk, SmellSeverity,
};
pub use repo_analysis::{
    FileMetadata, LanguageStats, RepoAnalyzer, RepoNodeType, RepoTree, TreeNode,
};
pub use research::{ResearchBreakdown, ResearchTask};
pub use response_cache::{CacheStats as ResponseCacheStats, CachedResponse, ResponseCache};
pub use scanner::Scanner;
pub use scoring::{
    CodebaseScore, ComplexityIndicators, FileScore, FileScorer, ScoreBreakdown, ScoringWeights,
    TodoBreakdown,
};
pub use server::run_server;
pub use tag_schema::{
    CodeAge, CodeStatus, Complexity, DirectoryNode, IssuesSummary, NodeStats, NodeType, Priority,
    SimpleIssueDetector, TagCategory, TagSchema, TagValidation,
};
pub use tags::TagScanner;
pub use tasks::TaskGenerator;
pub use test_generator::{
    Fixture, GeneratedTests, TestCase, TestFramework, TestGapAnalysis, TestGenerator, TestType,
    UntestFunction,
};
pub use tests_runner::{TestResults, TestRunner};
pub use todo_scanner::{TodoItem, TodoPriority, TodoScanner, TodoSummary};
pub use tree_state::{
    CategoryChangeSummary, ChangeType, DiffSummary, FileCategory, FileChange, FileState, TreeDiff,
    TreeState, TreeStateManager, TreeSummaryStats,
};
pub use types::*;

/// Re-export commonly used types
pub mod prelude {
    pub use crate::config::Config;
    pub use crate::context::{ContextBuilder as OldContextBuilder, GlobalContextBundle};
    pub use crate::context_builder::{Context, ContextBuilder, ContextFile, QueryBuilder};
    pub use crate::db::{
        Database, DatabaseStats, LlmCost, LlmCostStats, Note, NoteStatus, Repository,
    };
    pub use crate::directory_tree::{DirectoryTreeBuilder, Hotspot, TreeSummary};
    pub use crate::enhanced_scanner::EnhancedScanner;
    pub use crate::error::{AuditError, Result};
    pub use crate::git::GitManager;
    pub use crate::grok_client::{FileScoreResult, GrokClient, QuickAnalysisResult};
    pub use crate::grok_reasoning::{
        analyze_all_batches, BatchAnalysisResult, FileAnalysisResult, FileBatch, FileForAnalysis,
        GrokReasoningClient, IdentifiedIssue, Improvement, RetryConfig, TokenUsage,
    };
    pub use crate::query_templates::{QueryTemplate, TemplateCategory, TemplateRegistry};
    pub use crate::repo_analysis::{
        FileMetadata, LanguageStats, RepoAnalyzer, RepoNodeType, RepoTree, TreeNode,
    };
    pub use crate::response_cache::{
        CacheStats as ResponseCacheStats, CachedResponse, ResponseCache,
    };

    pub use crate::research::{ResearchBreakdown, ResearchTask};
    pub use crate::scanner::Scanner;
    pub use crate::tag_schema::{
        CodeAge, CodeStatus, Complexity, DirectoryNode, IssuesSummary, NodeStats, NodeType,
        Priority, SimpleIssueDetector, TagCategory, TagSchema, TagValidation,
    };
    pub use crate::tags::TagScanner;
    pub use crate::tasks::TaskGenerator;
    pub use crate::tests_runner::{TestResults, TestRunner};
    pub use crate::todo_scanner::{TodoItem, TodoPriority, TodoScanner, TodoSummary};
    pub use crate::tree_state::{
        CategoryChangeSummary, ChangeType, DiffSummary, FileCategory, FileChange, FileState,
        TreeDiff, TreeState, TreeStateManager, TreeSummaryStats,
    };
    pub use crate::types::*;
}
