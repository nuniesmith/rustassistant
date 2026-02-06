//! Rustassistant CLI
//!
//! Command-line interface for managing notes, repositories, and tasks.

use clap::{Parser, Subcommand};
use colored::Colorize;

use std::path::PathBuf;

// Import from our crate
use rustassistant::cli::{
    handle_github_command, handle_queue_command, handle_report_command, handle_scan_command,
    GithubCommands, QueueCommands, ReportCommands, ScanCommands,
};
use rustassistant::db::{
    self, create_note, get_next_task, get_stats, list_notes, list_repositories, list_tasks,
    search_notes, update_task_status,
};
use rustassistant::repo_cache::{CacheType, RepoCache};
use rustassistant::repo_cache_sql::{CacheSetParams as SqlCacheSetParams, RepoCacheSql};

// ============================================================================
// CLI Structure
// ============================================================================

#[derive(Parser)]
#[command(name = "rustassistant")]
#[command(about = "Developer workflow management tool", version)]
#[command(author = "nuniesmith")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage notes
    Note {
        #[command(subcommand)]
        action: NoteAction,
    },

    /// Manage repositories
    Repo {
        #[command(subcommand)]
        action: RepoAction,
    },

    /// Manage tasks
    Tasks {
        #[command(subcommand)]
        action: TaskAction,
    },

    /// Manage processing queue
    Queue {
        #[command(subcommand)]
        action: QueueCommands,
    },

    /// Scan repositories
    Scan {
        #[command(subcommand)]
        action: ScanCommands,
    },

    /// Generate reports
    Report {
        #[command(subcommand)]
        action: ReportCommands,
    },

    /// Get the next recommended task
    Next,

    /// Show statistics
    Stats,

    /// Test API connection (XAI/Grok)
    TestApi,

    /// Generate documentation
    Docs {
        #[command(subcommand)]
        action: DocsAction,
    },

    /// Refactoring assistant
    Refactor {
        #[command(subcommand)]
        action: RefactorAction,
    },

    /// Manage repository cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },

    /// GitHub integration
    Github {
        #[command(subcommand)]
        action: GithubCommands,
    },
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
        /// Maximum number of notes to show
        #[arg(short, long, default_value = "10")]
        limit: i64,

        /// Filter by status (inbox, processed, archived)
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by project
        #[arg(short, long)]
        project: Option<String>,

        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
    },

    /// Search notes
    Search {
        /// Search query
        query: String,

        /// Maximum results
        #[arg(short, long, default_value = "10")]
        limit: i64,
    },
}

#[derive(Subcommand)]
enum RepoAction {
    /// Add a repository to track
    Add {
        /// Path to repository
        path: String,

        /// Display name (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,
    },

    /// List tracked repositories
    List,

    /// Remove a repository
    Remove {
        /// Repository ID
        id: String,
    },

    /// Enable auto-scanning for a repository
    EnableAutoScan {
        /// Repository ID or path
        repo: String,

        /// Scan interval in minutes (default: 60)
        #[arg(short, long)]
        interval: Option<i64>,
    },

    /// Disable auto-scanning for a repository
    DisableAutoScan {
        /// Repository ID or path
        repo: String,
    },

    /// Force an immediate scan check
    ForceScan {
        /// Repository ID or path
        repo: String,
    },
}

