//! Audit service CLI for CI integration and local usage
//!
//! This CLI tool provides a command-line interface for running audits,
//! scanning for tags, and generating tasks. It's designed to be used in
//! CI/CD pipelines and for local development.

use clap::{Parser, Subcommand};
use rustassistant::formatter::{CodeFormatter, FormatMode, Formatter};
use rustassistant::grok_reasoning::{FileForAnalysis, GrokReasoningClient};
use rustassistant::llm::LlmClient;
use rustassistant::prelude::*;
use rustassistant::scoring::FileScorer;
use rustassistant::tree_state::{FileCategory, TreeStateManager};
use rustassistant::{research, GitManager, TagScanner, TaskGenerator, TodoPriority, TodoScanner};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "audit-cli")]
#[command(about = "Code audit CLI tool for static analysis and LLM-powered reviews", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Output format
    #[arg(short, long, value_enum, global = true, default_value = "text")]
    format: OutputFormat,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Json,
    Csv,
}

#[derive(Subcommand)]
#[clap(rename_all = "kebab-case")]
enum Commands {
    /// Perform a full audit of a repository
    Audit {
        /// Repository path or URL
        #[arg(value_name = "REPO")]
        repository: String,

        /// Branch to audit
        #[arg(short, long)]
        branch: Option<String>,

        /// Enable LLM analysis
        #[arg(short, long)]
        llm: bool,

        /// Exclude test files from analysis (tests included by default)
        #[arg(long)]
        exclude_tests: bool,

        /// Output file for report
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Scan for audit tags only
    Tags {
        /// Directory to scan
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Filter by tag type
        #[arg(short, long)]
        tag_type: Option<String>,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Scan for TODO comments in code
    Todo {
        /// Directory to scan
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Filter by priority (high, medium, low)
        #[arg(short, long)]
        priority: Option<String>,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run static analysis only (fast, no LLM)
    Static {
        /// Directory to analyze
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Focus on specific categories
        #[arg(short, long)]
        focus: Vec<String>,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Rate files using LLM analysis
    Rate {
        /// Directory or file to rate
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// LLM provider (xai, google, anthropic/claude)
        #[arg(short, long, default_value = "xai")]
        provider: String,

        /// Batch mode: analyze multiple files together
        #[arg(short, long)]
        batch: bool,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run LLM questionnaire on files
    Question {
        /// Directory or file to analyze
        #[arg(value_name = "PATH")]
        path: PathBuf,

        /// LLM provider (xai, google, anthropic/claude)
        #[arg(short, long, default_value = "xai")]
        provider: String,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate tasks from audit findings
    Tasks {
        /// Directory to scan
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Output format override
        #[arg(short = 'f', long)]
        format: Option<OutputFormat>,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Clone a repository for audit
    Clone {
        /// Repository URL
        #[arg(value_name = "URL")]
        url: String,

        /// Target directory name
        #[arg(short, long)]
        name: Option<String>,

        /// Branch to checkout
        #[arg(short, long)]
        branch: Option<String>,
    },

    /// Show audit statistics for a path
    Stats {
        /// Directory to analyze
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,
    },

    /// Analyze research material and generate implementation plans
    Research {
        /// Path to research file (.md or .txt)
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Generate actionable tasks from the research
        #[arg(short = 't', long)]
        generate_tasks: bool,

        /// Output directory for breakdown
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate directory tree with audit tags and issues
    Tree {
        /// Path to project root
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Maximum depth to display
        #[arg(short, long, default_value = "5")]
        depth: usize,

        /// Include audit tags in tree
        #[arg(short, long)]
        with_tags: bool,

        /// Output file for tree
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate architecture visualization diagrams
    Visualize {
        /// Path to project root
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Type of diagram (neuromorphic, component)
        #[arg(short, long, default_value = "neuromorphic")]
        diagram_type: String,

        /// Component to visualize (for component type)
        #[arg(short, long)]
        component: Option<String>,

        /// Output file for diagram
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Format code automatically (or check formatting)
    Format {
        /// Path to project root
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Check formatting without making changes
        #[arg(short, long)]
        check: bool,

        /// Specific formatters to use (rust, kotlin, prettier, black)
        #[arg(short = 'f', long)]
        formatters: Vec<String>,

        /// Output file for report
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run LLM-powered audit (Regular or Full mode)
    LlmAudit {
        /// Path to project root
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Audit mode (regular or full)
        #[arg(short, long, default_value = "regular")]
        mode: String,

        /// LLM provider (xai, google, anthropic/claude/opus/sonnet)
        /// Use 'opus' for Claude Opus 4.5 - best for JANUS whitepaper conformity
        #[arg(short, long, default_value = "xai")]
        provider: String,

        /// Focus areas (comma-separated)
        #[arg(short = 'f', long)]
        focus: Vec<String>,

        /// Output file for report
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Track tree state changes between runs (CI/CD integration)
    TreeState {
        /// Path to project root
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Show diff from previous state
        #[arg(short, long)]
        diff: bool,

        /// Generate CI summary report
        #[arg(long)]
        ci_summary: bool,

        /// Output file for state/diff
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run Grok 4.1 reasoning audit with batching and 2M context optimization
    GrokAudit {
        /// Path to project root
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Category to audit (audit, janus, clients, execution, all)
        #[arg(short, long, default_value = "all")]
        category: String,

        /// Maximum turns for agentic requests (cost control)
        #[arg(long, default_value = "5")]
        max_turns: usize,

        /// Maximum batch token size
        #[arg(long, default_value = "100000")]
        max_batch_tokens: usize,

        /// Only analyze changed files (requires previous tree state)
        #[arg(long)]
        changed_only: bool,

        /// Exclude documentation files from analysis
        #[arg(long)]
        exclude_docs: bool,

        /// Exclude config files from analysis
        #[arg(long)]
        exclude_config: bool,

        /// Enable code execution tool
        #[arg(long)]
        enable_code_exec: bool,

        /// Output file for results
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run JANUS whitepaper conformity audit with Claude Opus 4.5
    /// Uses Anthropic's most capable model for deep analysis of JANUS theory implementation
    JanusAudit {
        /// Path to project root
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// LLM provider (anthropic/claude/opus for Claude Opus 4.5, sonnet for Claude Sonnet 4)
        /// Default is Claude Opus 4.5 - best for whitepaper conformity verification
        #[arg(short, long, default_value = "opus")]
        provider: String,

        /// Focus areas for audit (neuromorphic, gaf, ltn, memory, compliance, all)
        #[arg(short, long, default_value = "all")]
        focus: String,

        /// Compare against specific whitepaper sections (comma-separated, e.g. "3.1,4.2,5")
        #[arg(long)]
        sections: Option<String>,

        /// Enable deep mathematical verification (GAF transforms, Lukasiewicz logic)
        #[arg(long)]
        verify_math: bool,

        /// Check brain-region component mappings
        #[arg(long)]
        verify_mappings: bool,

        /// Generate compliance report for regulatory review
        #[arg(long)]
        compliance_report: bool,

        /// Output file for results
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .init();

    // Load config
    let config = Config::load()?;

    match cli.command {
        Commands::Audit {
            repository,
            branch,
            llm,
            exclude_tests,
            output,
        } => {
            run_audit(
                &repository,
                branch,
                llm,
                !exclude_tests,
                output,
                cli.format,
                &config,
            )
            .await?;
        }
        Commands::Tags {
            path,
            tag_type,
            output,
        } => {
            run_tags_scan(&path, tag_type, output, cli.format)?;
        }
        Commands::Todo {
            path,
            priority,
            output,
        } => {
            run_todo_scan(&path, priority, output, cli.format)?;
        }
        Commands::Static {
            path,
            focus,
            output,
        } => {
            run_static_analysis(&path, focus, output, cli.format, &config).await?;
        }
        Commands::Rate {
            path,
            provider,
            batch,
            output,
        } => {
            run_llm_rating(&path, &provider, batch, output, cli.format, &config).await?;
        }
        Commands::Question {
            path,
            provider,
            output,
        } => {
            run_llm_questionnaire(&path, &provider, output, cli.format, &config).await?;
        }
        Commands::Tasks {
            path,
            format,
            output,
        } => {
            let fmt = format.unwrap_or(cli.format);
            run_tasks_generation(&path, output, fmt)?;
        }
        Commands::Clone { url, name, branch } => {
            run_clone(&url, name, branch, &config)?;
        }
        Commands::Stats { path } => {
            run_stats(&path)?;
        }
        Commands::Research {
            file,
            generate_tasks,
            output,
        } => {
            run_research_analysis(&file, generate_tasks, output, cli.format, &config).await?;
        }
        Commands::Tree {
            path,
            depth,
            with_tags,
            output,
        } => {
            run_tree_visualization(&path, depth, with_tags, output.as_ref())?;
        }
        Commands::Visualize {
            path,
            diagram_type,
            component,
            output,
        } => {
            run_visualization(&path, &diagram_type, component, output, cli.format)?;
        }
        Commands::Format {
            path,
            check,
            formatters,
            output,
        } => {
            run_format(&path, check, formatters, output, cli.format)?;
        }
        Commands::LlmAudit {
            path,
            mode,
            provider,
            focus,
            output,
        } => {
            run_llm_audit(&path, &mode, &provider, focus, output, &config).await?;
        }
        Commands::TreeState {
            path,
            diff,
            ci_summary,
            output,
        } => {
            run_tree_state(&path, diff, ci_summary, output, cli.format)?;
        }
        Commands::GrokAudit {
            path,
            category,
            max_turns,
            max_batch_tokens,
            changed_only,
            exclude_docs,
            exclude_config,
            enable_code_exec,
            output,
        } => {
            run_grok_audit(
                &path,
                &category,
                max_turns,
                max_batch_tokens,
                changed_only,
                exclude_docs,
                exclude_config,
                enable_code_exec,
                output,
                cli.format,
                &config,
            )
            .await?;
        }
        Commands::JanusAudit {
            path,
            provider,
            focus,
            sections,
            verify_math,
            verify_mappings,
            compliance_report,
            output,
        } => {
            run_janus_audit(
                &path,
                &provider,
                &focus,
                sections,
                verify_math,
                verify_mappings,
                compliance_report,
                output,
                cli.format,
                &config,
            )
            .await?;
        }
    }

    Ok(())
}

/// Run a full audit
async fn run_audit(
    repository: &str,
    branch: Option<String>,
    enable_llm: bool,
    include_tests: bool,
    output: Option<PathBuf>,
    format: OutputFormat,
    config: &Config,
) -> Result<()> {
    info!("Starting audit for: {}", repository);

    // Setup repository
    let repo_path = if repository.starts_with("http") || repository.starts_with("git@") {
        let git_manager =
            GitManager::new(config.git.workspace_dir.clone(), config.git.shallow_clone)?;
        git_manager.clone_repo(repository, None)?
    } else {
        PathBuf::from(repository)
    };

    // Create scanner
    let scanner = Scanner::new(
        repo_path.clone(),
        config.scanner.max_file_size,
        include_tests,
    )?;

    // Build request
    let request = AuditRequest {
        repository: repository.to_string(),
        branch: branch.clone(),
        enable_llm,
        focus: vec![],
        include_tests,
    };

    // Perform scan
    let report = scanner.scan(&request)?;

    // Output results
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&report)?;
            if let Some(path) = output {
                std::fs::write(path, json)?;
            } else {
                println!("{}", json);
            }
        }
        OutputFormat::Text => {
            print_audit_report(&report);
            if let Some(path) = output {
                let json = serde_json::to_string_pretty(&report)?;
                std::fs::write(path, json)?;
            }
        }
        OutputFormat::Csv => {
            let csv = generate_audit_csv(&report)?;
            if let Some(path) = output {
                std::fs::write(path, csv)?;
            } else {
                println!("{}", csv);
            }
        }
    }

    Ok(())
}

/// Run tags scan
fn run_tags_scan(
    path: &Path,
    tag_type_filter: Option<String>,
    output: Option<PathBuf>,
    format: OutputFormat,
) -> Result<()> {
    info!("Scanning for tags in: {}", path.display());

    let scanner = TagScanner::new()?;
    let mut tags = scanner.scan_directory(path)?;

    // Filter by tag type if specified
    if let Some(filter) = tag_type_filter {
        tags.retain(|t| format!("{:?}", t.tag_type).to_lowercase() == filter.to_lowercase());
    }

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&tags)?;
            if let Some(path) = output {
                std::fs::write(path, json)?;
            } else {
                println!("{}", json);
            }
        }
        OutputFormat::Text => {
            print_tags(&tags);
            if let Some(path) = output {
                let json = serde_json::to_string_pretty(&tags)?;
                std::fs::write(path, json)?;
            }
        }
        OutputFormat::Csv => {
            let csv = generate_tags_csv(&tags)?;
            if let Some(path) = output {
                std::fs::write(path, csv)?;
            } else {
                println!("{}", csv);
            }
        }
    }

    Ok(())
}

/// Run static analysis
async fn run_static_analysis(
    path: &Path,
    _focus: Vec<String>,
    output: Option<PathBuf>,
    format: OutputFormat,
    _config: &Config,
) -> Result<()> {
    info!("Running static analysis on: {}", path.display());

    let scanner = Scanner::new(path.to_path_buf(), _config.scanner.max_file_size, false)?;

    let request = AuditRequest {
        repository: path.to_string_lossy().to_string(),
        branch: None,
        enable_llm: false,
        focus: vec![],
        include_tests: false,
    };

    let report = scanner.scan(&request)?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&report)?;
            if let Some(path) = output {
                std::fs::write(path, json)?;
            } else {
                println!("{}", json);
            }
        }
        OutputFormat::Text => {
            print_static_analysis(&report);
            if let Some(path) = output {
                let json = serde_json::to_string_pretty(&report)?;
                std::fs::write(path, json)?;
            }
        }
        OutputFormat::Csv => {
            let csv = generate_issues_csv(&report)?;
            if let Some(path) = output {
                std::fs::write(path, csv)?;
            } else {
                println!("{}", csv);
            }
        }
    }

    Ok(())
}

/// Run tasks generation
fn run_tasks_generation(path: &Path, output: Option<PathBuf>, format: OutputFormat) -> Result<()> {
    info!("Generating tasks from: {}", path.display());

    // Scan for tags
    let tag_scanner = TagScanner::new()?;
    let tags = tag_scanner.scan_directory(path)?;

    // Run static analysis to get issues
    let scanner = Scanner::new(path.to_path_buf(), 10_000_000, false)?;
    let request = AuditRequest {
        repository: path.to_string_lossy().to_string(),
        branch: None,
        enable_llm: false,
        focus: vec![],
        include_tests: false,
    };
    let report = scanner.scan(&request)?;

    // Generate tasks from both tags and issues
    let mut generator = TaskGenerator::new();
    generator.generate_from_tags(&tags)?;
    generator.generate_from_analyses(&report.files)?;
    let tasks = generator.tasks();

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&tasks)?;
            if let Some(path) = output {
                std::fs::write(path, json)?;
            } else {
                println!("{}", json);
            }
        }
        OutputFormat::Text => {
            print_tasks(tasks);
            if let Some(path) = output {
                let json = serde_json::to_string_pretty(&tasks)?;
                std::fs::write(path, json)?;
            }
        }
        OutputFormat::Csv => {
            let csv = generator.to_csv()?;
            if let Some(path) = output {
                std::fs::write(path, csv)?;
            } else {
                println!("{}", csv);
            }
        }
    }

    Ok(())
}

/// Clone a repository
fn run_clone(
    url: &str,
    name: Option<String>,
    branch: Option<String>,
    _config: &Config,
) -> Result<()> {
    info!("Cloning repository: {}", url);

    let git_manager =
        GitManager::new(_config.git.workspace_dir.clone(), _config.git.shallow_clone)?;
    let repo_path = git_manager.clone_repo(url, name.as_deref())?;

    if let Some(branch) = branch {
        git_manager.checkout(&repo_path, &branch)?;
    }

    println!("✓ Repository cloned to: {}", repo_path.display());
    println!("✓ Branch: {}", git_manager.current_branch(&repo_path)?);

    Ok(())
}

/// Show statistics
fn run_stats(path: &Path) -> Result<()> {
    info!("Analyzing statistics for: {}", path.display());

    let scanner = Scanner::new(path.to_path_buf(), 10_000_000, true)?;

    let request = AuditRequest {
        repository: path.to_string_lossy().to_string(),
        branch: None,
        enable_llm: false,
        focus: vec![],
        include_tests: true,
    };

    let report = scanner.scan(&request)?;

    println!("\n📊 Audit Statistics");
    println!("═══════════════════════════════════════");
    println!("Total Files:      {}", report.summary.total_files);
    println!("Total Lines:      {}", report.summary.total_lines);
    println!("Total Issues:     {}", report.summary.total_issues);
    println!("Critical Files:   {}", report.summary.critical_files);
    println!();
    println!("Files by Category:");
    for (category, count) in &report.system_map.files_by_category {
        println!("  {:?}: {}", category, count);
    }
    println!();
    println!("Issues by Severity:");
    for (severity, count) in &report.issues_by_severity {
        println!("  {:?}: {}", severity, count);
    }

    Ok(())
}

// ===== Output Formatters =====

fn print_audit_report(report: &AuditReport) {
    println!("\n📋 Audit Report: {}", report.id);
    println!("═══════════════════════════════════════");
    println!("Repository:       {}", report.repository);
    println!("Branch:           {}", report.branch);
    println!("Created:          {}", report.created_at);
    println!();
    println!("Summary:");
    println!("  Total Files:    {}", report.summary.total_files);
    println!("  Total Lines:    {}", report.summary.total_lines);
    println!("  Total Issues:   {}", report.summary.total_issues);
    println!("  Total Tasks:    {}", report.summary.total_tasks);
    println!("  Critical Files: {}", report.summary.critical_files);
    println!();

    if !report.issues_by_severity.is_empty() {
        println!("Issues by Severity:");
        for (severity, count) in &report.issues_by_severity {
            println!("  {:?}: {}", severity, count);
        }
        println!();
    }

    if report.summary.total_issues > 0 {
        println!("Critical Issues:");
        for file in &report.files {
            for issue in &file.issues {
                if issue.severity == IssueSeverity::Critical {
                    println!(
                        "  ❌ {}:{} - {}",
                        file.path.display(),
                        issue.line,
                        issue.message
                    );
                }
            }
        }
    }
}

fn print_tags(tags: &[AuditTag]) {
    use std::collections::HashMap;

    println!("\n🏷️  Audit Tags Found: {}", tags.len());
    println!("═══════════════════════════════════════\n");

    let scanner = TagScanner::new().unwrap();
    let grouped = scanner.group_by_type(tags);

    // Group by file for statistics
    let mut by_file: HashMap<std::path::PathBuf, Vec<&AuditTag>> = HashMap::new();
    for tag in tags {
        by_file.entry(tag.file.clone()).or_default().push(tag);
    }

    for (tag_type, tag_list) in grouped {
        println!("{:?} Tags ({}):", tag_type, tag_list.len());
        println!("───────────────────────────────────────");

        for tag in tag_list.iter().take(20) {
            // Display with file and line
            println!("  📍 {}:{}", tag.file.display(), tag.line);

            // Show value if present
            if !tag.value.is_empty() {
                println!("     💬 {}", tag.value);
            }

            // Show context preview if available
            if let Some(context) = &tag.context {
                let preview: Vec<&str> = context.lines().take(2).collect();
                for line in preview {
                    let trimmed = line.trim();
                    if !trimmed.is_empty()
                        && !trimmed.starts_with("//")
                        && !trimmed.starts_with('#')
                    {
                        println!("     📝 {}", trimmed);
                        break;
                    }
                }
            }
            println!();
        }

        if tag_list.len() > 20 {
            println!("  ... and {} more\n", tag_list.len() - 20);
        }
    }

    // Show file statistics
    if !by_file.is_empty() {
        println!("\n📁 Tags by File:");
        println!("───────────────────────────────────────");

        let mut file_counts: Vec<_> = by_file.iter().collect();
        file_counts.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        for (file, file_tags) in file_counts.iter().take(10) {
            println!("  • {} ({} tags)", file.display(), file_tags.len());
        }

        if file_counts.len() > 10 {
            println!("  ... and {} more files", file_counts.len() - 10);
        }
    }
}

fn print_static_analysis(report: &AuditReport) {
    println!("\n🔍 Static Analysis Results");
    println!("═══════════════════════════════════════");
    println!("Files Analyzed:   {}", report.summary.total_files);
    println!("Issues Found:     {}", report.summary.total_issues);
    println!("Critical Files:   {}", report.summary.critical_files);
    println!();

    if !report.issues_by_severity.is_empty() {
        println!("Issues by Severity:");
        for (severity, count) in &report.issues_by_severity {
            let icon = match severity {
                IssueSeverity::Critical => "🔴",
                IssueSeverity::High => "🟠",
                IssueSeverity::Medium => "🟡",
                IssueSeverity::Low => "🟢",
                IssueSeverity::Info => "ℹ️",
            };
            println!("  {} {:?}: {}", icon, severity, count);
        }
    }
}

fn print_tasks(tasks: &[Task]) {
    println!("\n✅ Generated Tasks: {}", tasks.len());
    println!("═══════════════════════════════════════");

    for task in tasks.iter().take(20) {
        let icon = match task.priority {
            TaskPriority::Critical => "🔴",
            TaskPriority::High => "🟠",
            TaskPriority::Medium => "🟡",
            TaskPriority::Low => "🟢",
        };
        println!(
            "{} {} [{}] - {}",
            icon,
            task.id,
            task.file.display(),
            task.title
        );
    }

    if tasks.len() > 20 {
        println!("... and {} more tasks", tasks.len() - 20);
    }
}

fn generate_audit_csv(report: &AuditReport) -> Result<String> {
    let mut csv = String::from("File,Category,Priority,Lines,DocBlocks,Issues,SecurityRating\n");

    for file in &report.files {
        let security = file
            .security_rating
            .map(|r| format!("{:?}", r))
            .unwrap_or_else(|| "N/A".to_string());

        csv.push_str(&format!(
            "{},{:?},{:?},{},{},{},{}\n",
            file.path.display(),
            file.category,
            file.priority,
            file.lines,
            file.doc_blocks,
            file.issues.len(),
            security
        ));
    }

    Ok(csv)
}

fn generate_tags_csv(tags: &[AuditTag]) -> Result<String> {
    let mut csv = String::from("File,Line,Type,Value\n");

    for tag in tags {
        csv.push_str(&format!(
            "{},{},{:?},{}\n",
            tag.file.display(),
            tag.line,
            tag.tag_type,
            tag.value.replace(",", ";")
        ));
    }

    Ok(csv)
}

fn generate_issues_csv(report: &AuditReport) -> Result<String> {
    let mut csv = String::from("File,Line,Severity,Category,Message,Suggestion\n");

    for file in &report.files {
        for issue in &file.issues {
            let suggestion = issue
                .suggestion
                .as_ref()
                .map(|s| s.replace(",", ";"))
                .unwrap_or_default();

            csv.push_str(&format!(
                "{},{},{:?},{:?},{},{}\n",
                issue.file.display(),
                issue.line,
                issue.severity,
                issue.category,
                issue.message.replace(",", ";"),
                suggestion
            ));
        }
    }

    Ok(csv)
}

fn run_todo_scan(
    path: &Path,
    priority: Option<String>,
    output: Option<PathBuf>,
    format: OutputFormat,
) -> Result<()> {
    info!("Scanning for TODO comments in: {}", path.display());

    let scanner = TodoScanner::new()?;
    let todos = scanner.scan_directory(path)?;

    // Filter by priority if specified
    let filtered_todos: Vec<_> = if let Some(p) = priority {
        let target_priority = match p.to_lowercase().as_str() {
            "high" => TodoPriority::High,
            "medium" => TodoPriority::Medium,
            "low" => TodoPriority::Low,
            _ => {
                warn!("Invalid priority '{}', showing all", p);
                return print_todos(&todos, &scanner, output, format);
            }
        };
        todos
            .iter()
            .filter(|t| t.priority == target_priority)
            .cloned()
            .collect()
    } else {
        todos
    };

    print_todos(&filtered_todos, &scanner, output, format)
}

fn print_todos(
    todos: &[TodoItem],
    scanner: &TodoScanner,
    output: Option<PathBuf>,
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(todos)?;
            if let Some(path) = output {
                std::fs::write(path, json)?;
            } else {
                println!("{}", json);
            }
        }
        OutputFormat::Text => {
            let summary = scanner.generate_summary(todos);
            println!("\n📝 TODO Items Found: {}", summary.total);
            println!("═══════════════════════════════════════");
            println!("🔴 High Priority:   {}", summary.high_priority);
            println!("🟡 Medium Priority: {}", summary.medium_priority);
            println!("🟢 Low Priority:    {}", summary.low_priority);
            println!("📁 Files with TODOs: {}", summary.files_with_todos);
            println!();

            if !summary.by_category.is_empty() {
                println!("By Category:");
                for (category, count) in &summary.by_category {
                    println!("  {:?}: {}", category, count);
                }
                println!();
            }

            // Group by priority and show items
            let by_priority = scanner.group_by_priority(todos);

            if let Some(high) = by_priority.get(&TodoPriority::High) {
                if !high.is_empty() {
                    println!("🔴 High Priority TODOs:");
                    for (i, todo) in high.iter().take(10).enumerate() {
                        println!(
                            "  {}. [{}:{}] {}",
                            i + 1,
                            todo.file.display(),
                            todo.line,
                            todo.text
                        );
                    }
                    if high.len() > 10 {
                        println!("  ... and {} more", high.len() - 10);
                    }
                    println!();
                }
            }

            if let Some(medium) = by_priority.get(&TodoPriority::Medium) {
                if !medium.is_empty() {
                    println!("🟡 Medium Priority TODOs ({} total):", medium.len());
                    for (i, todo) in medium.iter().take(5).enumerate() {
                        println!(
                            "  {}. [{}:{}] {}",
                            i + 1,
                            todo.file.display(),
                            todo.line,
                            todo.text
                        );
                    }
                    if medium.len() > 5 {
                        println!("  ... and {} more", medium.len() - 5);
                    }
                    println!();
                }
            }

            if let Some(path) = output {
                let json = serde_json::to_string_pretty(todos)?;
                std::fs::write(path, json)?;
            }
        }
        OutputFormat::Csv => {
            let mut csv = String::from("File,Line,Priority,Category,Text\n");
            for todo in todos {
                csv.push_str(&format!(
                    "{},{},{:?},{:?},{}\n",
                    todo.file.display(),
                    todo.line,
                    todo.priority,
                    todo.category,
                    todo.text.replace(",", ";")
                ));
            }
            if let Some(path) = output {
                std::fs::write(path, csv)?;
            } else {
                println!("{}", csv);
            }
        }
    }

    Ok(())
}

async fn run_llm_rating(
    path: &Path,
    provider: &str,
    batch: bool,
    output: Option<PathBuf>,
    format: OutputFormat,
    _config: &Config,
) -> Result<()> {
    info!("Running LLM rating on: {}", path.display());

    let api_key = std::env::var("XAI_API_KEY")
        .or_else(|_| std::env::var("GOOGLE_API_KEY"))
        .map_err(|_| {
            AuditError::other("No API key found. Set XAI_API_KEY or GOOGLE_API_KEY".to_string())
        })?;

    let model = if provider == "google" {
        "gemini-2.0-flash-exp".to_string()
    } else {
        "grok-4-1-fast-reasoning".to_string()
    };

    let llm = LlmClient::new_with_provider(api_key, provider.to_string(), model, 16000, 0.2)?;

    let mut results = Vec::new();

    if path.is_file() {
        // Rate a single file
        let content = fs::read_to_string(path)?;
        let category = Category::from_path(&path.to_string_lossy());
        let result = llm.analyze_file(path, &content, category).await?;
        results.push((path.to_string_lossy().to_string(), result));
    } else if path.is_dir() {
        // Scan directory for files
        let files: Vec<PathBuf> = WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| matches!(ext, "rs" | "py" | "kt" | "ts" | "js"))
                    .unwrap_or(false)
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        if batch {
            // Batch analysis
            let batch_files: Vec<_> = files
                .iter()
                .take(20) // Limit batch size
                .filter_map(|f| {
                    fs::read_to_string(f).ok().map(|content| {
                        let category = Category::from_path(&f.to_string_lossy());
                        (f.to_string_lossy().to_string(), content, category)
                    })
                })
                .collect();

            info!("Analyzing {} files in batch mode", batch_files.len());
            let batch_results = llm.analyze_batch(batch_files).await?;

            for (i, result) in batch_results.iter().enumerate() {
                if i < files.len() {
                    results.push((files[i].to_string_lossy().to_string(), result.clone()));
                }
            }
        } else {
            // Individual file analysis
            for file in files.iter().take(10) {
                // Limit for individual mode
                if let Ok(content) = fs::read_to_string(file) {
                    let category = Category::from_path(&file.to_string_lossy());
                    match llm.analyze_file(file, &content, category).await {
                        Ok(result) => {
                            results.push((file.to_string_lossy().to_string(), result));
                        }
                        Err(e) => {
                            warn!("Failed to analyze {}: {}", file.display(), e);
                        }
                    }
                }
            }
        }
    }

