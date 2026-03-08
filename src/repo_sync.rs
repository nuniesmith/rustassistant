// src/repo_sync.rs
// STUB: RustAssistant RepoSyncService
// Handles repo registration, tree snapshots, TODO extraction, and .rustassistant/ cache management
// TODO: integrate with existing document indexing pipeline in src/search.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredRepo {
    pub id: String,
    pub name: String,
    pub local_path: PathBuf,
    pub remote_url: Option<String>,
    pub branch: String,
    pub last_synced: Option<u64>, // unix timestamp
    pub active: bool,
}

impl RegisteredRepo {
    pub fn new(name: impl Into<String>, local_path: impl Into<PathBuf>) -> Self {
        let name = name.into();
        let id = slugify(&name);
        Self {
            id,
            name,
            local_path: local_path.into(),
            remote_url: None,
            branch: "main".to_string(),
            last_synced: None,
            active: true,
        }
    }

    /// Path to this repo's .rustassistant/ cache dir.
    pub fn cache_dir(&self) -> PathBuf {
        self.local_path.join(".rustassistant")
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.cache_dir().join("manifest.json")
    }

    pub fn tree_path(&self) -> PathBuf {
        self.cache_dir().join("tree.txt")
    }

    pub fn todos_path(&self) -> PathBuf {
        self.cache_dir().join("todos.json")
    }

    pub fn symbols_path(&self) -> PathBuf {
        self.cache_dir().join("symbols.json")
    }

    pub fn context_path(&self) -> PathBuf {
        self.cache_dir().join("context.md")
    }
}

// ---------------------------------------------------------------------------
// Manifest (written to .rustassistant/manifest.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoManifest {
    pub id: String,
    pub name: String,
    pub remote_url: Option<String>,
    pub branch: String,
    pub synced_at: u64,
    pub file_count: usize,
    pub rust_file_count: usize,
    pub cargo_crate_name: Option<String>,
    pub rustassistant_version: String,
}

// ---------------------------------------------------------------------------
// TODO item (written to .rustassistant/todos.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub kind: TodoKind,
    pub message: String,
    pub file: String, // relative path from repo root
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoKind {
    Todo,
    Fixme,
    Stub,
    Hack,
    Note,
}