#[derive(Subcommand)]
enum DocsAction {
    /// Generate documentation for a module/file
    Module {
        /// File path
        file: String,

        /// Output file (prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Generate README for repository
    Readme {
        /// Repository path
        #[arg(default_value = ".")]
        repo: String,

        /// Output file (prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
enum RefactorAction {
    /// Analyze a file for refactoring opportunities
    Analyze {
        /// File path to analyze
        file: String,
    },

    /// Generate refactoring plan for a file
    Plan {
        /// File path
        file: String,

        /// Specific smell ID to focus on (optional)
        #[arg(short, long)]
        smell: Option<String>,
    },
}

#[derive(Subcommand)]
enum CacheAction {
    /// Initialize cache structure in a repository
    Init {
        /// Repository path (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Show cache status and statistics
    Status {
        /// Repository path (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Clear cache entries
    Clear {
        /// Repository path (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Cache type to clear (analysis, docs, refactor, todos)
        #[arg(short = 't', long)]
        cache_type: Option<String>,

        /// Clear all cache types
        #[arg(short, long)]
        all: bool,
    },

    /// Migrate cache from JSON to SQLite
    Migrate {
        /// Source path (JSON cache directory)
        #[arg(short, long)]
        source: Option<String>,

        /// Destination path (SQLite database file)
        #[arg(short, long)]
        destination: Option<String>,

        /// Create backup before migration
        #[arg(short, long)]
        backup: bool,

        /// Verify migration after completion
        #[arg(short, long)]
        verify: bool,
    },
}

#[derive(Subcommand)]
enum TaskAction {
    /// List tasks
    List {
        /// Maximum number of tasks
        #[arg(short, long, default_value = "20")]
        limit: i64,

        /// Filter by status (pending, in_progress, done)
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by max priority (1=critical, 2=high, 3=medium, 4=low)
        #[arg(short, long)]
        priority: Option<i32>,
    },

    /// Mark a task as done
    Done {
        /// Task ID
        id: String,
    },

    /// Start working on a task
    Start {
        /// Task ID
        id: String,
    },
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment
    dotenvy::dotenv().ok();

    // Initialize tracing for debug logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Get database URL
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/rustassistant.db".into());

    // Initialize database
    let pool = db::init_db(&database_url).await?;

    match cli.command {
        Commands::Note { action } => handle_note_action(&pool, action).await?,
        Commands::Repo { action } => handle_repo_action(&pool, action).await?,
        Commands::Tasks { action } => handle_task_action(&pool, action).await?,
        Commands::Queue { action } => handle_queue_command(&pool, action).await?,
        Commands::Scan { action } => handle_scan_command(&pool, action).await?,
        Commands::Report { action } => handle_report_command(&pool, action).await?,
        Commands::Next => handle_next(&pool).await?,
        Commands::Stats => handle_stats(&pool).await?,
        Commands::TestApi => handle_test_api().await?,
        Commands::Docs { action } => handle_docs_action(&pool, action).await?,
        Commands::Refactor { action } => handle_refactor_action(&pool, action).await?,
        Commands::Cache { action } => handle_cache_action(action).await?,
        Commands::Github { action } => handle_github_command(action, &pool).await?,
    }

    Ok(())
}

// ============================================================================
// Note Handlers
// ============================================================================

async fn handle_note_action(pool: &sqlx::SqlitePool, action: NoteAction) -> anyhow::Result<()> {
    match action {
        NoteAction::Add {
            content,
            tags,
            project,
        } => {
            let note = create_note(pool, &content, tags.as_deref(), project.as_deref()).await?;

            println!("{} Note created", "✓".green());
            println!("  {} {}", "ID:".dimmed(), note.id);
            println!("  {} {}", "Content:".dimmed(), note.content);
            if let Some(t) = &note.tags {
                println!("  {} {}", "Tags:".dimmed(), t);
            }
            if let Some(p) = &note.project {
                println!("  {} {}", "Project:".dimmed(), p);
            }
        }

        NoteAction::List {
            limit,
            status,
            project,
            tag,
        } => {
            let notes = list_notes(
                pool,
                limit,
                status.as_deref(),
                project.as_deref(),
                tag.as_deref(),
            )
            .await?;

            if notes.is_empty() {
                println!(
                    "{} No notes found. Add one with: {} note add \"Your note\"",
                    "📝".dimmed(),
                    "rustassistant".cyan()
                );
            } else {
                println!("📝 Notes ({}):\n", notes.len());
                for note in notes {
                    print_note(&note);
                }
            }
        }

        NoteAction::Search { query, limit } => {
            let notes = search_notes(pool, &query, limit).await?;

            if notes.is_empty() {
                println!("{} No notes matching \"{}\"", "🔍".dimmed(), query);
            } else {
                println!("🔍 Found {} notes matching \"{}\":\n", notes.len(), query);
                for note in notes {
                    print_note(&note);
                }
            }
        }
    }

    Ok(())
}

fn print_note(note: &db::Note) {
    let status_icon = match note.status.as_str() {
        "inbox" => "📥",
        "processed" => "✅",
        "archived" => "📦",
        _ => "📝",
    };

    println!("  {} [{}] {}", status_icon, note.id.dimmed(), note.content);

    let mut meta = Vec::new();
    if let Some(tags) = &note.tags {
        meta.push(format!("tags: {}", tags));
    }
    if let Some(project) = &note.project {
        meta.push(format!("project: {}", project));
    }
    if !meta.is_empty() {
        println!("     {}", meta.join(" | ").dimmed());
    }
    println!();
}

// ============================================================================
// Repo Handlers
// ============================================================================

async fn handle_repo_action(pool: &sqlx::SqlitePool, action: RepoAction) -> anyhow::Result<()> {
    match action {
        RepoAction::Add { path, name } => {
            // Expand and canonicalize path
            let expanded = shellexpand::tilde(&path).to_string();
            let canonical = std::fs::canonicalize(&expanded)?;
            let path_str = canonical.to_string_lossy().to_string();

            // Derive name from path if not provided
            let name = name.unwrap_or_else(|| {
                canonical
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unnamed")
                    .to_string()
            });

            // Check if path exists
            if !canonical.exists() {
                anyhow::bail!("Path does not exist: {}", path_str);
            }

            let repo = db::add_repository(pool, &path_str, &name).await?;

            println!("{} Repository added", "✓".green());
            println!("  {} {}", "ID:".dimmed(), repo.id);
            println!("  {} {}", "Name:".dimmed(), repo.name);
            println!("  {} {}", "Path:".dimmed(), repo.path);
        }

        RepoAction::List => {
            let repos = list_repositories(pool).await?;

            if repos.is_empty() {
                println!(
                    "{} No repositories tracked. Add one with: {} repo add <path>",
                    "📂".dimmed(),
                    "rustassistant".cyan()
                );
            } else {
                println!("📂 Tracked repositories ({}):\n", repos.len());
                for repo in repos {
                    let analyzed = repo
                        .last_analyzed
                        .map(|ts| {
                            chrono::DateTime::from_timestamp(ts, 0)
                                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                .unwrap_or_else(|| "unknown".into())
                        })
                        .unwrap_or_else(|| "never".into());

                    println!("  📁 {} ({})", repo.name.cyan(), repo.id.dimmed());
                    println!("     {} {}", "Path:".dimmed(), repo.path);
                    println!("     {} {}", "Analyzed:".dimmed(), analyzed);
                    println!();
                }
            }
        }

        RepoAction::Remove { id } => {
            db::remove_repository(pool, &id).await?;
            println!("{} Repository removed: {}", "✓".green(), id);
        }

        RepoAction::EnableAutoScan { repo, interval } => {
            // Resolve repo ID
            let repo_id = if repo.starts_with("gh-") || repo.len() == 36 {
                repo
            } else {
                // Try to find by path or name
                let repos = list_repositories(pool).await?;
                repos
                    .iter()
                    .find(|r| r.path == repo || r.name == repo)
                    .map(|r| r.id.clone())
                    .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo))?
            };

            rustassistant::auto_scanner::enable_auto_scan(pool, &repo_id, interval).await?;

            let interval_str = interval.unwrap_or(60);
            println!(
                "{} Auto-scan enabled for repository (interval: {} minutes)",
                "✓".green(),
                interval_str
            );
        }

        RepoAction::DisableAutoScan { repo } => {
            // Resolve repo ID
            let repo_id = if repo.starts_with("gh-") || repo.len() == 36 {
                repo
            } else {
                // Try to find by path or name
                let repos = list_repositories(pool).await?;
                repos
                    .iter()
                    .find(|r| r.path == repo || r.name == repo)
                    .map(|r| r.id.clone())
                    .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo))?
            };

            rustassistant::auto_scanner::disable_auto_scan(pool, &repo_id).await?;
            println!("{} Auto-scan disabled for repository", "✓".green());
        }

        RepoAction::ForceScan { repo } => {
            // Resolve repo ID
            let repo_id = if repo.starts_with("gh-") || repo.len() == 36 {
                repo
            } else {
                // Try to find by path or name
                let repos = list_repositories(pool).await?;
                repos
                    .iter()
                    .find(|r| r.path == repo || r.name == repo)
                    .map(|r| r.id.clone())
                    .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo))?
            };

            rustassistant::auto_scanner::force_scan(pool, &repo_id).await?;
            println!(
                "{} Forced scan check - will scan on next cycle",
                "✓".green()
            );
        }
    }

    Ok(())
}