    // Output results
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&results)?;
            if let Some(path) = output {
                std::fs::write(path, json)?;
            } else {
                println!("{}", json);
            }
        }
        OutputFormat::Text => {
            println!("\n🤖 LLM File Ratings");
            println!("═══════════════════════════════════════");
            println!("Files Analyzed: {}\n", results.len());

            for (file_path, result) in &results {
                println!("📄 {}", file_path);
                println!("  Security Rating: {:?}", result.security_rating);
                println!("  Importance: {:?}", result.importance);
                println!("  Summary: {}", result.summary);
                println!("  Issues: {}", result.issues.len());
                for issue in result.issues.iter().take(3) {
                    let line_str = issue.line.map_or("?".to_string(), |l: usize| l.to_string());
                    println!(
                        "    • [{:?}] {} (line {})",
                        issue.severity, issue.description, line_str
                    );
                }
                if result.issues.len() > 3 {
                    println!("    ... and {} more issues", result.issues.len() - 3);
                }
                println!();
            }

            if let Some(path) = output {
                let json = serde_json::to_string_pretty(&results)?;
                std::fs::write(path, json)?;
            }
        }
        OutputFormat::Csv => {
            let mut csv = String::from("File,SecurityRating,Importance,IssueCount,Summary\n");
            for (file_path, result) in &results {
                csv.push_str(&format!(
                    "{},{:?},{:?},{},{}\n",
                    file_path,
                    result.security_rating,
                    result.importance,
                    result.issues.len(),
                    result.summary.replace(",", ";").replace("\n", " ")
                ));
            }
            if let Some(path) = output {
                std::fs::write(path, csv)?;
            } else {
                println!("{}", csv);
            }
        }
    }

    Ok(())
}