impl TodoKind {
    fn from_tag(tag: &str) -> Option<Self> {
        match tag.to_uppercase().as_str() {
            "TODO" => Some(Self::Todo),
            "FIXME" => Some(Self::Fixme),
            "STUB" => Some(Self::Stub),
            "HACK" => Some(Self::Hack),
            "NOTE" => Some(Self::Note),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Symbol (written to .rustassistant/symbols.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub name: String,
    pub file: String,
    pub line: usize,
    pub is_pub: bool,
    pub is_async: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    TypeAlias,
    Const,
    Mod,
}

// ---------------------------------------------------------------------------
// SyncResult
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub repo_id: String,
    pub files_walked: usize,
    pub todos_found: usize,
    pub symbols_found: usize,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// RepoSyncService
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct RepoSyncService {
    /// In-memory registry — TODO: back this with SQLite via sqlx
    repos: HashMap<String, RegisteredRepo>,
    /// File extensions to index (skip target/, .git/, node_modules/ etc.)
    include_extensions: Vec<String>,
    /// Directories to always skip
    skip_dirs: Vec<String>,
}

impl Default for RepoSyncService {
    fn default() -> Self {
        Self {
            repos: HashMap::new(),
            include_extensions: vec![
                "rs".into(),
                "toml".into(),
                "md".into(),
                "sh".into(),
                "yml".into(),
                "yaml".into(),
                "json".into(),
                "sql".into(),
            ],
            skip_dirs: vec![
                "target".into(),
                ".git".into(),
                "node_modules".into(),
                ".sqlx".into(),
                "dist".into(),
            ],
        }
    }
}

impl RepoSyncService {
    pub fn new() -> Self {
        Self::default()
    }

    // -----------------------------------------------------------------------
    // Registration
    // -----------------------------------------------------------------------

    /// Register a repo by local path. Creates .rustassistant/ dir if missing.
    pub async fn register(&mut self, repo: RegisteredRepo) -> anyhow::Result<String> {
        let id = repo.id.clone();
        info!(repo = %id, path = ?repo.local_path, "Registering repo");

        // Ensure .rustassistant/ dir exists
        let cache_dir = repo.cache_dir();
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).await?;
            info!(path = ?cache_dir, "Created .rustassistant/ cache dir");
        }

        // Write a .gitignore inside .rustassistant/ to exclude embeddings binary
        let gitignore = cache_dir.join(".gitignore");
        if !gitignore.exists() {
            fs::write(&gitignore, "embeddings.bin\n").await?;
        }

        self.repos.insert(id.clone(), repo);
        Ok(id)
    }

    pub fn get_repo(&self, id: &str) -> Option<&RegisteredRepo> {
        self.repos.get(id)
    }

    pub fn list_repos(&self) -> Vec<&RegisteredRepo> {
        self.repos.values().filter(|r| r.active).collect()
    }

    pub fn remove_repo(&mut self, id: &str) -> bool {
        self.repos.remove(id).is_some()
    }

    // -----------------------------------------------------------------------
    // Full sync
    // -----------------------------------------------------------------------

    /// Perform a full sync of a registered repo: tree + todos + symbols + manifest.
    pub async fn sync(&mut self, repo_id: &str) -> anyhow::Result<SyncResult> {
        let repo = self
            .repos
            .get(repo_id)
            .ok_or_else(|| anyhow::anyhow!("Repo '{}' not registered", repo_id))?
            .clone();

        info!(repo = %repo_id, "Starting sync");
        let start = std::time::Instant::now();
        let mut errors = Vec::new();

        // 1. Walk tree
        let (tree_txt, walked_files) = self.walk_tree(&repo).await.unwrap_or_else(|e| {
            errors.push(format!("tree walk failed: {e}"));
            (String::new(), vec![])
        });

        // 2. Extract TODOs
        let todos = self
            .extract_todos(&repo, &walked_files)
            .await
            .unwrap_or_else(|e| {
                errors.push(format!("todo extraction failed: {e}"));
                vec![]
            });

        // 3. Extract symbols
        let symbols = self
            .extract_symbols(&repo, &walked_files)
            .await
            .unwrap_or_else(|e| {
                errors.push(format!("symbol extraction failed: {e}"));
                vec![]
            });

        let rust_files = walked_files
            .iter()
            .filter(|p| p.extension().map(|e| e == "rs").unwrap_or(false))
            .count();

        // 4. Write cache files
        let cache_dir = repo.cache_dir();

        write_file(&repo.tree_path(), &tree_txt).await?;
        write_json(&repo.todos_path(), &todos).await?;
        write_json(&repo.symbols_path(), &symbols).await?;

        // 5. Write manifest
        let manifest = RepoManifest {
            id: repo.id.clone(),
            name: repo.name.clone(),
            remote_url: repo.remote_url.clone(),
            branch: repo.branch.clone(),
            synced_at: unix_now(),
            file_count: walked_files.len(),
            rust_file_count: rust_files,
            cargo_crate_name: read_crate_name(&repo.local_path).await,
            rustassistant_version: env!("CARGO_PKG_VERSION").to_string(),
        };
        write_json(&repo.manifest_path(), &manifest).await?;

        // 6. Generate context.md summary
        let context = build_context_md(&repo, &manifest, &todos, &symbols);
        write_file(&repo.context_path(), &context).await?;

        // 7. Update last_synced timestamp
        if let Some(r) = self.repos.get_mut(repo_id) {
            r.last_synced = Some(unix_now());
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        info!(
            repo = %repo_id,
            files = walked_files.len(),
            todos = todos.len(),
            symbols = symbols.len(),
            duration_ms,
            "Sync complete"
        );

        Ok(SyncResult {
            repo_id: repo_id.to_string(),
            files_walked: walked_files.len(),
            todos_found: todos.len(),
            symbols_found: symbols.len(),
            duration_ms,
            errors,
        })
    }

    // -----------------------------------------------------------------------
    // Tree walker
    // -----------------------------------------------------------------------

    async fn walk_tree(&self, repo: &RegisteredRepo) -> anyhow::Result<(String, Vec<PathBuf>)> {
        let root = &repo.local_path;
        let mut lines = Vec::new();
        let mut files = Vec::new();

        walk_dir(
            root,
            root,
            &self.skip_dirs,
            &self.include_extensions,
            &mut lines,
            &mut files,
        )
        .await?;

        lines.sort();
        let tree_txt = format!(
            "# Project tree: {}\n# Generated: {}\n\n{}\n",
            repo.name,
            unix_now(),
            lines.join("\n")
        );

        Ok((tree_txt, files))
    }

    // -----------------------------------------------------------------------
    // TODO extractor
    // -----------------------------------------------------------------------

    async fn extract_todos(
        &self,
        repo: &RegisteredRepo,
        files: &[PathBuf],
    ) -> anyhow::Result<Vec<TodoItem>> {
        let mut todos = Vec::new();
        let root = &repo.local_path;

        for path in files {
            // Only scan text files likely to contain comments
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !["rs", "toml", "md", "sh", "yml", "yaml"].contains(&ext) {
                continue;
            }

            let content = match fs::read_to_string(path).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(path = ?path, error = %e, "Could not read file for TODO extraction");
                    continue;
                }
            };

            let relative = path.strip_prefix(root).unwrap_or(path);

            for (line_idx, line) in content.lines().enumerate() {
                // Match: // TODO: ..., // FIXME: ..., // STUB: ..., # TODO: ...
                if let Some(item) =
                    parse_todo_line(line, relative.to_string_lossy().as_ref(), line_idx + 1)
                {
                    todos.push(item);
                }
            }
        }

        debug!(count = todos.len(), "Extracted TODO items");
        Ok(todos)
    }