// ============================================================================
// Task Handlers
// ============================================================================

async fn handle_task_action(pool: &sqlx::SqlitePool, action: TaskAction) -> anyhow::Result<()> {
    match action {
        TaskAction::List {
            limit,
            status,
            priority,
        } => {
            let tasks = list_tasks(pool, limit, status.as_deref(), priority, None).await?;

            if tasks.is_empty() {
                println!("{} No tasks found", "📋".dimmed());
            } else {
                println!("📋 Tasks ({}):\n", tasks.len());
                for task in tasks {
                    print_task(&task);
                }
            }
        }

        TaskAction::Done { id } => {
            update_task_status(pool, &id, "done").await?;
            println!("{} Task marked as done: {}", "✓".green(), id);
        }

        TaskAction::Start { id } => {
            update_task_status(pool, &id, "in_progress").await?;
            println!("{} Task started: {}", "▶".blue(), id);
        }
    }

    Ok(())
}

fn print_task(task: &db::Task) {
    let priority_icon = match task.priority {
        1 => "🔴",
        2 => "🟠",
        3 => "🟡",
        4 => "🟢",
        _ => "⚪",
    };

    let priority_label = match task.priority {
        1 => "CRITICAL",
        2 => "HIGH",
        3 => "MEDIUM",
        4 => "LOW",
        _ => "UNKNOWN",
    };

    let status_icon = match task.status.as_str() {
        "pending" => "⏳",
        "in_progress" => "▶️",
        "done" => "✅",
        _ => "❓",
    };

    println!(
        "  {} {} [{}] {}",
        priority_icon,
        status_icon,
        task.id.cyan(),
        task.title
    );
    println!("     {} {}", "Priority:".dimmed(), priority_label);

    if let Some(desc) = &task.description {
        if !desc.is_empty() {
            println!("     {}", desc.dimmed());
        }
    }

    if let Some(file) = &task.file_path {
        let line = task
            .line_number
            .map(|n| format!(":{}", n))
            .unwrap_or_default();
        println!("     {} {}{}", "File:".dimmed(), file, line);
    }

    println!();
}

