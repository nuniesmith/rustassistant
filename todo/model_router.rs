// src/model_router.rs
// STUB: RustAssistant ModelRouter — routes tasks between local Ollama and remote Grok API
// TODO: wire into existing LLM client once local model serving is confirmed

use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Task classification
// ---------------------------------------------------------------------------

/// Describes the nature of a code/chat task so the router can pick the right model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind {
    /// Generate a stub, skeleton, or boilerplate (80% quality target — local model)
    ScaffoldStub,
    /// Insert TODO / FIXME / STUB tags into existing code
    TodoTagging,
    /// Walk a project tree and summarise structure
    TreeSummary,
    /// Extract symbols (fns, structs, traits, impls) from a file
    SymbolExtraction,
    /// Answer a general question about a repo or codebase
    RepoQuestion,
    /// Complex architectural reasoning or multi-file refactor (remote model)
    ArchitecturalReason,
    /// Final review / critique of generated code (remote model)
    CodeReview,
    /// Anything that doesn't clearly fit above — fall back to remote
    Unknown,
}

impl TaskKind {
    /// True if this task should be handled by the local model.
    pub fn is_local(&self) -> bool {
        matches!(
            self,
            TaskKind::ScaffoldStub
                | TaskKind::TodoTagging
                | TaskKind::TreeSummary
                | TaskKind::SymbolExtraction
                | TaskKind::RepoQuestion
        )
    }
}

impl fmt::Display for TaskKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ---------------------------------------------------------------------------
// Model targets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTarget {
    /// Local Ollama instance (e.g. Qwen2.5-Coder:7b)
    Local { model: String, base_url: String },
    /// Remote xAI Grok API
    Remote { model: String, api_key: String },
}

impl fmt::Display for ModelTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelTarget::Local { model, .. } => write!(f, "local/{}", model),
            ModelTarget::Remote { model, .. } => write!(f, "remote/{}", model),
        }
    }
}

// ---------------------------------------------------------------------------
// Router config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRouterConfig {
    pub local_model: String,
    pub local_base_url: String,
    pub remote_model: String,
    pub remote_api_key: String,
    /// If true, always use remote regardless of task kind (useful for debugging)
    pub force_remote: bool,
    /// If local Ollama is unreachable, fall back to remote automatically
    pub fallback_to_remote: bool,
}