async fn run_llm_questionnaire(
    path: &PathBuf,
    provider: &str,
    output: Option<PathBuf>,
    format: OutputFormat,
    _config: &Config,
) -> Result<()> {
    info!("Running LLM questionnaire on: {}", path.display());

    let api_key = std::env::var("XAI_API_KEY")
        .or_else(|_| std::env::var("GOOGLE_API_KEY"))
        .map_err(|_| {
            AuditError::other("No API key found. Set XAI_API_KEY or GOOGLE_API_KEY".to_string())
        })?;

    let model = if provider == "google" {
        "gemini-2.0-flash-exp".to_string()
    } else {
        "grok-4-1-fast-reasoning".to_string()
    };

    let llm = LlmClient::new_with_provider(api_key, provider.to_string(), model, 16000, 0.2)?;

    // Build simplified global context string for questionnaire
    let project_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let global_context = format!(
        "Project: {}\nAnalyzing codebase for quality and completeness",
        project_name
    );

    // Collect files to analyze
    let files: Vec<PathBuf> = WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| matches!(ext, "rs" | "py" | "kt" | "ts" | "js"))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .take(50) // Limit for questionnaire mode
        .collect();

    let file_paths: Vec<String> = files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    info!("Running questionnaire for {} files", file_paths.len());

    let results = llm
        .run_standard_questionnaire(&global_context, &file_paths)
        .await?;

    // Output results
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&results)?;
            if let Some(path) = output {
                std::fs::write(path, json)?;
            } else {
                println!("{}", json);
            }
        }
        OutputFormat::Text => {
            println!("\n📋 LLM Questionnaire Results");
            println!("═══════════════════════════════════════");
            println!("Files Audited: {}\n", results.len());

            for result in &results {
                println!("📄 {}", result.file);
                println!(
                    "  Reachable: {}",
                    if result.reachable {
                        "✅ Yes"
                    } else {
                        "❌ No"
                    }
                );
                println!("  Compliance Issues: {}", result.compliance_issues.len());
                println!(
                    "  Incomplete: {}",
                    if result.incomplete {
                        "⚠️ Yes"
                    } else {
                        "✅ No"
                    }
                );
                println!("  Suggested Tags: {}", result.suggested_tags.join(", "));
                if !result.improvement.is_empty() {
                    println!("  Improvement: {}", result.improvement);
                }
                println!();
            }

            if let Some(path) = output {
                let json = serde_json::to_string_pretty(&results)?;
                std::fs::write(path, json)?;
            }
        }
        OutputFormat::Csv => {
            let mut csv = String::from(
                "File,Reachable,ComplianceIssues,Incomplete,SuggestedTags,Improvement\n",
            );
            for result in &results {
                csv.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    result.file,
                    result.reachable,
                    result.compliance_issues.len(),
                    result.incomplete,
                    result.suggested_tags.join(";"),
                    result.improvement.replace(",", ";").replace("\n", " ")
                ));
            }
            if let Some(path) = output {
                std::fs::write(path, csv)?;
            } else {
                println!("{}", csv);
            }
        }
    }

    Ok(())
}