// ============================================================================
// Other Handlers
// ============================================================================

async fn handle_next(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    match get_next_task(pool).await? {
        Some(task) => {
            println!("🎯 Next recommended task:\n");
            print_task(&task);
            println!(
                "Start working on it: {} tasks start {}",
                "rustassistant".cyan(),
                task.id
            );
        }
        None => {
            println!("🎉 No pending tasks! Time to relax or add some work.");
        }
    }

    Ok(())
}

async fn handle_stats(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    let stats = get_stats(pool).await?;

    println!("📊 Rustassistant Statistics\n");
    println!("  {} {}", "Total notes:".dimmed(), stats.total_notes);
    println!("  {} {}", "Inbox notes:".dimmed(), stats.inbox_notes);
    println!("  {} {}", "Repositories:".dimmed(), stats.total_repos);
    println!("  {} {}", "Total tasks:".dimmed(), stats.total_tasks);
    println!("  {} {}", "Pending tasks:".dimmed(), stats.pending_tasks);

    Ok(())
}

async fn handle_test_api() -> anyhow::Result<()> {
    println!("🔌 Testing XAI API connection...\n");

    let api_key = std::env::var("XAI_API_KEY");

    match api_key {
        Ok(key) if !key.is_empty() => {
            println!("  {} XAI API key found", "✓".green());
            println!(
                "  {} Key prefix: {}...",
                "🔑".dimmed(),
                &key[..12.min(key.len())]
            );

            // TODO: Actually test the API connection
            // For now, just check if the key exists
            println!(
                "\n  {} API connection test not yet implemented",
                "⚠".yellow()
            );
            println!(
                "  {} Add XAI client code to test actual connectivity",
                "ℹ".blue()
            );
        }
        _ => {
            println!("  {} XAI_API_KEY not set", "✗".red());
            println!(
                "\n  Set it in your .env file or environment:\n  export XAI_API_KEY=xai-your-key-here"
            );
        }
    }

    Ok(())
}

