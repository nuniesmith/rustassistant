//! Rustassistant CLI - Minimal workflow management tool
//!
//! Phase 1 MVP: Focus on note capture and basic repo tracking

use clap::{Parser, Subcommand};
use rustassistant::db::{Database, Note, NoteStatus};
use rustassistant::repo_analysis::RepoAnalyzer;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "devflow")]
#[command(about = "Developer workflow management system", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Database file path
    #[arg(short, long, global = true, default_value = "data/rustassistant.db")]
    database: PathBuf,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Note management
    #[command(subcommand)]
    Note(NoteCommands),

    /// Repository tracking
    #[command(subcommand)]
    Repo(RepoCommands),

    /// AI-powered analysis with Grok
    #[command(subcommand)]
    Analyze(AnalyzeCommands),

    /// Show next recommended action
    Next,

    /// Show statistics
    Stats,

    /// Show LLM cost statistics
    Costs,

    /// Manage response cache
    #[command(subcommand)]
    Cache(CacheCommands),

    /// Use query templates
    #[command(subcommand)]
    Template(TemplateCommands),

    /// Code review automation
    #[command(subcommand)]
    Review(ReviewCommands),

    /// Test generation
    #[command(subcommand)]
    Test(TestCommands),

    /// Refactoring assistant
    #[command(subcommand)]
    Refactor(RefactorCommands),
}

#[derive(Subcommand)]
enum CacheCommands {
    /// Show cache statistics
    Stats,

    /// Clear all cached responses
    Clear,

    /// Clear expired cache entries
    Prune,

    /// Show most frequently accessed entries
    Hot {
        /// Number of entries to show
        #[arg(short, long, default_value = "10")]
        limit: i64,
    },
}

#[derive(Subcommand)]
enum ReviewCommands {
    /// Review git diff changes
    Diff {
        /// Repository path (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// Base branch to compare against
        #[arg(short, long)]
        base: Option<String>,

        /// Output file for review report
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Format output for GitHub PR comment
        #[arg(long)]
        github: bool,
    },

    /// Review specific files
    Files {
        /// Files to review
        #[arg(required = true)]
        files: Vec<PathBuf>,

        /// Output file for review report
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Format output for GitHub PR comment
        #[arg(long)]
        github: bool,
    },