impl Default for ModelRouterConfig {
    fn default() -> Self {
        Self {
            local_model: "qwen2.5-coder:7b".to_string(),
            local_base_url: "http://localhost:11434".to_string(),
            remote_model: "grok-2-latest".to_string(),
            remote_api_key: String::new(),
            force_remote: false,
            fallback_to_remote: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ModelRouter {
    config: ModelRouterConfig,
}

impl ModelRouter {
    pub fn new(config: ModelRouterConfig) -> Self {
        Self { config }
    }

    /// Classify a raw user prompt into a TaskKind.
    ///
    /// TODO: replace naive keyword matching with a lightweight classifier
    ///       or a dedicated classification prompt against the local model.
    pub fn classify_prompt(&self, prompt: &str) -> TaskKind {
        let lower = prompt.to_lowercase();

        if lower.contains("stub")
            || lower.contains("scaffold")
            || lower.contains("skeleton")
            || lower.contains("placeholder")
            || lower.contains("boilerplate")
            || lower.contains("generate")
            || lower.contains("create a fn")
            || lower.contains("create a struct")
        {
            return TaskKind::ScaffoldStub;
        }

        if lower.contains("todo") || lower.contains("fixme") || lower.contains("tag") {
            return TaskKind::TodoTagging;
        }

        if lower.contains("tree") || lower.contains("structure") || lower.contains("layout") {
            return TaskKind::TreeSummary;
        }

        if lower.contains("symbol") || lower.contains("extract") || lower.contains("list function") {
            return TaskKind::SymbolExtraction;
        }

        if lower.contains("review") || lower.contains("critique") || lower.contains("is this correct") {
            return TaskKind::CodeReview;
        }

        if lower.contains("architect") || lower.contains("design") || lower.contains("refactor") {
            return TaskKind::ArchitecturalReason;
        }

        if lower.contains("repo") || lower.contains("codebase") || lower.contains("where is") {
            return TaskKind::RepoQuestion;
        }

        TaskKind::Unknown
    }

    /// Decide which model target to use for a given task.
    pub fn route(&self, task: &TaskKind) -> ModelTarget {
        if self.config.force_remote || !task.is_local() {
            info!(task = %task, target = "remote", "Routing to remote model");
            return ModelTarget::Remote {
                model: self.config.remote_model.clone(),
                api_key: self.config.remote_api_key.clone(),
            };
        }

        debug!(task = %task, target = "local", "Routing to local model");
        ModelTarget::Local {
            model: self.config.local_model.clone(),
            base_url: self.config.local_base_url.clone(),
        }
    }

    /// Route by raw prompt — classifies then routes.
    pub fn route_prompt(&self, prompt: &str) -> (TaskKind, ModelTarget) {
        let kind = self.classify_prompt(prompt);
        let target = self.route(&kind);
        (kind, target)
    }

    /// Called when a local model request fails. Returns fallback target if configured.
    pub fn on_local_failure(&self, task: &TaskKind) -> Option<ModelTarget> {
        if self.config.fallback_to_remote {
            warn!(task = %task, "Local model failed — falling back to remote");
            Some(ModelTarget::Remote {
                model: self.config.remote_model.clone(),
                api_key: self.config.remote_api_key.clone(),
            })
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// LLM completion request/response (shared shape for both targets)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub system_prompt: Option<String>,
    pub user_prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    /// Injected repo context (tree, symbols, todos) — prepended to user_prompt
    pub repo_context: Option<String>,
}

impl CompletionRequest {
    pub fn for_stub(user_prompt: impl Into<String>, repo_context: Option<String>) -> Self {
        Self {
            system_prompt: Some(RUST_STUB_SYSTEM_PROMPT.to_string()),
            user_prompt: user_prompt.into(),
            max_tokens: 1024,
            temperature: 0.2, // low temp for deterministic scaffold
            repo_context,
        }
    }

    /// Build the final prompt string injecting repo context if present.
    pub fn build_prompt(&self) -> String {
        match &self.repo_context {
            Some(ctx) => format!(
                "### Repo Context\n{}\n\n### Task\n{}",
                ctx, self.user_prompt
            ),
            None => self.user_prompt.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub content: String,
    pub model_used: String,
    pub task_kind: TaskKind,
    pub used_fallback: bool,
    pub tokens_used: Option<u32>,
}

// ---------------------------------------------------------------------------
// System prompt for Rust stub generation
// ---------------------------------------------------------------------------

pub const RUST_STUB_SYSTEM_PROMPT: &str = r#"
You are a Rust code scaffolding assistant. Your job is to generate high-quality stub code (~80% complete).

Rules:
- Always use `// TODO: <description>` on lines that need real implementation
- Always use `// STUB: generated by rustassistant` at the top of each generated block
- Prefer `unimplemented!("stub: <reason>")` over `todo!()` for fn bodies
- Preserve existing type signatures exactly — do not invent types
- Match the module structure shown in the repo context
- For async fns, use `async fn` and return `Result<T, crate::error::AppError>`
- Always derive `Debug` on new structs unless there's a reason not to
- Add `#[allow(dead_code)]` to stub impls to avoid compiler noise
- Output ONLY valid Rust code — no markdown fences, no prose
"#;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn router() -> ModelRouter {
        ModelRouter::new(ModelRouterConfig::default())
    }

    #[test]
    fn classifies_stub_prompts() {
        let r = router();
        assert_eq!(r.classify_prompt("generate a stub for the retry handler"), TaskKind::ScaffoldStub);
        assert_eq!(r.classify_prompt("scaffold the webhook module"), TaskKind::ScaffoldStub);
    }

    #[test]
    fn classifies_review_as_remote() {
        let r = router();
        let (kind, target) = r.route_prompt("review this code and tell me if it's correct");
        assert_eq!(kind, TaskKind::CodeReview);
        assert!(matches!(target, ModelTarget::Remote { .. }));
    }

    #[test]
    fn stub_routes_local() {
        let r = router();
        let (kind, target) = r.route_prompt("create a stub for the cache invalidation fn");
        assert_eq!(kind, TaskKind::ScaffoldStub);
        assert!(matches!(target, ModelTarget::Local { .. }));
    }

    #[test]
    fn force_remote_overrides() {
        let mut config = ModelRouterConfig::default();
        config.force_remote = true;
        let r = ModelRouter::new(config);
        let (_, target) = r.route_prompt("generate a stub");
        assert!(matches!(target, ModelTarget::Remote { .. }));
    }
}