async fn handle_refactor_action(
    pool: &sqlx::SqlitePool,
    action: RefactorAction,
) -> anyhow::Result<()> {
    use rustassistant::db::Database;
    use rustassistant::refactor_assistant::{RefactorAssistant, SmellSeverity};

    let db = Database::from_pool(pool.clone());
    let assistant = RefactorAssistant::new(db).await?;

    match action {
        RefactorAction::Analyze { file } => {
            // Use SQLite cache organized by repo in XDG cache directory
            let repo_path = std::env::current_dir()?;
            let cache = RepoCacheSql::new_for_repo(&repo_path).await?;
            let repo_path_str = repo_path.to_string_lossy().to_string();

            // Read file content for cache checking
            let file_content = std::fs::read_to_string(&file)?;

            // Check cache first
            let analysis = if let Some(cached) = cache
                .get(
                    CacheType::Refactor,
                    &file,
                    &file_content,
                    "xai",
                    "grok-beta",
                    None,
                    None,
                )
                .await?
            {
                println!("📦 Using cached analysis for {}\n", file);
                serde_json::from_value(cached)?
            } else {
                println!("🔍 Analyzing {} for refactoring opportunities...\n", file);
                let analysis = assistant.analyze_file(&file).await?;

                // Cache the result
                let result_json = serde_json::to_value(&analysis)?;
                cache
                    .set(SqlCacheSetParams {
                        cache_type: CacheType::Refactor,
                        repo_path: &repo_path_str,
                        file_path: &file,
                        content: &file_content,
                        provider: "xai",
                        model: "grok-beta",
                        result: result_json,
                        tokens_used: analysis.tokens_used,
                        prompt_hash: None,
                        schema_version: None,
                    })
                    .await?;

                if let Some(tokens) = analysis.tokens_used {
                    println!("💾 Analysis cached (tokens used: {})\n", tokens);
                } else {
                    println!("💾 Analysis cached\n");
                }

                analysis
            };

            println!("📊 Refactoring Analysis:\n");
            println!("  {} {}", "File:".dimmed(), file);
            println!(
                "  {} {}",
                "Code Smells Found:".dimmed(),
                analysis.code_smells.len()
            );
            println!();

            if analysis.code_smells.is_empty() {
                println!("{} No code smells detected! Code looks good.", "✓".green());
            } else {
                for smell in &analysis.code_smells {
                    let severity_icon = match smell.severity {
                        SmellSeverity::Critical => "🔴",
                        SmellSeverity::High => "🟠",
                        SmellSeverity::Medium => "🟡",
                        SmellSeverity::Low => "🟢",
                    };

                    let location = if let Some(ref loc) = smell.location {
                        if let Some(line) = loc.line_start {
                            format!("Line {}", line)
                        } else {
                            "Unknown location".to_string()
                        }
                    } else {
                        "Unknown location".to_string()
                    };
                    println!("  {} {:?} ({})", severity_icon, smell.smell_type, location);
                    println!("     {}", smell.description);
                    println!();
                }
            }

            if !analysis.suggestions.is_empty() {
                println!("💡 Refactoring Suggestions:");
                for (i, suggestion) in analysis.suggestions.iter().enumerate() {
                    println!(
                        "  {}. {} ({:?})",
                        i + 1,
                        suggestion.title,
                        suggestion.refactoring_type
                    );
                    println!("     {}", suggestion.description);
                    println!();
                }

                println!(
                    "\nGenerate a detailed plan with: {} refactor plan {}",
                    "rustassistant".cyan(),
                    file
                );
            }
        }

        RefactorAction::Plan { file, smell: _ } => {
            println!("📋 Generating refactoring plan for {}...\n", file);

            let analysis = assistant.analyze_file(&file).await?;

            if analysis.code_smells.is_empty() {
                println!("{} No code smells found. Nothing to refactor!", "✓".green());
                return Ok(());
            }

            // For now, just use the file path to generate plan
            // The generate_plan method will analyze and create a comprehensive plan
            let plan = assistant.generate_plan(&file, "").await?;

            println!("📋 Refactoring Plan:\n");
            println!("  {} {}", "Title:".dimmed(), plan.title);
            println!("  {} {}", "Goal:".dimmed(), plan.goal);
            println!("  {} {:?}", "Estimated Effort:".dimmed(), plan.total_effort);
            println!("  {} {}", "Files:".dimmed(), plan.files.join(", "));
            println!();

            if !plan.steps.is_empty() {
                println!("Steps:");
                for step in &plan.steps {
                    println!("  {}. {}", step.step_number, step.description);
                    println!("     Effort: {:?}", step.effort);
                    if !step.affected_files.is_empty() {
                        println!("     Files: {}", step.affected_files.join(", "));
                    }
                }
                println!();
            }

            if !plan.risks.is_empty() {
                println!("⚠️  Risks:");
                for risk in &plan.risks {
                    println!("  • {} ({})", risk.description, risk.mitigation);
                }
                println!();
            }

            if !plan.benefits.is_empty() {
                println!("✨ Benefits:");
                for benefit in &plan.benefits {
                    println!("  • {}", benefit);
                }
                println!();
            }
        }
    }

    Ok(())
}