/// Run research analysis on a file
async fn run_research_analysis(
    file: &PathBuf,
    generate_tasks: bool,
    output: Option<PathBuf>,
    format: OutputFormat,
    config: &Config,
) -> anyhow::Result<()> {
    info!("🔍 Analyzing research file: {:?}", file);

    // Check if file exists
    if !file.exists() {
        return Err(anyhow::anyhow!("File not found: {:?}", file));
    }

    // Check if LLM is enabled
    if !config.llm.enabled {
        return Err(anyhow::anyhow!(
            "LLM is not enabled. Set LLM_ENABLED=true in environment"
        ));
    }

    let api_key = config.llm.api_key.as_ref().ok_or_else(|| {
        anyhow::anyhow!("LLM API key not found. Set XAI_API_KEY or GOOGLE_API_KEY")
    })?;

    // Create LLM client
    let llm_client = LlmClient::new_with_provider(
        api_key.clone(),
        config.llm.provider.clone(),
        config.llm.model.clone(),
        config.llm.max_tokens,
        config.llm.temperature,
    )?;

    // Analyze the file
    let breakdown = research::analyze_file(file, &llm_client, config).await?;

    // Determine output directory
    let output_dir = if let Some(ref out_path) = output {
        out_path.clone()
    } else {
        config
            .research
            .as_ref()
            .map(|r| PathBuf::from(&r.output_dir))
            .unwrap_or_else(|| PathBuf::from("docs/research_breakdowns"))
    };

    // Save breakdown
    let file_stem = file.file_stem().and_then(|s| s.to_str());
    let breakdown_path = research::save_breakdown(&breakdown, &output_dir, file_stem)?;

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&breakdown)?;
            println!("{}", json);
        }
        OutputFormat::Text => {
            println!("\n{}", "=".repeat(80));
            println!("📚 Research Breakdown: {}", breakdown.title);
            println!("{}", "=".repeat(80));
            println!("\n{}", breakdown.markdown_content);
            println!("\n{}", "=".repeat(80));
            println!("✅ Saved to: {:?}", breakdown_path);
        }
        OutputFormat::Csv => {
            println!("CSV format not supported for research breakdown");
        }
    }

    // Extract tasks if requested
    if generate_tasks {
        info!("📋 Extracting tasks from research...");

        let tasks =
            research::extract_tasks(&breakdown.markdown_content, &llm_client, config).await?;

        let task_path = breakdown_path.with_extension("tasks.json");
        research::save_tasks(&tasks, &task_path)?;

        println!("\n{}", "=".repeat(80));
        println!("📋 Extracted {} Tasks", tasks.len());
        println!("{}", "=".repeat(80));

        match format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&tasks)?;
                println!("{}", json);
            }
            OutputFormat::Text => {
                for (i, task) in tasks.iter().enumerate() {
                    println!("\n{}. {} [{:?}]", i + 1, task.title, task.complexity);
                    println!("   Component: {:?}", task.target_component);
                    println!("   {}", task.description);
                    if let Some(hours) = task.estimated_hours {
                        println!("   Estimated: {} hours", hours);
                    }
                    if !task.dependencies.is_empty() {
                        println!("   Dependencies: {}", task.dependencies.join(", "));
                    }
                }
            }
            OutputFormat::Csv => {
                println!("Title,Complexity,Component,Description,EstimatedHours");
                for task in &tasks {
                    println!(
                        "{},{:?},{:?},{},{}",
                        task.title,
                        task.complexity,
                        task.target_component,
                        task.description.replace(",", ";"),
                        task.estimated_hours.unwrap_or(0)
                    );
                }
            }
        }

        println!("\n✅ Saved tasks to: {:?}", task_path);
    }

    Ok(())
}