    // -----------------------------------------------------------------------
    // Symbol extractor (naive regex-free line scanner)
    // -----------------------------------------------------------------------

    async fn extract_symbols(
        &self,
        repo: &RegisteredRepo,
        files: &[PathBuf],
    ) -> anyhow::Result<Vec<Symbol>> {
        let mut symbols = Vec::new();
        let root = &repo.local_path;

        for path in files {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "rs" {
                continue;
            }

            let content = match fs::read_to_string(path).await {
                Ok(c) => c,
                Err(_) => continue,
            };

            let relative = path.strip_prefix(root).unwrap_or(path);

            for (line_idx, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if let Some(sym) =
                    parse_symbol_line(trimmed, relative.to_string_lossy().as_ref(), line_idx + 1)
                {
                    symbols.push(sym);
                }
            }
        }

        debug!(count = symbols.len(), "Extracted symbols");
        Ok(symbols)
    }

    // -----------------------------------------------------------------------
    // Context builder for chat injection
    // -----------------------------------------------------------------------

    /// Build a compact context string suitable for LLM prompt injection.
    /// Keeps it under ~2000 chars to avoid context bloat.
    pub async fn build_prompt_context(&self, repo_id: &str) -> anyhow::Result<String> {
        let repo = self
            .repos
            .get(repo_id)
            .ok_or_else(|| anyhow::anyhow!("Repo not found: {}", repo_id))?;

        let tree = fs::read_to_string(repo.tree_path())
            .await
            .unwrap_or_default();
        let todos_raw = fs::read_to_string(repo.todos_path())
            .await
            .unwrap_or_default();
        let todos: Vec<TodoItem> = serde_json::from_str(&todos_raw).unwrap_or_default();

        // Truncate tree to first 80 lines
        let tree_snippet: String = tree.lines().take(80).collect::<Vec<_>>().join("\n");

        // Top 10 TODOs
        let todo_snippet: String = todos
            .iter()
            .take(10)
            .map(|t| format!("  [{:?}] {}:{} — {}", t.kind, t.file, t.line, t.message))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(format!(
            "### Repo: {}\n\n#### Project Tree (truncated)\n```\n{}\n```\n\n#### Open TODOs\n{}\n",
            repo.name, tree_snippet, todo_snippet
        ))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Recursive async directory walker.
#[async_recursion::async_recursion]
async fn walk_dir(
    root: &Path,
    current: &Path,
    skip_dirs: &[String],
    include_exts: &[String],
    lines: &mut Vec<String>,
    files: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    // TODO: replace with tokio::fs::ReadDir stream for better performance on large repos
    let mut entries = fs::read_dir(current).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if skip_dirs.iter().any(|s| s == &name) {
                continue;
            }
            walk_dir(root, &path, skip_dirs, include_exts, lines, files).await?;
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !include_exts.iter().any(|e| e == ext) {
                continue;
            }
            let relative = path.strip_prefix(root).unwrap_or(&path);
            lines.push(relative.to_string_lossy().to_string());
            files.push(path);
        }
    }