async fn handle_docs_action(pool: &sqlx::SqlitePool, action: DocsAction) -> anyhow::Result<()> {
    use rustassistant::db::Database;
    use rustassistant::doc_generator::DocGenerator;

    let db = Database::from_pool(pool.clone());
    let generator = DocGenerator::new(db).await?;

    match action {
        DocsAction::Module { file, output } => {
            // Use SQLite cache organized by repo in XDG cache directory
            let repo_path = std::env::current_dir()?;
            let cache = RepoCacheSql::new_for_repo(&repo_path).await?;
            let repo_path_str = repo_path.to_string_lossy().to_string();

            // Read file content for cache checking
            let file_content = std::fs::read_to_string(&file)?;

            // Check cache first
            let doc = if let Some(cached) = cache
                .get(
                    CacheType::Docs,
                    &file,
                    &file_content,
                    "xai",
                    "grok-beta",
                    None,
                    None,
                )
                .await?
            {
                println!("📦 Using cached documentation for {}\n", file);
                serde_json::from_value(cached)?
            } else {
                println!("📝 Generating documentation for {}...\n", file);
                let doc = generator.generate_module_docs(&file).await?;

                // Cache the result
                let result_json = serde_json::to_value(&doc)?;
                cache
                    .set(SqlCacheSetParams {
                        cache_type: CacheType::Docs,
                        repo_path: &repo_path_str,
                        file_path: &file,
                        content: &file_content,
                        provider: "xai",
                        model: "grok-beta",
                        result: result_json,
                        tokens_used: None,
                        prompt_hash: None,
                        schema_version: None,
                    })
                    .await?;
                println!("💾 Documentation cached\n");

                doc
            };

            let markdown = generator.format_module_doc(&doc);

            if let Some(output_path) = output {
                std::fs::write(&output_path, &markdown)?;
                println!("{} Documentation written to {}", "✓".green(), output_path);
            } else {
                println!("{}", markdown);
            }
        }

        DocsAction::Readme { repo, output } => {
            println!("📖 Generating README for {}...\n", repo);

            let content = generator.generate_readme(&repo).await?;
            let markdown = generator.format_readme(&content);

            if let Some(output_path) = output {
                std::fs::write(&output_path, &markdown)?;
                println!("{} README written to {}", "✓".green(), output_path);
            } else {
                println!("{}", markdown);
            }
        }
    }

    Ok(())
}