/// Run architecture visualization
fn run_visualization(
    path: &PathBuf,
    diagram_type: &str,
    component: Option<String>,
    output: Option<PathBuf>,
    format: OutputFormat,
) -> anyhow::Result<()> {
    info!("🎨 Generating {} diagram for: {:?}", diagram_type, path);

    let mermaid_code = match diagram_type {
        "neuromorphic" => {
            // TODO: Implement neuromorphic brain architecture diagram when neuromorphic_mapper is added
            println!("\n{}", "=".repeat(80));
            println!("🧠 Neuromorphic Architecture Analysis");
            println!("{}", "=".repeat(80));
            println!("Feature not yet implemented");
            println!("\n{}", "=".repeat(80));

            "graph TD\n    A[Neuromorphic] --> B[Not Implemented Yet]".to_string()
        }
        "component" => {
            // TODO: Implement component diagram generation when neuromorphic_mapper is added
            let _comp = component.ok_or_else(|| {
                anyhow::anyhow!("Component name required for component diagram type")
            })?;
            "graph TD\n    A[Component] --> B[Not Implemented Yet]".to_string()
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unknown diagram type: {}. Use 'neuromorphic' or 'component'",
                diagram_type
            ));
        }
    };

    match format {
        OutputFormat::Json => {
            // Output as JSON with the mermaid code
            let json = serde_json::json!({
                "diagram_type": diagram_type,
                "mermaid": mermaid_code,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Text => {
            println!("\n{}", "=".repeat(80));
            println!("📊 Mermaid Diagram ({} type)", diagram_type);
            println!("{}", "=".repeat(80));
            println!("\n{}\n", mermaid_code);
            println!("{}", "=".repeat(80));
            println!("\n💡 To visualize:");
            println!("   1. Copy the code above");
            println!("   2. Visit https://mermaid.live/");
            println!("   3. Paste and view the interactive diagram");
            println!("\n   Or save to file:");
            println!("   ./target/release/audit-cli visualize . -o diagram.mmd");
            println!("{}", "=".repeat(80));
        }
        OutputFormat::Csv => {
            println!("CSV format not supported for diagrams. Use 'text' or 'json'.");
        }
    }

    // Save to file if specified
    if let Some(output_path) = output {
        std::fs::write(&output_path, &mermaid_code)?;
        println!("\n✅ Saved diagram to: {:?}", output_path);
    }

    Ok(())
}

fn run_tree_visualization(
    path: &PathBuf,
    max_depth: usize,
    with_tags: bool,
    output: Option<&PathBuf>,
) -> anyhow::Result<()> {
    use rustassistant::directory_tree::DirectoryTreeBuilder;

    println!("🌲 Building directory tree...");

    let builder = DirectoryTreeBuilder::new(path);

    let tree = if with_tags {
        println!("📋 Scanning for audit tags...");
        let scanner = TagScanner::new()?;
        let tags = scanner.scan_directory(path)?;
        println!("Found {} audit tags", tags.len());
        builder.build_with_tags(&tags)?
    } else {
        builder.build()?
    };

    let summary = builder.generate_summary(&tree);
    let ascii_tree = builder.to_ascii_tree(&tree, max_depth);

    // Print summary
    println!("\n📊 Tree Summary:");
    println!("  Total files: {}", summary.total_files);
    println!("  Total lines: {}", summary.total_lines);
    println!("  Total TODOs: {}", summary.total_todos);
    println!("  Total FIXMEs: {}", summary.total_fixmes);
    println!("  Total audit tags: {}", summary.total_tags);
    println!("  Total issues: {}", summary.total_issues);
    println!("    - Critical: {}", summary.critical_issues);
    println!("    - High: {}", summary.high_issues);
    println!("  Directories analyzed: {}", summary.directories_analyzed);

    // Print hotspots (files/dirs with most issues)
    let hotspots = builder.find_hotspots(&tree, 10);
    if !hotspots.is_empty() {
        println!("\n🔥 Top Issue Hotspots:");
        for (i, hotspot) in hotspots.iter().enumerate() {
            println!(
                "  {}. {} ({} issues: {} critical, {} high)",
                i + 1,
                hotspot.name,
                hotspot.total_issues,
                hotspot.critical,
                hotspot.high
            );
        }
    }

    // Print tree
    println!("\n🌲 Directory Tree (depth: {}):", max_depth);
    println!("{}", ascii_tree);

    // Save to file if requested
    if let Some(output_path) = output {
        let json = serde_json::to_string_pretty(&tree)?;
        fs::write(output_path, json)?;
        println!("\n✅ Tree saved to: {}", output_path.display());
    }

    Ok(())
}

/// Run code formatting
fn run_format(
    path: &PathBuf,
    check: bool,
    formatter_names: Vec<String>,
    output: Option<PathBuf>,
    format: OutputFormat,
) -> Result<()> {
    let mode = if check {
        FormatMode::Check
    } else {
        FormatMode::Fix
    };

    info!(
        "Running code formatting in {:?} mode on: {}",
        mode,
        path.display()
    );

    // Parse formatter names
    let formatters = if formatter_names.is_empty() {
        vec![]
    } else {
        formatter_names
            .iter()
            .filter_map(|name| match name.to_lowercase().as_str() {
                "rust" | "cargo-fmt" | "rustfmt" => Some(Formatter::RustFmt),
                "kotlin" | "ktlint" => Some(Formatter::KtLint),
                "prettier" | "ts" | "typescript" | "js" | "javascript" => Some(Formatter::Prettier),
                "python" | "black" => Some(Formatter::Black),
                _ => {
                    warn!("Unknown formatter: {}", name);
                    None
                }
            })
            .collect()
    };

    let formatter = CodeFormatter::new(path, mode).with_formatters(formatters);

    let result = formatter.run()?;

    // Print results
    match format {
        OutputFormat::Text => {
            println!("\n{}", "=".repeat(60));
            println!("Code Formatting Results");
            println!("{}", "=".repeat(60));
            println!();

            for fmt_result in &result.results {
                let status = if fmt_result.success {
                    if fmt_result.files_changed > 0 {
                        if check {
                            "⚠️  NEEDS FORMATTING"
                        } else {
                            "✓ FORMATTED"
                        }
                    } else {
                        "✓ OK"
                    }
                } else {
                    "✗ FAILED"
                };

                println!("📦 {}: {}", fmt_result.formatter.name(), status);
                println!(
                    "   Files processed: {}, Files changed: {}",
                    fmt_result.files_processed, fmt_result.files_changed
                );

                if !fmt_result.warnings.is_empty() {
                    for warning in &fmt_result.warnings {
                        println!("   ⚠️  {}", warning);
                    }
                }

                if !fmt_result.errors.is_empty() {
                    for error in &fmt_result.errors {
                        println!("   ❌ {}", error);
                    }
                }
                println!();
            }

            println!("{}", "=".repeat(60));
            println!("{}", result.summary());
            println!("{}", "=".repeat(60));

            if check && result.total_changed > 0 {
                println!("\n💡 Run without --check to apply formatting");
            }
        }
        OutputFormat::Json => {
            let json_output = serde_json::json!({
                "mode": if check { "check" } else { "fix" },
                "total_files": result.total_files,
                "total_changed": result.total_changed,
                "success": result.success,
                "results": result.results.iter().map(|r| serde_json::json!({
                    "formatter": r.formatter.name(),
                    "files_processed": r.files_processed,
                    "files_changed": r.files_changed,
                    "success": r.success,
                    "errors": r.errors,
                    "warnings": r.warnings,
                })).collect::<Vec<_>>(),
            });

            println!("{}", serde_json::to_string_pretty(&json_output)?);
        }
        OutputFormat::Csv => {
            println!("formatter,files_processed,files_changed,success");
            for fmt_result in &result.results {
                println!(
                    "{},{},{},{}",
                    fmt_result.formatter.name(),
                    fmt_result.files_processed,
                    fmt_result.files_changed,
                    fmt_result.success
                );
            }
        }
    }

    // Save to file if requested
    if let Some(output_path) = output {
        let json_output = serde_json::json!({
            "mode": if check { "check" } else { "fix" },
            "total_files": result.total_files,
            "total_changed": result.total_changed,
            "success": result.success,
            "results": result.results.iter().map(|r| serde_json::json!({
                "formatter": r.formatter.name(),
                "files_processed": r.files_processed,
                "files_changed": r.files_changed,
                "success": r.success,
                "errors": r.errors,
                "warnings": r.warnings,
            })).collect::<Vec<_>>(),
        });

        fs::write(&output_path, serde_json::to_string_pretty(&json_output)?)?;
        info!("Results saved to: {}", output_path.display());
    }

    // Exit with error code if formatting check failed
    if check && !result.success {
        std::process::exit(1);
    }

    Ok(())
}

/// Run LLM audit (Regular or Full mode)
async fn run_llm_audit(
    path: &Path,
    mode: &str,
    provider: &str,
    focus_areas: Vec<String>,
    output: Option<PathBuf>,
    _config: &Config,
) -> Result<()> {
    use rustassistant::llm_audit::{AuditMode, LlmAuditor};

    let audit_mode = match mode.to_lowercase().as_str() {
        "full" => AuditMode::Full,
        "regular" | "reg" => AuditMode::Regular,
        _ => {
            warn!("Unknown audit mode '{}', defaulting to Regular", mode);
            AuditMode::Regular
        }
    };

    info!(
        "🤖 Running {} LLM Audit with {} provider on: {}",
        audit_mode,
        provider,
        path.display()
    );

    // Create auditor with specified provider
    let auditor = LlmAuditor::new_with_provider(provider, path)?;

    match audit_mode {
        AuditMode::Regular => {
            let result = auditor.run_regular_audit(path, focus_areas).await?;

            println!("\n{}", "=".repeat(70));
            println!("🔍 REGULAR AUDIT RESULTS");
            println!("{}", "=".repeat(70));
            println!();
            println!("📊 Overall Health: {:.1}%", result.overall_health);
            println!("🎯 Confidence: {:.1}%", result.confidence);
            println!();
            println!("🏗️  Architecture Assessment:");
            println!("{}", result.architecture_assessment);
            println!();
            println!("🔐 Security Concerns: {}", result.security_concerns.len());
            println!("📋 Recommendations: {}", result.recommendations.len());
            println!("{}", "=".repeat(70));

            // Save to file if requested
            if let Some(output_path) = output {
                let json = serde_json::to_string_pretty(&result)?;
                fs::write(&output_path, json)?;
                info!("Results saved to: {}", output_path.display());
            }
        }
        AuditMode::Full => {
            info!("📊 Running full audit (placeholder)...");
            let result = auditor.run_full_audit(path).await?;

            println!("\n{}", "=".repeat(70));
            println!("🔬 FULL AUDIT RESULTS");
            println!("{}", "=".repeat(70));
            println!();
            println!("📊 Overall Health: {:.1}%", result.overall_health);
            println!("📁 Files Analyzed: {}", result.file_analyses.len());
            println!("🚨 Critical Files: {}", result.critical_files.len());
            println!();
            println!("📝 Master Review:");
            println!("{}", result.master_review.executive_summary);
            println!();
            println!("🎯 Top Priorities:");
            for (i, priority) in result
                .master_review
                .top_priorities
                .iter()
                .take(5)
                .enumerate()
            {
                println!("  {}. {}", i + 1, priority);
            }
            println!();
            println!("💪 Strengths: {}", result.master_review.strengths.len());
            println!("⚠️  Weaknesses: {}", result.master_review.weaknesses.len());
            println!("{}", "=".repeat(70));

            // Save to file if requested
            if let Some(output_path) = output {
                let json = serde_json::to_string_pretty(&result)?;
                fs::write(&output_path, json)?;
                info!("Results saved to: {}", output_path.display());
            }
        }
    }

    Ok(())
}

/// Run tree state tracking (CI/CD integration)
fn run_tree_state(
    path: &Path,
    show_diff: bool,
    ci_summary: bool,
    output: Option<PathBuf>,
    format: OutputFormat,
) -> Result<()> {
    info!("📁 Building tree state for: {}", path.display());

    let manager = TreeStateManager::new(path);
    let current_state = manager.build_current_state()?;

    if show_diff {
        // Load previous state and compare
        if let Some(previous_state) = manager.load_previous_state()? {
            let diff = manager.diff(&previous_state, &current_state);

            manager.print_diff(&diff);

            if ci_summary {
                let summary = manager.generate_ci_summary(&diff);
                println!("\n{}", summary);
            }

            // Save diff if output specified
            if let Some(ref output_path) = output {
                match format {
                    OutputFormat::Json => {
                        let json = serde_json::to_string_pretty(&diff)?;
                        fs::write(output_path, json)?;
                    }
                    OutputFormat::Text | OutputFormat::Csv => {
                        let summary = manager.generate_ci_summary(&diff);
                        fs::write(output_path, summary)?;
                    }
                }
                info!("Diff saved to: {}", output_path.display());
            }
        } else {
            println!("No previous state found - this is the first run.");
            manager.print_summary(&current_state);
        }
    } else {
        manager.print_summary(&current_state);
    }

    // Always save current state
    manager.save_state(&current_state)?;

    // Save state to output if specified and not in diff mode
    if !show_diff {
        if let Some(ref output_path) = output {
            let json = serde_json::to_string_pretty(&current_state)?;
            fs::write(output_path, json)?;
            info!("State saved to: {}", output_path.display());
        }
    }

    Ok(())
}

/// Run Grok 4.1 reasoning audit with batching
#[allow(clippy::too_many_arguments)]
async fn run_grok_audit(
    path: &Path,
    category: &str,
    max_turns: usize,
    max_batch_tokens: usize,
    changed_only: bool,
    exclude_docs: bool,
    exclude_config: bool,
    enable_code_exec: bool,
    output: Option<PathBuf>,
    format: OutputFormat,
    _config: &Config,
) -> Result<()> {
    use rustassistant::cache::AuditCache;
    use rustassistant::grok_reasoning::{analyze_all_batches, RetryConfig};
    use rustassistant::llm_config::{CacheConfig, LlmConfig};
    use sha2::{Digest, Sha256};

    println!("\n{}", "=".repeat(70));
    println!("🧠 GROK 4.1 REASONING AUDIT");
    println!("{}", "=".repeat(70));

    // Load LLM config for budget and retry settings
    let llm_config = LlmConfig::load(path).unwrap_or_default();

    // Get API key
    let api_key = std::env::var("XAI_API_KEY")
        .map_err(|_| AuditError::other("XAI_API_KEY environment variable not set".to_string()))?;

    // Create retry config from llm_config
    let retry_config = RetryConfig::from_limits(&llm_config.limits);

    // Create client with retry config
    let client = GrokReasoningClient::with_full_config(
        api_key,
        None,
        Some(max_turns),
        enable_code_exec,
        true, // enable reasoning
        Some(retry_config),
    )?;

    println!("📋 Configuration:");
    println!("   Model: {}", client.model());
    println!("   Max Turns: {}", client.max_turns());
    println!("   Max Batch Tokens: {}", max_batch_tokens);
    println!("   Category: {}", category);
    println!("   Changed Only: {}", changed_only);
    println!("   Exclude Docs: {}", exclude_docs);
    println!("   Exclude Config: {}", exclude_config);
    println!(
        "   Retry: {} attempts with {}ms initial delay",
        client.retry_config().max_retries,
        client.retry_config().initial_delay_ms
    );
    if let Some(budget) = llm_config.limits.max_monthly_cost_usd {
        println!("   Monthly Budget: ${:.2}", budget);
    }
    println!();

    // Build tree state
    let manager = TreeStateManager::new(path);
    let current_state = manager.build_current_state()?;

    // Determine which files to analyze
    let files_to_analyze: Vec<&rustassistant::tree_state::FileState> = if changed_only {
        if let Some(previous_state) = manager.load_previous_state()? {
            let diff = manager.diff(&previous_state, &current_state);
            let changed_paths: std::collections::HashSet<_> = diff
                .changes
                .iter()
                .filter(|c| c.needs_llm_analysis)
                .map(|c| c.path.clone())
                .collect();

            current_state
                .files
                .values()
                .filter(|f| changed_paths.contains(&f.path))
                .collect()
        } else {
            println!("⚠️  No previous state found, analyzing all files");
            current_state.files.values().collect()
        }
    } else {
        current_state.files.values().collect()
    };

    // Filter by category
    let category_filter = match category.to_lowercase().as_str() {
        "audit" => Some(FileCategory::Audit),
        "janus" => Some(FileCategory::Janus),
        "clients" => Some(FileCategory::Clients),
        "execution" => Some(FileCategory::Execution),
        "all" => None,
        _ => None,
    };

    let filtered_files: Vec<_> = files_to_analyze
        .into_iter()
        .filter(|f| {
            // Exclude docs if requested
            if exclude_docs && f.category == FileCategory::Docs {
                return false;
            }
            // Exclude config if requested
            if exclude_config && f.category == FileCategory::Config {
                return false;
            }
            // Apply category filter
            if let Some(ref cat) = category_filter {
                f.category == *cat
            } else {
                true
            }
        })
        .collect();

    println!("📁 Files to analyze: {}", filtered_files.len());

    if filtered_files.is_empty() {
        println!("No files to analyze.");
        return Ok(());
    }

    // Score files for prioritization (could integrate with FileScorer for priority ordering)
    let _scorer = FileScorer::new();

    // Prepare files for analysis
    let mut files_for_analysis: Vec<FileForAnalysis> = Vec::new();

    for file_state in filtered_files {
        let full_path = path.join(&file_state.path);
        if let Ok(content) = fs::read_to_string(&full_path) {
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let hash = format!("{:x}", hasher.finalize());

            files_for_analysis.push(FileForAnalysis {
                path: file_state.path.clone(),
                content,
                lines: file_state.lines,
                score: None, // Could integrate with scorer here
                category: file_state.category,
                content_hash: hash,
            });
        }
    }

    // Create batches
    let batches = client.create_batches(files_for_analysis, max_batch_tokens);
    println!("📦 Created {} batches", batches.len());

    // Initialize cache
    let cache_config = CacheConfig::default();
    let cache = AuditCache::new(path, &cache_config).ok();

    // Analyze batches with progress
    let progress_callback = Box::new(|current: usize, total: usize, msg: &str| {
        println!("   [{}/{}] {}", current, total, msg);
    });

    let results =
        analyze_all_batches(&client, batches, cache.as_ref(), Some(progress_callback)).await?;

    // Aggregate results
    let total_files: usize = results.iter().map(|r| r.file_results.len()).sum();
    let total_tokens: usize = results.iter().map(|r| r.total_tokens.total_tokens).sum();
    let total_time: u64 = results.iter().map(|r| r.processing_time_ms).sum();

    let all_file_results: Vec<_> = results
        .iter()
        .flat_map(|r| r.file_results.clone())
        .collect();

    let avg_score: f64 = if !all_file_results.is_empty() {
        all_file_results
            .iter()
            .map(|r| r.overall_score)
            .sum::<f64>()
            / all_file_results.len() as f64
    } else {
        0.0
    };

    let critical_issues: usize = all_file_results
        .iter()
        .flat_map(|r| &r.issues)
        .filter(|i| i.severity == "critical")
        .count();

    let high_issues: usize = all_file_results
        .iter()
        .flat_map(|r| &r.issues)
        .filter(|i| i.severity == "high")
        .count();

    // Calculate estimated cost
    let estimated_input_tokens: usize = results
        .iter()
        .flat_map(|r| &r.file_results)
        .map(|f| f.tokens_used.prompt_tokens)
        .sum();
    let estimated_output_tokens: usize = results
        .iter()
        .flat_map(|r| &r.file_results)
        .map(|f| f.tokens_used.completion_tokens)
        .sum();
    let estimated_cost = llm_config.estimate_cost(estimated_input_tokens, estimated_output_tokens);

    // Print summary
    println!("\n{}", "=".repeat(70));
    println!("📊 AUDIT RESULTS SUMMARY");
    println!("{}", "=".repeat(70));
    println!("   Files Analyzed: {}", total_files);
    println!("   Total Tokens: {}", total_tokens);
    println!(
        "   Token Breakdown: {} input, {} output",
        estimated_input_tokens, estimated_output_tokens
    );
    println!("   Estimated Cost: ${:.4}", estimated_cost);
    println!("   Processing Time: {}ms", total_time);
    println!("   Average Score: {:.1}", avg_score);
    println!("   Critical Issues: {}", critical_issues);
    println!("   High Issues: {}", high_issues);

    // Check budget status
    match llm_config.check_budget(estimated_cost) {
        rustassistant::llm_config::BudgetStatus::Warning {
            current,
            limit,
            usage_pct,
        } => {
            println!(
                "\n⚠️  BUDGET WARNING: ${:.2}/${:.2} ({:.1}% used)",
                current, limit, usage_pct
            );
        }
        rustassistant::llm_config::BudgetStatus::Exceeded { current, limit } => {
            println!(
                "\n🚫 BUDGET EXCEEDED: ${:.2}/${:.2} - consider increasing budget",
                current, limit
            );
        }
        _ => {}
    }

    // Show worst files
    let mut sorted_results = all_file_results.clone();
    sorted_results.sort_by(|a, b| {
        a.overall_score
            .partial_cmp(&b.overall_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if !sorted_results.is_empty() {
        println!("\n⚠️  Files Needing Attention (lowest scores):");
        for result in sorted_results.iter().take(5) {
            println!(
                "   {}: {:.1} (security: {:.1}, quality: {:.1})",
                result.path, result.overall_score, result.security_score, result.quality_score
            );
        }
    }

    // Save results
    if let Some(output_path) = output {
        let output_data = serde_json::json!({
            "summary": {
                "files_analyzed": total_files,
                "total_tokens": total_tokens,
                "input_tokens": estimated_input_tokens,
                "output_tokens": estimated_output_tokens,
                "estimated_cost_usd": estimated_cost,
                "processing_time_ms": total_time,
                "average_score": avg_score,
                "critical_issues": critical_issues,
                "high_issues": high_issues,
            },
            "file_results": all_file_results,
            "batch_results": results,
        });

        match format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&output_data)?;
                fs::write(&output_path, json)?;
            }
            OutputFormat::Text | OutputFormat::Csv => {
                let json = serde_json::to_string_pretty(&output_data)?;
                fs::write(&output_path, json)?;
            }
        }
        info!("Results saved to: {}", output_path.display());
    }

    // Save updated cache
    if let Some(c) = cache {
        c.save()?;
        c.print_summary();
    }

    // Save updated tree state
    manager.save_state(&current_state)?;

    println!("{}", "=".repeat(70));

    Ok(())
}

/// Run JANUS whitepaper conformity audit with Claude Opus 4.5
///
/// Uses Anthropic's most capable model (Claude Opus 4.5) to perform deep analysis
/// of the JANUS neuromorphic trading system implementation against the technical
/// whitepaper specifications.
///
/// Key verification areas:
/// - GAF transformation mathematics (normalization, polar transforms)
/// - LTN Łukasiewicz logic implementation
/// - Brain-region component mappings (hippocampus, amygdala, etc.)
/// - Memory hierarchy (episodic buffer, SWR replay, neocortex schemas)
/// - Compliance constraints (wash sale rules, position limits)
#[allow(clippy::too_many_arguments)]
async fn run_janus_audit(
    path: &Path,
    provider: &str,
    focus: &str,
    sections: Option<String>,
    verify_math: bool,
    verify_mappings: bool,
    compliance_report: bool,
    output: Option<PathBuf>,
    format: OutputFormat,
    _config: &Config,
) -> Result<()> {
    use rustassistant::llm_audit::LlmAuditor;
    use rustassistant::llm_config::claude_models;

    println!("\n{}", "=".repeat(70));
    println!("🧠 JANUS WHITEPAPER CONFORMITY AUDIT");
    println!("{}", "=".repeat(70));
    println!();

    // Determine model based on provider
    let model_name = match provider.to_lowercase().as_str() {
        "opus" | "claude" | "anthropic" => claude_models::CLAUDE_OPUS_4_5,
        "sonnet" => claude_models::CLAUDE_SONNET_4,
        "haiku" => claude_models::CLAUDE_HAIKU_3_5,
        _ => claude_models::CLAUDE_OPUS_4_5,
    };

    println!("📋 Configuration:");
    println!("   Model: {} (Claude Opus 4.5)", model_name);
    println!("   Focus: {}", focus);
    println!("   Verify Math: {}", verify_math);
    println!("   Verify Mappings: {}", verify_mappings);
    println!("   Compliance Report: {}", compliance_report);
    if let Some(ref s) = sections {
        println!("   Whitepaper Sections: {}", s);
    }
    println!();

    // Build focus areas based on input
    let mut focus_areas = Vec::new();

    match focus.to_lowercase().as_str() {
        "all" => {
            focus_areas.push("GAF transformation and ViViT integration".to_string());
            focus_areas.push("LTN Łukasiewicz logic constraints".to_string());
            focus_areas.push("Neuromorphic brain-region mappings".to_string());
            focus_areas.push("Memory hierarchy (hippocampus, SWR, neocortex)".to_string());
            focus_areas.push("Compliance (wash sale, position limits)".to_string());
            focus_areas.push("Forward/Backward service architecture".to_string());
        }
        "neuromorphic" => {
            focus_areas.push("Brain-region component mappings".to_string());
            focus_areas.push("Visual cortex (GAF pattern recognition)".to_string());
            focus_areas.push("Hippocampus (episodic memory)".to_string());
            focus_areas.push("Amygdala (threat detection)".to_string());
            focus_areas.push("Basal ganglia (action selection)".to_string());
        }
        "gaf" => {
            focus_areas.push("GAF normalization with learnable parameters (γ, β)".to_string());
            focus_areas.push("Polar coordinate transformation".to_string());
            focus_areas.push("Gramian matrix generation".to_string());
            focus_areas.push("ViViT patch embedding".to_string());
        }
        "ltn" => {
            focus_areas.push("Łukasiewicz t-norm implementation".to_string());
            focus_areas.push("Product logic for training".to_string());
            focus_areas.push("Constraint satisfaction tracking".to_string());
            focus_areas.push("Predicate evaluation".to_string());
        }
        "memory" => {
            focus_areas.push("Hippocampal episodic buffer".to_string());
            focus_areas.push("Sharp-Wave Ripple (SWR) replay".to_string());
            focus_areas.push("TD-error priority calculation".to_string());
            focus_areas.push("Neocortical schema formation".to_string());
            focus_areas.push("Qdrant vector storage integration".to_string());
        }
        "compliance" => {
            focus_areas.push("Wash sale rule implementation (30-day window)".to_string());
            focus_areas.push("Position limits (MAX_POSITION_SIZE = 10%)".to_string());
            focus_areas.push("Daily loss limits (DAILY_LOSS_LIMIT = 2%)".to_string());
            focus_areas.push("Circuit breaker thresholds".to_string());
        }
        _ => {
            focus_areas.push(focus.to_string());
        }
    }

    // Add section-specific focus if provided
    if let Some(ref section_list) = sections {
        for section in section_list.split(',') {
            focus_areas.push(format!("Whitepaper Section {}", section.trim()));
        }
    }

    // Add math verification focus
    if verify_math {
        focus_areas
            .push("MATHEMATICAL VERIFICATION: Check all formulas match whitepaper".to_string());
        focus_areas.push("GAF: x̃ = γ * tanh(arccos(x)/π) + β".to_string());
        focus_areas.push("PER: P(i) = (|δᵢ| + ε)^α / Σⱼ(|δⱼ| + ε)^α".to_string());
        focus_areas.push("Mahalanobis: D = √((x-μ)ᵀΣ⁻¹(x-μ))".to_string());
    }

    // Add mapping verification focus
    if verify_mappings {
        focus_areas.push(
            "MAPPING VERIFICATION: Verify brain-region to code component mappings".to_string(),
        );
        focus_areas.push("Visual Cortex → GAF + ViViT modules".to_string());
        focus_areas.push("Hippocampus → Episode storage + SWR buffer".to_string());
        focus_areas.push("Amygdala → Threat detection + circuit breakers".to_string());
        focus_areas.push("Basal Ganglia → Dual-pathway action selection".to_string());
        focus_areas.push("Neocortex → Schema formation + Qdrant storage".to_string());
    }

    println!("🎯 Focus Areas ({}):", focus_areas.len());
    for area in &focus_areas {
        println!("   • {}", area);
    }
    println!();

    // Create auditor with Claude provider
    info!("🤖 Initializing Claude Opus 4.5 auditor for JANUS conformity check...");

    let auditor = match LlmAuditor::new_with_provider(provider, path) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("❌ Failed to initialize Claude auditor: {}", e);
            eprintln!();
            eprintln!("💡 Make sure ANTHROPIC_API_KEY is set in your environment:");
            eprintln!("   export ANTHROPIC_API_KEY=your-api-key");
            return Err(e);
        }
    };

    println!("🔍 Running JANUS whitepaper conformity audit...");
    println!("   (Using Claude Opus 4.5 - Anthropic's most capable model)");
    println!();

    // Run the audit
    let result = auditor.run_regular_audit(path, focus_areas).await?;

    // Display results
    println!("\n{}", "=".repeat(70));
    println!("📊 JANUS CONFORMITY AUDIT RESULTS");
    println!("{}", "=".repeat(70));
    println!();

    println!("🎯 Overall Conformity Score: {:.1}%", result.overall_health);
    println!("📈 Confidence Level: {:.1}%", result.confidence);
    println!();

    println!("🏗️  Architecture Assessment:");
    println!("{}", result.architecture_assessment);
    println!();

    if !result.patterns.is_empty() {
        println!("📋 Detected Patterns:");
        for pattern in &result.patterns {
            println!("   • {}", pattern);
        }
        println!();
    }

    if !result.security_concerns.is_empty() {
        println!(
            "⚠️  Security Concerns ({}):",
            result.security_concerns.len()
        );
        for concern in &result.security_concerns {
            println!("   [{:?}] {}", concern.severity, concern.description);
            if !concern.recommendation.is_empty() {
                println!("      → {}", concern.recommendation);
            }
        }
        println!();
    }

    if !result.quality_observations.is_empty() {
        println!("📝 Quality Observations:");
        for obs in &result.quality_observations {
            println!("   • {}", obs);
        }
        println!();
    }

    if !result.recommendations.is_empty() {
        println!("💡 Recommendations ({}):", result.recommendations.len());
        for rec in &result.recommendations {
            println!("   [{}] {}", rec.priority, rec.recommendation);
            println!(
                "      Category: {} | Benefit: {}",
                rec.category, rec.benefit
            );
        }
        println!();
    }

    // Generate compliance report if requested
    if compliance_report {
        println!("{}", "=".repeat(70));
        println!("📋 COMPLIANCE SUMMARY REPORT");
        println!("{}", "=".repeat(70));
        println!();
        println!("This report is generated for regulatory review purposes.");
        println!();
        println!("System: Project JANUS - Neuromorphic Trading System");
        println!(
            "Audit Date: {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!("Audit Model: Claude Opus 4.5 (Anthropic)");
        println!();
        println!("Conformity Status: {:.1}%", result.overall_health);
        println!("Security Concerns: {}", result.security_concerns.len());
        println!("Recommendations: {}", result.recommendations.len());
        println!();
    }

    println!("{}", "=".repeat(70));

    // Save results to file if requested
    if let Some(output_path) = output {
        let output_data = serde_json::json!({
            "audit_type": "janus_whitepaper_conformity",
            "model": model_name,
            "provider": "anthropic",
            "focus": focus,
            "verify_math": verify_math,
            "verify_mappings": verify_mappings,
            "compliance_report": compliance_report,
            "sections": sections,
            "results": {
                "overall_health": result.overall_health,
                "confidence": result.confidence,
                "architecture_assessment": result.architecture_assessment,
                "patterns": result.patterns,
                "security_concerns": result.security_concerns,
                "quality_observations": result.quality_observations,
                "tech_debt_areas": result.tech_debt_areas,
                "recommendations": result.recommendations,
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        match format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&output_data)?;
                fs::write(&output_path, json)?;
            }
            OutputFormat::Text | OutputFormat::Csv => {
                // For text/csv, still save as JSON but with different extension handling
                let json = serde_json::to_string_pretty(&output_data)?;
                fs::write(&output_path, json)?;
            }
        }
        info!("✅ Results saved to: {}", output_path.display());
    }

    Ok(())
}