    Ok(())
}

fn parse_todo_line(line: &str, file: &str, line_num: usize) -> Option<TodoItem> {
    // Match: // TODO: message  OR  # TODO: message  OR  -- TODO: message
    let patterns = ["// ", "# ", "-- ", "/* "];
    let tags = ["TODO", "FIXME", "STUB", "HACK", "NOTE"];

    let stripped = line.trim();
    for prefix in &patterns {
        if let Some(rest) = stripped.strip_prefix(prefix) {
            for tag in &tags {
                let tag_colon = format!("{}:", tag);
                let tag_space = format!("{} ", tag);
                let msg = if let Some(m) = rest.strip_prefix(&tag_colon) {
                    Some(m.trim().to_string())
                } else if let Some(m) = rest.strip_prefix(&tag_space) {
                    Some(m.trim().to_string())
                } else {
                    None
                };
                if let Some(message) = msg {
                    return Some(TodoItem {
                        kind: TodoKind::from_tag(tag).unwrap_or(TodoKind::Todo),
                        message,
                        file: file.to_string(),
                        line: line_num,
                    });
                }
            }
        }
    }
    None
}

fn parse_symbol_line(line: &str, file: &str, line_num: usize) -> Option<Symbol> {
    // TODO: replace with syn-based AST parsing for accuracy
    // This naive scanner covers ~90% of top-level declarations
    let is_pub = line.starts_with("pub ");
    let is_async = line.contains("async fn");

    let check_line = line
        .trim_start_matches("pub ")
        .trim_start_matches("async ")
        .trim_start_matches("unsafe ");

    let (kind, name) = if check_line.starts_with("fn ") {
        let n = extract_name(check_line, "fn ")?;
        (SymbolKind::Function, n)
    } else if check_line.starts_with("struct ") {
        let n = extract_name(check_line, "struct ")?;
        (SymbolKind::Struct, n)
    } else if check_line.starts_with("enum ") {
        let n = extract_name(check_line, "enum ")?;
        (SymbolKind::Enum, n)
    } else if check_line.starts_with("trait ") {
        let n = extract_name(check_line, "trait ")?;
        (SymbolKind::Trait, n)
    } else if check_line.starts_with("impl ") {
        let n = extract_impl_name(check_line)?;
        (SymbolKind::Impl, n)
    } else if check_line.starts_with("type ") {
        let n = extract_name(check_line, "type ")?;
        (SymbolKind::TypeAlias, n)
    } else if check_line.starts_with("const ") {
        let n = extract_name(check_line, "const ")?;
        (SymbolKind::Const, n)
    } else if check_line.starts_with("mod ") {
        let n = extract_name(check_line, "mod ")?;
        (SymbolKind::Mod, n)
    } else {
        return None;
    };

    Some(Symbol {
        kind,
        name,
        file: file.to_string(),
        line: line_num,
        is_pub,
        is_async,
    })
}