    /// Generate PR description from changes
    Pr {
        /// Repository path (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// Base branch to compare against (defaults to 'main')
        #[arg(short, long, default_value = "main")]
        base: String,

        /// Output file for PR description
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum TemplateCommands {
    /// List all available templates
    List {
        /// Filter by keyword
        #[arg(short, long)]
        search: Option<String>,
    },

    /// Show template details
    Show {
        /// Template name
        name: String,
    },

    /// Use a template to generate a query
    Use {
        /// Template name
        name: String,

        /// Variables in key=value format
        #[arg(short = 'V', long)]
        var: Vec<String>,

        /// Execute the query immediately
        #[arg(short, long)]
        execute: bool,
    },
}

#[derive(Subcommand)]
enum TestCommands {
    /// Generate tests for a file
    Generate {
        /// File to generate tests for
        file: PathBuf,

        /// Specific function to test (optional)
        #[arg(short, long)]
        function: Option<String>,

        /// Output file for generated tests
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Format as markdown documentation
        #[arg(long)]
        markdown: bool,
    },

    /// Analyze test coverage gaps
    Gaps {
        /// File or directory to analyze
        path: PathBuf,

        /// Output file for gap analysis
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate test fixtures
    Fixtures {
        /// File containing data structures
        file: PathBuf,

        /// Output file for fixtures
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum RefactorCommands {
    /// Analyze file or directory for refactoring opportunities
    Analyze {
        /// File or directory to analyze
        path: PathBuf,

        /// Output file for analysis report
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Suggest specific refactoring for a code section
    Suggest {
        /// File to analyze
        file: PathBuf,

        /// Start line of code section
        #[arg(long)]
        start_line: Option<usize>,

        /// End line of code section
        #[arg(long)]
        end_line: Option<usize>,

        /// Output file for suggestions
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate comprehensive refactoring plan
    Plan {
        /// File or directory to create plan for
        path: PathBuf,

        /// Goal of the refactoring
        #[arg(short, long)]
        goal: String,

        /// Output file for plan
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum NoteCommands {
    /// Add a new note
    Add {
        /// Note content
        content: String,

        /// Tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,

        /// Status (inbox, active, processed, archived)
        #[arg(short, long, default_value = "inbox")]
        status: String,
    },

    /// List notes
    List {
        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Search notes
    Search {
        /// Search query
        query: String,
    },

    /// Show a specific note
    Show {
        /// Note ID
        id: i64,
    },

    /// Update note status
    Update {
        /// Note ID
        id: i64,

        /// New status
        #[arg(short, long)]
        status: Option<String>,

        /// New content
        #[arg(short, long)]
        content: Option<String>,
    },

    /// Delete a note
    Delete {
        /// Note ID
        id: i64,
    },

    /// Add tag to note
    Tag {
        /// Note ID
        id: i64,

        /// Tag name
        tag: String,
    },

    /// Remove tag from note
    Untag {
        /// Note ID
        id: i64,

        /// Tag name
        tag: String,
    },
}

#[derive(Subcommand)]
enum RepoCommands {
    /// Add a repository to track
    Add {
        /// Repository path
        path: PathBuf,

        /// Repository name (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,
    },

    /// List tracked repositories
    List,

    /// Show repository status
    Status {
        /// Repository name
        name: String,
    },

    /// Analyze repository and cache directory tree
    Analyze {
        /// Repository name
        name: String,

        /// Save tree to file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Show directory tree
    Tree {
        /// Repository name
        name: String,

        /// Maximum depth to display
        #[arg(short = 'D', long)]
        depth: Option<usize>,
    },

    /// List files in repository
    Files {
        /// Repository name
        name: String,

        /// Filter by language
        #[arg(short, long)]
        language: Option<String>,

        /// Show only largest files
        #[arg(long)]
        largest: Option<usize>,

        /// Show only recently modified
        #[arg(long)]
        recent: Option<usize>,
    },

    /// Remove a repository from tracking
    Remove {
        /// Repository name
        name: String,
    },
}

#[derive(Subcommand)]
enum AnalyzeCommands {
    /// Score a file using Grok AI
    File {
        /// File path to analyze
        path: PathBuf,
    },

    /// Quick analysis of code
    Quick {
        /// File path or inline code
        input: String,
    },

    /// Ask Grok a question
    Ask {
        /// Question to ask
        question: String,

        /// Optional context (file path or repo name)
        #[arg(short, long)]
        context: Option<String>,
    },

    /// Analyze entire repository with full context
    Repo {
        /// Repository name
        name: String,

        /// Focus on specific language
        #[arg(short, long)]
        language: Option<String>,

        /// Include recent files only
        #[arg(short, long)]
        recent: Option<usize>,
    },

    /// Ask a question with full repository context
    Query {
        /// Question to ask
        question: String,

        /// Repository name
        #[arg(short, long)]
        repo: String,

        /// Include notes in context
        #[arg(short = 'n', long)]
        with_notes: bool,

        /// Focus on specific language
        #[arg(short, long)]
        language: Option<String>,
    },

    /// Find patterns across codebase
    Patterns {
        /// Pattern to search for (e.g., "TODO", "unsafe", "unwrap")
        pattern: String,

        /// Repository name
        #[arg(short, long)]
        repo: String,

        /// Language filter
        #[arg(short, long)]
        language: Option<String>,
    },

    /// Batch analyze multiple files (more efficient)
    Batch {
        /// File paths or glob patterns
        #[arg(required = true)]
        files: Vec<PathBuf>,

        /// Output report to file
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Maximum files per batch
        #[arg(long, default_value = "20")]
        batch_size: usize,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if it exists (ignore errors if not found)
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    // Initialize logging
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("devflow=debug")
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter("devflow=info")
            .init();
    }

    // Connect to database
    let db = Database::new(&cli.database).await?;

    match cli.command {
        Commands::Note(cmd) => handle_note_command(db, cmd).await?,
        Commands::Repo(cmd) => handle_repo_command(db, cmd).await?,
        Commands::Analyze(cmd) => handle_analyze_command(db, cmd).await?,
        Commands::Next => handle_next(db).await?,
        Commands::Stats => handle_stats(db).await?,
        Commands::Costs => handle_costs(db).await?,
        Commands::Cache(cmd) => handle_cache_command(cmd).await?,
        Commands::Template(cmd) => handle_template_command(cmd).await?,
        Commands::Review(cmd) => handle_review_command(db, cmd).await?,
        Commands::Test(cmd) => handle_test_command(db, cmd).await?,
        Commands::Refactor(cmd) => handle_refactor_command(db, cmd).await?,
    }

    Ok(())
}

async fn handle_note_command(db: Database, cmd: NoteCommands) -> anyhow::Result<()> {
    match cmd {
        NoteCommands::Add {
            content,
            tags,
            status,
        } => {
            let status = parse_status(&status)?;
            let note_id = db.create_note(&content, status).await?;

            // Add tags if provided
            if let Some(tags_str) = tags {
                for tag in tags_str.split(',') {
                    let tag = tag.trim();
                    if !tag.is_empty() {
                        db.add_tag_to_note(note_id, tag).await?;
                    }
                }
            }

            println!("✓ Note created with ID: {}", note_id);
            if let Some(note) = db.get_note(note_id).await? {
                print_note(&note);
            }
        }

        NoteCommands::List { tag, status, limit } => {
            let status = status.as_ref().map(|s| parse_status(s)).transpose()?;
            let mut notes = db.list_notes(status, tag.as_deref(), None).await?;

            if let Some(lim) = limit {
                notes.truncate(lim);
            }

            if notes.is_empty() {
                println!("No notes found.");
            } else {
                println!("Found {} note(s):\n", notes.len());
                for note in notes {
                    print_note_summary(&note);
                    println!();
                }
            }
        }

        NoteCommands::Search { query } => {
            let notes = db.search_notes(&query).await?;

            if notes.is_empty() {
                println!("No notes found matching '{}'", query);
            } else {
                println!("Found {} note(s) matching '{}':\n", notes.len(), query);
                for note in notes {
                    print_note_summary(&note);
                    println!();
                }
            }
        }

        NoteCommands::Show { id } => {
            if let Some(note) = db.get_note(id).await? {
                print_note(&note);
            } else {
                println!("Note {} not found", id);
            }
        }

        NoteCommands::Update {
            id,
            status,
            content,
        } => {
            if let Some(status_str) = status {
                let status = parse_status(&status_str)?;
                db.update_note_status(id, status).await?;
                println!("✓ Updated note {} status to {}", id, status);
            }

            if let Some(new_content) = content {
                db.update_note_content(id, &new_content).await?;
                println!("✓ Updated note {} content", id);
            }

            if let Some(note) = db.get_note(id).await? {
                println!();
                print_note(&note);
            }
        }

        NoteCommands::Delete { id } => {
            db.delete_note(id).await?;
            println!("✓ Deleted note {}", id);
        }

        NoteCommands::Tag { id, tag } => {
            db.add_tag_to_note(id, &tag).await?;
            println!("✓ Added tag '{}' to note {}", tag, id);
        }

        NoteCommands::Untag { id, tag } => {
            db.remove_tag_from_note(id, &tag).await?;
            println!("✓ Removed tag '{}' from note {}", tag, id);
        }
    }

    Ok(())
}

async fn handle_repo_command(db: Database, cmd: RepoCommands) -> anyhow::Result<()> {
    match cmd {
        RepoCommands::Add { path, name } => {
            let path = path.canonicalize()?;
            let name = name.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("repo")
                    .to_string()
            });

            // Detect if it's a git repo
            let git_dir = path.join(".git");
            if !git_dir.exists() {
                println!("⚠ Warning: {} is not a git repository", path.display());
            }

            // Get remote URL if available (requires opening the git repo directly)
            let remote_url = git2::Repository::open(&path).ok().and_then(|repo| {
                repo.find_remote("origin")
                    .ok()
                    .and_then(|remote| remote.url().map(|s| s.to_string()))
            });

            let repo_id = db
                .add_repository(&name, path.to_str().unwrap(), remote_url.as_deref(), "main")
                .await?;

            println!("✓ Added repository '{}' (ID: {})", name, repo_id);
            println!("  Path: {}", path.display());
            if let Some(url) = remote_url {
                println!("  Remote: {}", url);
            }
        }

        RepoCommands::List => {
            let repos = db.list_repositories().await?;

            if repos.is_empty() {
                println!("No repositories tracked.");
                println!("\nAdd a repository with: devflow repo add <path>");
            } else {
                println!("Tracked repositories:\n");
                for repo in repos {
                    println!("  {} ({})", repo.name, repo.path);
                    if let Some(url) = repo.remote_url {
                        println!("    Remote: {}", url);
                    }
                    if let Some(analyzed) = repo.last_analyzed {
                        println!("    Last analyzed: {}", analyzed.format("%Y-%m-%d %H:%M"));
                    }
                    println!();
                }
            }
        }

        RepoCommands::Status { name } => {
            if let Some(repo) = db.get_repository(&name).await? {
                println!("Repository: {}", repo.name);
                println!("Path: {}", repo.path);
                if let Some(url) = repo.remote_url {
                    println!("Remote: {}", url);
                }
                println!("Branch: {}", repo.default_branch);
                if let Some(analyzed) = repo.last_analyzed {
                    println!("Last analyzed: {}", analyzed.format("%Y-%m-%d %H:%M:%S"));
                } else {
                    println!("Last analyzed: never");
                }
            } else {
                println!("Repository '{}' not found", name);
            }
        }

        RepoCommands::Analyze { name, output } => {
            if let Some(repo) = db.get_repository(&name).await? {
                println!("🔍 Analyzing repository '{}'...", name);
                let analyzer = RepoAnalyzer::new(&repo.path);
                let tree = analyzer.build_tree().await?;

                println!("✓ Analysis complete!");
                println!("  Files: {}", tree.total_files);
                println!("  Directories: {}", tree.total_dirs);
                println!(
                    "  Total size: {:.2} MB",
                    tree.total_size as f64 / (1024.0 * 1024.0)
                );

                if !tree.languages.is_empty() {
                    println!("\n  Languages:");
                    let mut langs: Vec<_> = tree.languages.iter().collect();
                    langs.sort_by(|a, b| b.1.file_count.cmp(&a.1.file_count));
                    for (lang, stats) in langs.iter().take(5) {
                        println!("    {} - {} files", lang, stats.file_count);
                    }
                }

                // Save to file if requested
                if let Some(output_path) = output {
                    tree.save_to_file(&output_path)?;
                    println!("\n✓ Saved tree to {}", output_path.display());
                }

                // Update last analyzed timestamp
                db.update_repository_analyzed(repo.id).await?;
            } else {
                println!("Repository '{}' not found", name);
            }
        }

        RepoCommands::Tree { name, depth } => {
            if let Some(repo) = db.get_repository(&name).await? {
                let analyzer = RepoAnalyzer::new(&repo.path);
                let tree = analyzer.build_tree().await?;
                tree.print_tree(depth);
            } else {
                println!("Repository '{}' not found", name);
            }
        }

        RepoCommands::Files {
            name,
            language,
            largest,
            recent,
        } => {
            if let Some(repo) = db.get_repository(&name).await? {
                let analyzer = RepoAnalyzer::new(&repo.path);
                let tree = analyzer.build_tree().await?;

                let files = if let Some(lang) = language {
                    RepoAnalyzer::get_files_by_language(&tree, &lang)
                } else if let Some(limit) = largest {
                    RepoAnalyzer::get_largest_files(&tree, limit)
                } else if let Some(limit) = recent {
                    RepoAnalyzer::get_recently_modified(&tree, limit)
                } else {
                    RepoAnalyzer::get_all_files(&tree)
                };

                println!("Files in '{}' ({} total):\n", name, files.len());
                for file in files.iter().take(50) {
                    print!(
                        "  {}",
                        file.path
                            .strip_prefix(&repo.path)
                            .unwrap_or(&file.path)
                            .display()
                    );
                    if let Some(ref metadata) = file.metadata {
                        if let Some(ref lang) = metadata.language {
                            print!(" [{}]", lang);
                        }
                        if metadata.size > 1024 * 1024 {
                            print!(" ({:.2} MB)", metadata.size as f64 / (1024.0 * 1024.0));
                        } else if metadata.size > 1024 {
                            print!(" ({:.2} KB)", metadata.size as f64 / 1024.0);
                        }
                    }
                    println!();
                }

                if files.len() > 50 {
                    println!("\n... and {} more files", files.len() - 50);
                }
            } else {
                println!("Repository '{}' not found", name);
            }
        }

        RepoCommands::Remove { name } => {
            db.delete_repository(&name).await?;
            println!("✓ Removed repository '{}'", name);
        }
    }

    Ok(())
}

async fn handle_analyze_command(db: Database, cmd: AnalyzeCommands) -> anyhow::Result<()> {
    use rustassistant::context_builder::ContextBuilder;
    use rustassistant::grok_client::GrokClient;

    // Create Grok client with caching
    let client = match GrokClient::from_env(db).await {
        Ok(c) => c,
        Err(_) => {
            println!("❌ Error: XAI_API_KEY or GROK_API_KEY environment variable not set");
            println!("\nTo use Grok AI features:");
            println!("  1. Get an API key from https://x.ai");
            println!("  2. Set environment variable: export XAI_API_KEY='your-key'");
            println!("  3. Or add to .env file: XAI_API_KEY=your-key");
            return Ok(());
        }
    };

    match cmd {
        AnalyzeCommands::File { path } => {
            println!("🤖 Analyzing file with Grok AI...");

            let content = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))?;

            let result = client.score_file(path.to_str().unwrap(), &content).await?;

            println!("\n📊 Analysis Results for {}:", path.display());
            println!("─────────────────────────────────────────");
            println!("Overall Score:        {:.1}/100", result.overall_score);
            println!("Security:             {:.1}/100", result.security_score);
            println!("Quality:              {:.1}/100", result.quality_score);
            println!("Complexity:           {:.1}/100", result.complexity_score);
            println!(
                "Maintainability:      {:.1}/100",
                result.maintainability_score
            );
            println!("─────────────────────────────────────────");

            if !result.summary.is_empty() {
                println!("\n📝 Summary:\n{}", result.summary);
            }

            if !result.issues.is_empty() {
                println!("\n⚠️  Issues Found:");
                for (i, issue) in result.issues.iter().enumerate() {
                    println!("  {}. {}", i + 1, issue);
                }
            }

            if !result.suggestions.is_empty() {
                println!("\n💡 Suggestions:");
                for (i, suggestion) in result.suggestions.iter().enumerate() {
                    println!("  {}. {}", i + 1, suggestion);
                }
            }

            // Show cost
            let cost = client.get_cost_last_n_days(1).await?;
            println!("\n💰 Cost today: ${:.4}", cost);
        }

        AnalyzeCommands::Quick { input } => {
            println!("🤖 Quick analysis with Grok AI...");

            let code = if std::path::Path::new(&input).exists() {
                std::fs::read_to_string(&input)?
            } else {
                input
            };

            let result = client.quick_analysis(&code).await?;

            println!("\n📊 Quick Analysis:");
            println!("─────────────────────────────────────────");
            println!("Quality Rating: {}/10", result.quality_rating);
            println!("\n{}", result.findings);

            if !result.concerns.is_empty() {
                println!("\n⚠️  Concerns:");
                for concern in &result.concerns {
                    println!("  • {}", concern);
                }
            }
        }

        AnalyzeCommands::Ask { question, context } => {
            println!("🤖 Asking Grok AI...");

            let ctx = if let Some(context_input) = context {
                if std::path::Path::new(&context_input).exists() {
                    Some(std::fs::read_to_string(&context_input)?)
                } else {
                    Some(context_input)
                }
            } else {
                None
            };

            let answer = client.ask(&question, ctx.as_deref()).await?;

            println!("\n💬 Grok's Answer:");
            println!("─────────────────────────────────────────");
            println!("{}", answer);
        }

        AnalyzeCommands::Repo {
            name,
            language,
            recent,
        } => {
            println!("🤖 Analyzing repository '{}' with full context...", name);

            // Check if repo exists
            let repo = match rustassistant::db::Database::new("data/rustassistant.db")
                .await?
                .get_repository(&name)
                .await?
            {
                Some(r) => r,
                None => {
                    println!("❌ Repository '{}' not found", name);
                    return Ok(());
                }
            };

            // Build context
            let db_for_context = rustassistant::db::Database::new("data/rustassistant.db").await?;
            let mut builder = ContextBuilder::new(db_for_context).with_repository(&name);

            if let Some(lang) = language {
                builder = builder.with_language(lang);
            }

            if let Some(count) = recent {
                builder = builder.with_recent_files(count);
            }

            println!("📊 Building context...");
            let context = builder.build().await?;

            println!(
                "✓ Context built: {} files, ~{} tokens",
                context.file_count(),
                context.estimated_tokens()
            );
            println!();

            // Analyze repository
            println!("🔍 Running AI analysis...");
            let analysis = client.analyze_repository(&context, Some(repo.id)).await?;

            println!("\n📊 Repository Analysis:");
            println!("─────────────────────────────────────────");
            println!("Overall Health: {:.1}/100", analysis.overall_health);
            println!("─────────────────────────────────────────");

            if !analysis.strengths.is_empty() {
                println!("\n✨ Strengths:");
                for strength in &analysis.strengths {
                    println!("  • {}", strength);
                }
            }

            if !analysis.weaknesses.is_empty() {
                println!("\n⚠️  Weaknesses:");
                for weakness in &analysis.weaknesses {
                    println!("  • {}", weakness);
                }
            }

            if !analysis.security_concerns.is_empty() {
                println!("\n🔒 Security Concerns:");
                for concern in &analysis.security_concerns {
                    println!("  • {}", concern);
                }
            }

            if !analysis.architecture_notes.is_empty() {
                println!("\n🏗️  Architecture Notes:");
                println!("{}", analysis.architecture_notes);
            }

            if !analysis.tech_debt_areas.is_empty() {
                println!("\n🔧 Technical Debt:");
                for debt in &analysis.tech_debt_areas {
                    println!("  • {}", debt);
                }
            }

            if !analysis.recommendations.is_empty() {
                println!("\n💡 Recommendations:");
                for (i, rec) in analysis.recommendations.iter().enumerate() {
                    println!("  {}. {}", i + 1, rec);
                }
            }

            let cost = client.get_cost_last_n_days(1).await?;
            println!("\n💰 Cost today: ${:.4}", cost);
        }

        AnalyzeCommands::Query {
            question,
            repo,
            with_notes,
            language,
        } => {
            println!("🤖 Querying with full codebase context...");

            // Check if repo exists
            let db_for_query = rustassistant::db::Database::new("data/rustassistant.db").await?;
            let repo_obj = match db_for_query.get_repository(&repo).await? {
                Some(r) => r,
                None => {
                    println!("❌ Repository '{}' not found", repo);
                    return Ok(());
                }
            };

            // Build context
            let mut builder = ContextBuilder::new(db_for_query).with_repository(&repo);

            if with_notes {
                builder = builder.with_notes();
            }

            if let Some(lang) = language {
                builder = builder.with_language(lang);
            }

            println!("📊 Building context...");
            let context = builder.build().await?;

            println!(
                "✓ Context: {} files, {} notes, ~{} tokens",
                context.file_count(),
                context.metadata.note_count,
                context.estimated_tokens()
            );
            println!();

            // Ask question with context
            let answer = client
                .ask_with_context(&question, &context, Some(repo_obj.id))
                .await?;

            println!("💬 Answer:");
            println!("─────────────────────────────────────────");
            println!("{}", answer);

            let cost = client.get_cost_last_n_days(1).await?;
            println!("\n💰 Cost today: ${:.4}", cost);
        }

        AnalyzeCommands::Patterns {
            pattern,
            repo,
            language,
        } => {
            println!("🔍 Searching for '{}' patterns...", pattern);

            // Check if repo exists
            let db_for_patterns = rustassistant::db::Database::new("data/rustassistant.db").await?;
            let repo_obj = match db_for_patterns.get_repository(&repo).await? {
                Some(r) => r,
                None => {
                    println!("❌ Repository '{}' not found", repo);
                    return Ok(());
                }
            };

            // Build context
            let mut builder = ContextBuilder::new(db_for_patterns).with_repository(&repo);

            if let Some(lang) = language {
                builder = builder.with_language(lang);
            }

            let context = builder.build().await?;

            println!(
                "✓ Context: {} files, ~{} tokens",
                context.file_count(),
                context.estimated_tokens()
            );
            println!();

            // Find patterns
            let findings = client
                .find_patterns(&context, &pattern, Some(repo_obj.id))
                .await?;

            if findings.is_empty() {
                println!("No instances of '{}' found", pattern);
            } else {
                println!("Found {} instances:\n", findings.len());
                for finding in findings {
                    println!("  {}", finding);
                }
            }

            let cost = client.get_cost_last_n_days(1).await?;
            println!("\n💰 Cost today: ${:.4}", cost);
        }

        AnalyzeCommands::Batch {
            files,
            output,
            batch_size,
        } => {
            println!("🤖 Batch analyzing {} files...", files.len());

            // Expand glob patterns and collect all files
            let mut all_files = Vec::new();
            for pattern in files {
                if pattern.to_string_lossy().contains('*') {
                    // Glob pattern
                    use walkdir::WalkDir;
                    let parent = pattern.parent().unwrap_or(std::path::Path::new("."));
                    let pattern_str = pattern.to_string_lossy().to_string();

                    for entry in WalkDir::new(parent)
                        .follow_links(true)
                        .into_iter()
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_type().is_file())
                    {
                        let path = entry.path();
                        if path
                            .to_string_lossy()
                            .contains(&pattern_str.replace("*", ""))
                        {
                            all_files.push(path.to_path_buf());
                        }
                    }
                } else if pattern.is_file() {
                    all_files.push(pattern);
                } else if pattern.is_dir() {
                    // Directory - analyze all source files
                    use walkdir::WalkDir;
                    for entry in WalkDir::new(&pattern)
                        .follow_links(true)
                        .into_iter()
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_type().is_file())
                    {
                        let path = entry.path();
                        if is_source_file(path) {
                            all_files.push(path.to_path_buf());
                        }
                    }
                } else {
                    println!("⚠️  Skipping invalid path: {}", pattern.display());
                }
            }

            if all_files.is_empty() {
                println!("❌ No files found to analyze");
                return Ok(());
            }

            println!("✓ Found {} files to analyze", all_files.len());
            println!("📦 Creating batches of {} files each...", batch_size);

            // Create batches
            let batches: Vec<_> = all_files.chunks(batch_size).collect();
            println!("✓ Created {} batch(es)\n", batches.len());

            let mut all_results = Vec::new();
            let start_time = std::time::Instant::now();

            for (batch_idx, batch) in batches.iter().enumerate() {
                println!(
                    "📊 Processing batch {}/{} ({} files)...",
                    batch_idx + 1,
                    batches.len(),
                    batch.len()
                );

                for file_path in batch.iter() {
                    let content = match std::fs::read_to_string(file_path) {
                        Ok(c) => c,
                        Err(e) => {
                            println!("  ⚠️  Skipping {}: {}", file_path.display(), e);
                            continue;
                        }
                    };

                    // Skip very large files
                    if content.len() > 100_000 {
                        println!(
                            "  ⚠️  Skipping {} (too large: {} bytes)",
                            file_path.display(),
                            content.len()
                        );
                        continue;
                    }

                    let result = client
                        .score_file(file_path.to_str().unwrap(), &content)
                        .await?;

                    println!(
                        "  ✓ {} - Score: {:.1}/100",
                        file_path.file_name().unwrap().to_string_lossy(),
                        result.overall_score
                    );

                    all_results.push((file_path.clone(), result));
                }

                println!();
            }

            let elapsed = start_time.elapsed();

            // Summary
            println!("═══════════════════════════════════════════");
            println!("📊 Batch Analysis Summary");
            println!("═══════════════════════════════════════════");
            println!("Files analyzed:       {}", all_results.len());
            println!("Time elapsed:         {:.2}s", elapsed.as_secs_f64());
            println!(
                "Average time/file:    {:.2}s",
                elapsed.as_secs_f64() / all_results.len() as f64
            );

            // Calculate statistics
            let avg_score: f64 = all_results
                .iter()
                .map(|(_, r)| r.overall_score)
                .sum::<f64>()
                / all_results.len() as f64;

            let avg_security: f64 = all_results
                .iter()
                .map(|(_, r)| r.security_score)
                .sum::<f64>()
                / all_results.len() as f64;

            println!("\n📈 Score Statistics:");
            println!("  Average overall:    {:.1}/100", avg_score);
            println!("  Average security:   {:.1}/100", avg_security);

            // Find files with issues
            let files_with_issues: Vec<_> = all_results
                .iter()
                .filter(|(_, r)| !r.issues.is_empty())
                .collect();

            if !files_with_issues.is_empty() {
                println!("\n⚠️  Files with issues: {}", files_with_issues.len());
                for (path, result) in files_with_issues.iter().take(5) {
                    println!(
                        "  • {} ({} issues)",
                        path.file_name().unwrap().to_string_lossy(),
                        result.issues.len()
                    );
                }
                if files_with_issues.len() > 5 {
                    println!("  ... and {} more", files_with_issues.len() - 5);
                }
            }

            // Find low-scoring files
            let low_scores: Vec<_> = all_results
                .iter()
                .filter(|(_, r)| r.overall_score < 70.0)
                .collect();

            if !low_scores.is_empty() {
                println!(
                    "\n🔴 Files needing attention (score < 70): {}",
                    low_scores.len()
                );
                for (path, result) in low_scores.iter().take(5) {
                    println!(
                        "  • {} - {:.1}/100",
                        path.file_name().unwrap().to_string_lossy(),
                        result.overall_score
                    );
                }
                if low_scores.len() > 5 {
                    println!("  ... and {} more", low_scores.len() - 5);
                }
            }

            // Show cost
            let cost = client.get_cost_last_n_days(1).await?;
            println!("\n💰 Cost today: ${:.4}", cost);

            // Save to file if requested
            if let Some(output_path) = output {
                use std::io::Write;
                let mut output_file = std::fs::File::create(&output_path)?;

                writeln!(output_file, "# Batch Analysis Report")?;
                writeln!(output_file, "\nGenerated: {}", chrono::Utc::now())?;
                writeln!(output_file, "Files analyzed: {}", all_results.len())?;
                writeln!(output_file, "Average score: {:.1}/100\n", avg_score)?;

                writeln!(output_file, "## Results\n")?;
                for (path, result) in &all_results {
                    writeln!(output_file, "### {}", path.display())?;
                    writeln!(output_file, "- Overall: {:.1}/100", result.overall_score)?;
                    writeln!(output_file, "- Security: {:.1}/100", result.security_score)?;
                    writeln!(output_file, "- Quality: {:.1}/100", result.quality_score)?;

                    if !result.issues.is_empty() {
                        writeln!(output_file, "\n**Issues:**")?;
                        for issue in &result.issues {
                            writeln!(output_file, "- {}", issue)?;
                        }
                    }

                    if !result.suggestions.is_empty() {
                        writeln!(output_file, "\n**Suggestions:**")?;
                        for suggestion in &result.suggestions {
                            writeln!(output_file, "- {}", suggestion)?;
                        }
                    }
                    writeln!(output_file)?;
                }

                println!("\n📄 Report saved to: {}", output_path.display());
            }
        }
    }

    Ok(())
}

// Helper function to check if file is a source file
fn is_source_file(path: &std::path::Path) -> bool {
    if let Some(ext) = path.extension() {
        matches!(
            ext.to_str().unwrap_or(""),
            "rs" | "py" | "js" | "ts" | "java" | "kt" | "go" | "c" | "cpp" | "h" | "hpp"
        )
    } else {
        false
    }
}

async fn handle_next(db: Database) -> anyhow::Result<()> {
    // Simple next action recommendation based on inbox
    let inbox_notes = db.list_notes(Some(NoteStatus::Inbox), None, None).await?;
    let active_notes = db.list_notes(Some(NoteStatus::Active), None, None).await?;

    println!("📋 What should you work on next?\n");

    if !active_notes.is_empty() {
        println!("🔥 Active work ({} items):", active_notes.len());
        for note in active_notes.iter().take(3) {
            println!("  • {} (ID: {})", truncate(&note.content, 60), note.id);
            if !note.tags.is_empty() {
                println!("    Tags: {}", note.tags.join(", "));
            }
        }
        println!();
    }

    if !inbox_notes.is_empty() {
        println!("📥 Inbox to process ({} items):", inbox_notes.len());
        for note in inbox_notes.iter().take(3) {
            println!("  • {} (ID: {})", truncate(&note.content, 60), note.id);
            if !note.tags.is_empty() {
                println!("    Tags: {}", note.tags.join(", "));
            }
        }
        println!();
    }

    if active_notes.is_empty() && inbox_notes.is_empty() {
        println!("✨ No pending items! Time to capture new ideas.");
        println!("\nAdd a note with: devflow note add \"your idea\"");
    } else if !inbox_notes.is_empty() {
        println!("💡 Recommendation: Review inbox and mark important items as active");
        println!("   devflow note update <id> --status active");
    }

    Ok(())
}

async fn handle_stats(db: Database) -> anyhow::Result<()> {
    let stats = db.get_stats().await?;
    let tags = db.list_tags().await?;

    println!("📊 Rustassistant Statistics\n");
    println!("Notes:");
    println!("  Total: {}", stats.total_notes);
    println!("  Inbox: {}", stats.inbox_notes);
    println!();
    println!("Tags: {}", stats.total_tags);
    if !tags.is_empty() {
        println!("  Top tags:");
        for (tag, count) in tags.iter().take(5) {
            println!("    {} ({})", tag, count);
        }
    }
    println!();
    println!("Repositories: {}", stats.total_repositories);

    Ok(())
}

async fn handle_test_command(db: Database, cmd: TestCommands) -> anyhow::Result<()> {
    use rustassistant::test_generator::TestGenerator;

    let generator = TestGenerator::new(db).await?;

    match cmd {
        TestCommands::Generate {
            file,
            function,
            output,
            markdown,
        } => {
            if let Some(func_name) = function {
                println!("🧪 Generating tests for function '{}'...", func_name);
                let tests = generator
                    .generate_tests_for_function(&file, &func_name)
                    .await?;

                println!("\n✅ Generated {} test case(s)", tests.test_cases.len());
                println!(
                    "   Coverage improvement: {:.1}%",
                    tests.coverage_improvement
                );

                let formatted = if markdown {
                    tests.format_as_markdown()
                } else {
                    tests.format_as_code()
                };

                if let Some(output_path) = output {
                    std::fs::write(&output_path, &formatted)?;
                    println!("\n📄 Tests saved to: {}", output_path.display());
                } else {
                    println!("\n{}", formatted);
                }
            } else {
                println!("🧪 Generating tests for file '{}'...", file.display());
                let tests = generator.generate_tests_for_file(&file).await?;

                println!("\n✅ Generated {} test case(s)", tests.test_cases.len());
                println!(
                    "   Coverage improvement: {:.1}%",
                    tests.coverage_improvement
                );
                println!("   Framework: {}", tests.framework);

                let formatted = if markdown {
                    tests.format_as_markdown()
                } else {
                    tests.format_as_code()
                };

                if let Some(output_path) = output {
                    std::fs::write(&output_path, &formatted)?;
                    println!("\n📄 Tests saved to: {}", output_path.display());
                } else {
                    println!("\n{}", formatted);
                }
            }
        }

        TestCommands::Gaps { path, output } => {
            println!("🔍 Analyzing test gaps in {}...", path.display());

            let analyses = generator.analyze_test_gaps(&path).await?;

            if analyses.is_empty() {
                println!("\n✅ No source files found or all have good coverage");
                return Ok(());
            }

            println!("\n📊 Gap Analysis Complete!");
            println!("   Files analyzed: {}", analyses.len());

            let total_coverage: f64 =
                analyses.iter().map(|a| a.estimated_coverage).sum::<f64>() / analyses.len() as f64;

            println!("   Average coverage: {:.1}%", total_coverage);

            let total_untested: usize = analyses.iter().map(|a| a.untested_functions.len()).sum();

            println!("   Untested functions: {}", total_untested);

            let mut full_report = String::new();
            for analysis in &analyses {
                full_report.push_str(&analysis.format_as_markdown());
                full_report.push_str("\n---\n\n");
            }

            if let Some(output_path) = output {
                std::fs::write(&output_path, &full_report)?;
                println!("\n📄 Gap analysis saved to: {}", output_path.display());
            } else {
                println!("\n{}", full_report);
            }
        }

        TestCommands::Fixtures { file, output } => {
            println!("🔧 Generating test fixtures for {}...", file.display());

            let fixtures = generator.generate_fixtures(&file).await?;

            println!("\n✅ Generated {} fixture(s)", fixtures.len());

            let mut formatted = String::new();
            formatted.push_str(&format!("// Test fixtures for: {}\n\n", file.display()));

            for fixture in &fixtures {
                formatted.push_str(&format!(
                    "// Fixture: {} ({})\n",
                    fixture.name, fixture.fixture_type
                ));
                formatted.push_str(&format!("// Sample: {}\n", fixture.sample_data));
                formatted.push_str(&format!("{}\n\n", fixture.creation_code));
            }

            if let Some(output_path) = output {
                std::fs::write(&output_path, &formatted)?;
                println!("\n📄 Fixtures saved to: {}", output_path.display());
            } else {
                println!("\n{}", formatted);
            }
        }
    }

    Ok(())
}

async fn handle_refactor_command(db: Database, cmd: RefactorCommands) -> anyhow::Result<()> {
    use rustassistant::refactor_assistant::RefactorAssistant;

    let assistant = RefactorAssistant::new(db).await?;

    match cmd {
        RefactorCommands::Analyze { path, output } => {
            if path.is_file() {
                println!(
                    "🔍 Analyzing {} for refactoring opportunities...",
                    path.display()
                );
                let analysis = assistant.analyze_file(&path).await?;

                println!("\n📊 Analysis Complete!");
                println!("   Code Smells: {}", analysis.code_smells.len());
                println!("   Suggestions: {}", analysis.suggestions.len());
                println!("   Complexity: {:.1}/100", analysis.complexity_score);
                println!(
                    "   Maintainability: {:.1}/100",
                    analysis.maintainability_score
                );
                println!("   Estimated Effort: {:?}", analysis.estimated_effort);

                let formatted = analysis.format_markdown();

                if let Some(output_path) = output {
                    std::fs::write(&output_path, &formatted)?;
                    println!("\n📄 Analysis saved to: {}", output_path.display());
                } else {
                    println!("\n{}", formatted);
                }
            } else {
                println!(
                    "🔍 Analyzing directory {} for refactoring opportunities...",
                    path.display()
                );
                let analyses = assistant.analyze_directory(&path).await?;

                println!("\n📊 Analysis Complete!");
                println!("   Files analyzed: {}", analyses.len());

                let avg_complexity: f64 = analyses.iter().map(|a| a.complexity_score).sum::<f64>()
                    / analyses.len().max(1) as f64;
                let avg_maintainability: f64 = analyses
                    .iter()
                    .map(|a| a.maintainability_score)
                    .sum::<f64>()
                    / analyses.len().max(1) as f64;
                let total_smells: usize = analyses.iter().map(|a| a.code_smells.len()).sum();

                println!("   Average Complexity: {:.1}/100", avg_complexity);
                println!("   Average Maintainability: {:.1}/100", avg_maintainability);
                println!("   Total Code Smells: {}", total_smells);

                let mut full_report = String::new();
                full_report.push_str("# Refactoring Analysis Report\n\n");
                full_report.push_str(&format!("**Files Analyzed:** {}\n", analyses.len()));
                full_report.push_str(&format!(
                    "**Average Complexity:** {:.1}/100\n",
                    avg_complexity
                ));
                full_report.push_str(&format!(
                    "**Average Maintainability:** {:.1}/100\n",
                    avg_maintainability
                ));
                full_report.push_str(&format!("**Total Code Smells:** {}\n\n", total_smells));
                full_report.push_str("---\n\n");

                for analysis in &analyses {
                    full_report.push_str(&analysis.format_markdown());
                    full_report.push_str("\n---\n\n");
                }

                if let Some(output_path) = output {
                    std::fs::write(&output_path, &full_report)?;
                    println!("\n📄 Analysis saved to: {}", output_path.display());
                } else {
                    println!("\n{}", full_report);
                }
            }
        }

        RefactorCommands::Suggest {
            file,
            start_line,
            end_line,
            output,
        } => {
            println!(
                "💡 Generating refactoring suggestions for {}...",
                file.display()
            );

            if start_line.is_some() || end_line.is_some() {
                println!("   Lines: {:?} - {:?}", start_line, end_line);
            }

            let analysis = assistant.analyze_file(&file).await?;

            println!(
                "\n✅ Generated {} suggestion(s)",
                analysis.suggestions.len()
            );
            println!("   Code Smells: {}", analysis.code_smells.len());

            let formatted = analysis.format_markdown();

            if let Some(output_path) = output {
                std::fs::write(&output_path, &formatted)?;
                println!("\n📄 Suggestions saved to: {}", output_path.display());
            } else {
                println!("\n{}", formatted);
            }
        }

        RefactorCommands::Plan { path, goal, output } => {
            println!("📋 Generating refactoring plan for {}...", path.display());
            println!("   Goal: {}", goal);

            let plan = assistant.generate_plan(&path, &goal).await?;

            println!("\n✅ Plan Generated!");
            println!("   Title: {}", plan.title);
            println!("   Steps: {}", plan.steps.len());
            println!("   Total Effort: {:?}", plan.total_effort);
            println!("   Risks: {}", plan.risks.len());

            let formatted = plan.format_markdown();

            if let Some(output_path) = output {
                std::fs::write(&output_path, &formatted)?;
                println!("\n📄 Plan saved to: {}", output_path.display());
            } else {
                println!("\n{}", formatted);
            }
        }
    }

    Ok(())
}

async fn handle_costs(db: Database) -> anyhow::Result<()> {
    let total = db.get_total_llm_cost().await?;
    let last_24h = db.get_llm_cost_by_period(1).await?;
    let last_7d = db.get_llm_cost_by_period(7).await?;
    let last_30d = db.get_llm_cost_by_period(30).await?;
    let by_model = db.get_cost_by_model().await?;
    let recent_ops = db.get_recent_llm_operations(10).await?;

    println!("💰 LLM Cost Statistics\n");
    println!("Total Costs:");
    println!("  All time:     ${:.4}", total);
    println!("  Last 24h:     ${:.4}", last_24h);
    println!("  Last 7 days:  ${:.4}", last_7d);
    println!("  Last 30 days: ${:.4}", last_30d);
    println!();

    if !by_model.is_empty() {
        println!("By Model:");
        for (model, cost, tokens) in by_model {
            println!("  {} - ${:.4} ({} tokens)", model, cost, tokens);
        }
        println!();
    }

    if !recent_ops.is_empty() {
        println!("Recent Operations:");
        for op in recent_ops {
            println!(
                "  {} - {} ({} tokens) - ${:.4} - {}",
                op.created_at.format("%Y-%m-%d %H:%M"),
                op.operation,
                op.total_tokens,
                op.estimated_cost_usd,
                op.model
            );
        }
    } else {
        println!("No LLM operations recorded yet.");
        println!("\nUse 'rustassistant analyze' commands to start using Grok AI.");
    }

    Ok(())
}

async fn handle_cache_command(cmd: CacheCommands) -> anyhow::Result<()> {
    use rustassistant::response_cache::ResponseCache;

    let cache = ResponseCache::new("data/rustassistant_cache.db").await?;

    match cmd {
        CacheCommands::Stats => {
            let stats = cache.get_stats().await?;

            println!("📦 Response Cache Statistics\n");
            println!("Total Entries: {}", stats.total_entries);
            println!("Total Hits: {}", stats.total_hits);
            println!("Hit Rate: {:.2} hits per entry", stats.hit_rate);
            println!(
                "Cache Size: {:.2} MB",
                stats.total_size_bytes as f64 / (1024.0 * 1024.0)
            );
            println!();

            if let Some(oldest) = stats.oldest_entry {
                println!("Oldest Entry: {}", oldest.format("%Y-%m-%d %H:%M:%S"));
            }
            if let Some(newest) = stats.newest_entry {
                println!("Newest Entry: {}", newest.format("%Y-%m-%d %H:%M:%S"));
            }
            println!();

            // Calculate savings (assuming $0.40 per query average)
            let savings = cache.calculate_savings(0.40).await?;
            println!("💰 Estimated Savings: ${:.2}", savings);
            println!(
                "   (Based on {} cached hits at $0.40/query)",
                stats.total_hits
            );
        }

        CacheCommands::Clear => {
            let count = cache.clear_all().await?;
            println!("✓ Cleared {} cache entries", count);
        }

        CacheCommands::Prune => {
            let count = cache.clear_expired().await?;
            println!("✓ Removed {} expired cache entries", count);
        }

        CacheCommands::Hot { limit } => {
            let entries = cache.get_hot_entries(limit).await?;

            if entries.is_empty() {
                println!("No cache entries found.");
            } else {
                println!("🔥 Most Frequently Accessed Cache Entries:\n");
                for (i, entry) in entries.iter().enumerate() {
                    println!("{}. {} - {} hits", i + 1, entry.operation, entry.hit_count);
                    println!("   Created: {}", entry.created_at.format("%Y-%m-%d %H:%M"));
                    println!(
                        "   Last accessed: {}",
                        entry.last_accessed.format("%Y-%m-%d %H:%M")
                    );
                    println!();
                }
            }
        }
    }

    Ok(())
}

async fn handle_review_command(db: Database, cmd: ReviewCommands) -> anyhow::Result<()> {
    use rustassistant::code_review::CodeReviewer;

    let reviewer = CodeReviewer::new(db).await?;

    match cmd {
        ReviewCommands::Diff {
            path,
            base,
            output,
            github,
        } => {
            println!("🔍 Reviewing changes in {}...", path.display());
            if let Some(ref branch) = base {
                println!("   Base branch: {}", branch);
            }

            let review = reviewer.review_diff(&path, base.as_deref()).await?;

            if review.files.is_empty() {
                println!("\n✅ No changes to review");
                return Ok(());
            }

            println!("\n📊 Review Complete!");
            println!("   Files: {}", review.stats.total_files);
            println!("   Issues: {}", review.stats.total_issues);
            println!("   Quality: {:.1}/100", review.stats.avg_quality);
            println!("   Security: {:.1}/100", review.stats.avg_security);

            let formatted = if github {
                review.format_github_comment()
            } else {
                review.format_markdown()
            };

            if let Some(output_path) = output {
                std::fs::write(&output_path, &formatted)?;
                println!("\n📄 Report saved to: {}", output_path.display());
            } else {
                println!("\n{}", formatted);
            }
        }

        ReviewCommands::Files {
            files,
            output,
            github,
        } => {
            println!("🔍 Reviewing {} files...", files.len());

            let review = reviewer.review_files(files).await?;

            println!("\n📊 Review Complete!");
            println!("   Files: {}", review.stats.total_files);
            println!("   Issues: {}", review.stats.total_issues);
            println!("   Quality: {:.1}/100", review.stats.avg_quality);
            println!("   Security: {:.1}/100", review.stats.avg_security);

            let formatted = if github {
                review.format_github_comment()
            } else {
                review.format_markdown()
            };

            if let Some(output_path) = output {
                std::fs::write(&output_path, &formatted)?;
                println!("\n📄 Report saved to: {}", output_path.display());
            } else {
                println!("\n{}", formatted);
            }
        }

        ReviewCommands::Pr { path, base, output } => {
            println!("🔍 Generating PR description for {}...", path.display());
            println!("   Base branch: {}", base);

            let review = reviewer.review_diff(&path, Some(&base)).await?;

            if review.files.is_empty() {
                println!("\n✅ No changes to review");
                return Ok(());
            }

            // Generate PR description
            let mut pr_description = String::new();
            pr_description.push_str("## Changes\n\n");
            pr_description.push_str(&format!(
                "- **Files Changed:** {}\n",
                review.stats.total_files
            ));
            pr_description.push_str(&format!(
                "- **Lines Changed:** {}\n",
                review.stats.total_lines_changed
            ));
            pr_description.push_str("\n");

            pr_description.push_str("## Code Quality\n\n");
            pr_description.push_str(&review.summary);
            pr_description.push_str("\n");

            if review.stats.total_issues > 0 {
                pr_description.push_str("## Issues to Address\n\n");
                for file in &review.files {
                    if !file.issues.is_empty() {
                        pr_description.push_str(&format!("### {}\n\n", file.path));
                        for issue in &file.issues {
                            pr_description.push_str(&format!(
                                "- **{}:** {}\n",
                                issue.severity, issue.description
                            ));
                        }
                        pr_description.push_str("\n");
                    }
                }
            }

            if let Some(output_path) = output {
                std::fs::write(&output_path, &pr_description)?;
                println!("\n📄 PR description saved to: {}", output_path.display());
            } else {
                println!("\n{}", pr_description);
            }
        }
    }

    Ok(())
}

async fn handle_template_command(cmd: TemplateCommands) -> anyhow::Result<()> {
    use rustassistant::query_templates::TemplateRegistry;

    let registry = TemplateRegistry::new();

    match cmd {
        TemplateCommands::List { search } => {
            let templates = if let Some(keyword) = search {
                registry.search(&keyword)
            } else {
                registry.list()
            };

            if templates.is_empty() {
                println!("No templates found.");
            } else {
                println!("📋 Available Query Templates ({}):\n", templates.len());
                for template in templates {
                    println!("  {} - {}", template.name, template.description);
                    println!(
                        "    Tokens: ~{}, Cache TTL: {}h, Operation: {}",
                        template.estimated_tokens, template.cache_ttl, template.operation
                    );
                    println!();
                }
            }
        }

        TemplateCommands::Show { name } => {
            let template = registry.get(&name)?;

            println!("📋 Template: {}\n", template.name);
            println!("Description: {}", template.description);
            println!("Operation: {}", template.operation);
            println!("Estimated Tokens: {}", template.estimated_tokens);
            println!("Cache TTL: {} hours", template.cache_ttl);
            println!();

            println!("Required Variables:");
            for var in &template.required_vars {
                println!("  - {}", var);
            }

            if !template.optional_vars.is_empty() {
                println!("\nOptional Variables:");
                for (key, default) in &template.optional_vars {
                    println!("  - {} (default: {})", key, default);
                }
            }

            println!("\nPattern:");
            println!("{}", template.pattern);
            println!();

            println!("Example Usage:");
            let example_vars = template
                .required_vars
                .iter()
                .map(|v| format!("--var {}=value", v))
                .collect::<Vec<_>>()
                .join(" ");
            println!("  devflow template use {} {}", template.name, example_vars);
        }

        TemplateCommands::Use { name, var, execute } => {
            let template = registry.get(&name)?;

            // Parse variables
            let vars: Vec<(&str, String)> = var
                .iter()
                .filter_map(|v| {
                    let parts: Vec<&str> = v.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        Some((parts[0], parts[1].to_string()))
                    } else {
                        None
                    }
                })
                .collect();

            let var_refs: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (*k, v.as_str())).collect();

            let rendered = template.render(&var_refs)?;

            println!("📝 Generated Query:\n");
            println!("{}", rendered);
            println!();

            if execute {
                println!("🤖 Executing query...");
                println!("(Integration with Grok client would go here)");
                println!("Estimated cost: ${:.4}", template.estimated_cost(0.005));
            } else {
                println!("ℹ️  Use --execute to run this query with Grok");
                println!("Estimated cost: ${:.4}", template.estimated_cost(0.005));
            }
        }
    }

    Ok(())
}

// Helper functions

fn parse_status(s: &str) -> anyhow::Result<NoteStatus> {
    NoteStatus::from_str(s).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid status '{}'. Valid options: inbox, active, processed, archived",
            s
        )
    })
}