// ============================================================================
// Cache Handlers
// ============================================================================

async fn handle_cache_action(action: CacheAction) -> anyhow::Result<()> {
    match action {
        CacheAction::Init { path } => {
            let repo_path = path.unwrap_or_else(|| ".".to_string());
            let cache = RepoCache::new(&repo_path)?;

            println!("{} Cache initialized", "✓".green());
            println!("  {} {}", "Location:".dimmed(), cache.cache_dir().display());
            println!();
            println!("Cache structure created:");
            println!("  - cache/analysis/");
            println!("  - cache/docs/");
            println!("  - cache/refactor/");
            println!("  - cache/todos/");
        }

        CacheAction::Status { path } => {
            // Use SQLite cache for stats
            let repo_path = if let Some(p) = path {
                PathBuf::from(p)
            } else {
                std::env::current_dir()?
            };

            let cache = RepoCacheSql::new_for_repo(&repo_path).await?;
            let stats = cache.stats().await?;

            // Use default budget config ($3/month)
            let budget_config = rustassistant::BudgetConfig::default();

            // Compute cache location
            use sha2::{Digest, Sha256};
            let canonical_path = repo_path
                .canonicalize()
                .unwrap_or_else(|_| repo_path.clone());
            let mut hasher = Sha256::new();
            hasher.update(canonical_path.to_string_lossy().as_bytes());
            let hash = hasher.finalize();
            let repo_hash = format!("{:x}", hash)[..8].to_string();

            let cache_dir = if let Some(cache_home) = std::env::var_os("XDG_CACHE_HOME") {
                PathBuf::from(cache_home)
            } else if let Some(home) = dirs::home_dir() {
                home.join(".cache")
            } else {
                PathBuf::from(".")
            };
            let cache_location = cache_dir
                .join("rustassistant")
                .join("repos")
                .join(&repo_hash)
                .join("cache.db");

            println!("📦 SQLite Cache Summary");
            println!("  Repository: {}", canonical_path.display());
            println!("  Cache Location: {}", cache_location.display());
            println!();

            // Group by cache type
            for type_stats in &stats.by_type {
                println!("  {} cache:", type_stats.cache_type);
                println!("    Entries: {}", type_stats.entries);
                println!("    Tokens: {}", type_stats.tokens);
                println!("    Estimated cost: ${:.4}", type_stats.cost);
            }

            println!();
            println!("  Total entries: {}", stats.total_entries);
            println!("  Total tokens: {}", stats.total_tokens);
            println!("  Total estimated cost: ${:.4}", stats.estimated_cost);
            println!();

            // Budget status
            let remaining = budget_config.monthly_budget - stats.estimated_cost;
            let percentage = (stats.estimated_cost / budget_config.monthly_budget) * 100.0;

            println!("💰 Budget Status:");
            if percentage >= budget_config.alert_threshold * 100.0 {
                println!(
                    "  🔴 Budget Alert: ${:.2} / ${:.2} ({:.1}%)",
                    stats.estimated_cost, budget_config.monthly_budget, percentage
                );
            } else if percentage >= budget_config.warning_threshold * 100.0 {
                println!(
                    "  ⚠️  Budget Warning: ${:.2} / ${:.2} ({:.1}%)",
                    stats.estimated_cost, budget_config.monthly_budget, percentage
                );
            } else {
                println!(
                    "  ✅ Budget OK: ${:.2} / ${:.2} ({:.1}%)",
                    stats.estimated_cost, budget_config.monthly_budget, percentage
                );
            }
            println!("  Remaining: ${:.2}", remaining);

            if stats.total_tokens > 0 {
                let tokens_per_dollar =
                    stats.total_tokens as f64 / stats.estimated_cost.max(0.0001);
                let remaining_tokens = (remaining * tokens_per_dollar) as usize;
                println!("  Estimated tokens remaining: ~{}", remaining_tokens);
            }
        }

        CacheAction::Clear {
            path,
            cache_type,
            all,
        } => {
            // Use SQLite cache
            let repo_path = if let Some(p) = path {
                PathBuf::from(p)
            } else {
                std::env::current_dir()?
            };

            let cache = RepoCacheSql::new_for_repo(&repo_path).await?;

            if all {
                let removed = cache.clear_all().await?;
                println!("{} Cleared {} cache entries", "✓".green(), removed);
            } else if let Some(type_str) = cache_type {
                let cache_type = match type_str.as_str() {
                    "analysis" => CacheType::Analysis,
                    "docs" => CacheType::Docs,
                    "refactor" => CacheType::Refactor,
                    "todos" => CacheType::Todos,
                    _ => {
                        eprintln!(
                            "{} Invalid cache type. Use: analysis, docs, refactor, or todos",
                            "✗".red()
                        );
                        return Ok(());
                    }
                };

                let removed = cache.clear_type(cache_type).await?;
                println!(
                    "{} Cleared {} {} cache entries",
                    "✓".green(),
                    removed,
                    type_str
                );
            } else {
                eprintln!("{} Specify --all or --cache-type", "✗".red());
            }
        }

        CacheAction::Migrate {
            source,
            destination,
            backup,
            verify,
        } => {
            use rustassistant::CacheMigrator;

            // Determine source and destination paths
            let source_path = source.unwrap_or_else(|| {
                let home = dirs::home_dir().expect("Could not find home directory");
                home.join(".rustassistant/cache/repos")
                    .to_string_lossy()
                    .to_string()
            });

            let dest_path = destination.unwrap_or_else(|| {
                let home = dirs::home_dir().expect("Could not find home directory");
                home.join(".rustassistant/cache.db")
                    .to_string_lossy()
                    .to_string()
            });

            println!("{} Starting cache migration", "🔄".blue());
            println!("  Source: {}", source_path);
            println!("  Destination: {}", dest_path);
            println!();

            // Create migrator
            let migrator = CacheMigrator::new(&source_path, &dest_path).await?;

            // Create backup if requested
            if backup {
                let backup_path = format!("{}.backup", source_path);
                println!("{} Creating backup at {}", "💾".blue(), backup_path);
                migrator.backup(&backup_path)?;
                println!("{} Backup created\n", "✓".green());
            }

            // Run migration with progress
            println!("{} Migrating entries...", "🔄".blue());
            let result = migrator
                .migrate(|progress| {
                    if progress.migrated % 10 == 0 || progress.migrated == progress.total {
                        println!(
                            "  Progress: {}/{} ({} failed)",
                            progress.migrated, progress.total, progress.failed
                        );
                    }
                })
                .await?;

            println!();
            println!("{} Migration complete!", "✓".green());
            println!("  Total entries: {}", result.total_entries);
            println!("  Migrated: {}", result.total_migrated);
            println!("  Failed: {}", result.total_failed);
            println!("  Source size: {} bytes", result.source_size);
            println!("  Destination size: {} bytes", result.destination_size);
            println!(
                "  Space saved: {} bytes ({:.1}%)",
                result.space_saved,
                if result.source_size > 0 {
                    (result.space_saved as f64 / result.source_size as f64) * 100.0
                } else {
                    0.0
                }
            );

            if !result.failures.is_empty() {
                println!();
                println!("{} Failed migrations:", "⚠️".yellow());
                for failure in result.failures.iter().take(5) {
                    println!("  - {}: {}", failure.file_path, failure.error);
                }
                if result.failures.len() > 5 {
                    println!("  ... and {} more", result.failures.len() - 5);
                }
            }

            // Verify if requested
            if verify {
                println!();
                println!("{} Verifying migration...", "🔍".blue());
                let valid = migrator.verify().await?;
                if valid {
                    println!("{} Verification passed!", "✓".green());
                } else {
                    println!("{} Verification failed - entry count mismatch", "✗".red());
                }
            }
        }
    }

    Ok(())
}