fn extract_name(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn extract_impl_name(line: &str) -> Option<String> {
    // impl Foo  OR  impl<T> Foo  OR  impl Trait for Foo
    let rest = line.strip_prefix("impl")?;
    let rest = rest.trim();
    // Skip generic params
    let rest = if rest.starts_with('<') {
        let end = rest.find('>')?;
        rest[end + 1..].trim()
    } else {
        rest
    };
    // If "Trait for Type", take the type
    let name_part = if let Some(idx) = rest.find(" for ") {
        &rest[idx + 5..]
    } else {
        rest
    };
    let name: String = name_part
        .trim()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

async fn read_crate_name(path: &Path) -> Option<String> {
    let cargo_toml = path.join("Cargo.toml");
    let content = fs::read_to_string(cargo_toml).await.ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("name") {
            let val = rest.trim().trim_start_matches('=').trim().trim_matches('"');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn build_context_md(
    repo: &RegisteredRepo,
    manifest: &RepoManifest,
    todos: &[TodoItem],
    symbols: &[Symbol],
) -> String {
    let todo_count = todos.len();
    let stub_count = todos.iter().filter(|t| t.kind == TodoKind::Stub).count();
    let fixme_count = todos.iter().filter(|t| t.kind == TodoKind::Fixme).count();
    let pub_fn_count = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function && s.is_pub)
        .count();
    let struct_count = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Struct)
        .count();

    format!(
        r#"# RustAssistant Context: {}

**Crate:** {}
**Branch:** {}
**Synced:** {}
**Files:** {} total, {} Rust

## Annotations
- {} total TODOs ({} STUBs, {} FIXMEs)

## Symbols
- {} public functions
- {} structs

## Remote
{}
"#,
        repo.name,
        manifest.cargo_crate_name.as_deref().unwrap_or("unknown"),
        repo.branch,
        manifest.synced_at,
        manifest.file_count,
        manifest.rust_file_count,
        todo_count,
        stub_count,
        fixme_count,
        pub_fn_count,
        struct_count,
        repo.remote_url.as_deref().unwrap_or("not set"),
    )
}

async fn write_file(path: &Path, content: &str) -> anyhow::Result<()> {
    let mut f = fs::File::create(path).await?;
    f.write_all(content.as_bytes()).await?;
    Ok(())
}

async fn write_json<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    write_file(path, &json).await
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_todo_rust_comment() {
        let item = parse_todo_line("    // TODO: implement retry logic", "src/webhooks.rs", 42);
        assert!(item.is_some());
        let item = item.unwrap();
        assert_eq!(item.kind, TodoKind::Todo);
        assert_eq!(item.line, 42);
        assert!(item.message.contains("retry"));
    }

    #[test]
    fn parse_stub_tag() {
        let item = parse_todo_line(
            "// STUB: generated by rustassistant",
            "src/cache_layer.rs",
            10,
        );
        assert!(item.is_some());
        assert_eq!(item.unwrap().kind, TodoKind::Stub);
    }

    #[test]
    fn parse_symbol_pub_fn() {
        let sym = parse_symbol_line("pub async fn handle_webhook(", "src/webhooks.rs", 55);
        assert!(sym.is_some());
        let sym = sym.unwrap();
        assert_eq!(sym.kind, SymbolKind::Function);
        assert!(sym.is_pub);
        assert!(sym.is_async);
        assert_eq!(sym.name, "handle_webhook");
    }

    #[test]
    fn parse_symbol_struct() {
        let sym = parse_symbol_line("pub struct WebhookEvent {", "src/webhooks.rs", 12);
        assert!(sym.is_some());
        assert_eq!(sym.unwrap().kind, SymbolKind::Struct);
    }

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("My Cool Repo"), "my-cool-repo");
        assert_eq!(slugify("rustassistant"), "rustassistant");
    }
}