fn print_note(note: &Note) {
    println!("─────────────────────────────────────────");
    println!("ID: {}", note.id);
    println!("Status: {}", note.status);
    if !note.tags.is_empty() {
        println!("Tags: {}", note.tags.join(", "));
    }
    println!("Created: {}", note.created_at.format("%Y-%m-%d %H:%M:%S"));
    if note.created_at != note.updated_at {
        println!("Updated: {}", note.updated_at.format("%Y-%m-%d %H:%M:%S"));
    }
    println!("─────────────────────────────────────────");
    println!("{}", note.content);
    println!("─────────────────────────────────────────");
}

fn print_note_summary(note: &Note) {
    let status_emoji = match note.status {
        NoteStatus::Inbox => "📥",
        NoteStatus::Active => "🔥",
        NoteStatus::Processed => "✅",
        NoteStatus::Archived => "📦",
    };

    println!(
        "{} [{}] {} (ID: {})",
        status_emoji,
        note.status,
        truncate(&note.content, 70),
        note.id
    );

    if !note.tags.is_empty() {
        println!("   Tags: {}", note.tags.join(", "));
    }

    println!("   Created: {}", note.created_at.format("%Y-%m-%d %H:%M"));
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut truncated = s.chars().take(max_len - 3).collect::<String>();
        truncated.push_str("...");
        truncated
    }
}
