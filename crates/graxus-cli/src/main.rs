pub mod args;
mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "graxus", version = "0.1.0", about = "AI-native codebase knowledge engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new graxus project
    Init {
        /// Project root path
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Project name override
        #[arg(long)]
        name: Option<String>,
        /// Overwrite existing graxus.yaml
        #[arg(long)]
        force: bool,
        /// Minimal config
        #[arg(long)]
        minimal: bool,
    },

    /// Index the project (scan files, build graph and codemap)
    Index {
        /// Only scan docs
        #[arg(long)]
        docs_only: bool,
        /// Only scan code
        #[arg(long)]
        code_only: bool,
        /// Include glob (repeatable)
        #[arg(long)]
        include: Vec<String>,
        /// Exclude glob (repeatable)
        #[arg(long)]
        exclude: Vec<String>,
        /// Filter by language (repeatable)
        #[arg(long)]
        lang: Vec<String>,
        /// Maximum files to process
        #[arg(long)]
        max_files: Option<usize>,
    },

    /// Show project status
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Docs graph operations
    Graph {
        #[command(subcommand)]
        sub: GraphCmd,
    },

    /// Code codemap operations
    Codemap {
        #[command(subcommand)]
        sub: CodemapCmd,
    },

    /// Search the project (literal text search)
    Find {
        /// Search query
        query: String,
        /// Search only docs
        #[arg(long)]
        docs: bool,
        /// Search only code
        #[arg(long)]
        code: bool,
        /// Search for symbols
        #[arg(long)]
        symbol: bool,
        /// Maximum results
        #[arg(long, default_value = "200")]
        max_results: usize,
        /// Context lines around match
        #[arg(long, default_value = "2")]
        context_lines: usize,
        /// Case-sensitive search
        #[arg(long)]
        case_sensitive: bool,
    },

    /// Replace text across the project
    Replace {
        /// Pattern to find
        pattern: String,
        /// Replacement text
        replacement: String,
        /// Use regex
        #[arg(long)]
        regex: bool,
        /// Preview changes (default)
        #[arg(long)]
        preview: bool,
        /// Apply changes
        #[arg(long)]
        r#apply: bool,
        /// Include glob
        #[arg(long)]
        include: Vec<String>,
        /// Exclude glob
        #[arg(long)]
        exclude: Vec<String>,
        /// Filter by language
        #[arg(long)]
        lang: Vec<String>,
        /// Max files to modify
        #[arg(long, default_value = "100")]
        max_files: usize,
        /// Max replacements
        #[arg(long, default_value = "1000")]
        max_replacements: usize,
    },

    /// Agent context queries
    Context {
        /// Query string
        #[arg(short, long)]
        query: Option<String>,
        /// File path
        #[arg(short, long)]
        file: Option<String>,
        /// Symbol name
        #[arg(short, long)]
        symbol: Option<String>,
        /// Max token budget
        #[arg(long, default_value = "12000")]
        budget: usize,
        /// Max files in context
        #[arg(long, default_value = "20")]
        max_files: usize,
        /// Max symbols in context
        #[arg(long, default_value = "100")]
        max_symbols: usize,
        /// Max docs in context
        #[arg(long, default_value = "20")]
        max_notes: usize,
        /// Max depth
        #[arg(long, default_value = "2")]
        depth: usize,
        /// Minimum confidence (0-100)
        #[arg(long, default_value = "0")]
        min_confidence: f64,
    },

    /// Export agent context
    AgentExport {
        /// Max token budget for bounded export
        #[arg(long)]
        budget: Option<usize>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Run health diagnostics
    Doctor {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Fail on warnings
        #[arg(long)]
        strict: bool,
    },

    /// Show blast radius of file changes
    Impact {
        /// File or symbol to analyze
        target: String,
        /// Max traversal depth
        #[arg(long, default_value = "3")]
        depth: usize,
        /// Direction: callers, callees, both, importers
        #[arg(long, default_value = "callers")]
        direction: String,
        /// Max symbols returned
        #[arg(long, default_value = "200")]
        max_symbols: usize,
        /// Max files returned
        #[arg(long, default_value = "100")]
        max_files: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show most-called symbols (hotspots)
    Hotspots {
        /// Number of results
        #[arg(long, default_value = "25")]
        limit: usize,
        /// Minimum usage count
        #[arg(long, default_value = "1")]
        min_usage: usize,
        /// Exclude test symbols
        #[arg(long)]
        exclude_tests: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show potentially dead code (uncalled symbols)
    DeadCode {
        /// Minimum confidence (0-100)
        #[arg(long, default_value = "70")]
        min_confidence: f64,
        /// Number of results
        #[arg(long, default_value = "100")]
        limit: usize,
        /// Include exported symbols
        #[arg(long)]
        include_exported: bool,
        /// Exclude test symbols
        #[arg(long)]
        exclude_tests: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show edit history
    History {
        /// Filter by file path
        #[arg(long)]
        file: Option<String>,
        /// Max entries to show
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Watch for file changes and auto-reindex
    Watch {
        /// Debounce milliseconds
        #[arg(long, default_value = "500")]
        debounce: u64,
    },

    /// Detect and list workspaces in the project
    Workspaces {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Manage plugins
    Plugins {
        #[command(subcommand)]
        sub: PluginCmd,
    },

    /// Manage configuration and API keys
    Config {
        #[command(subcommand)]
        sub: ConfigCmd,
    },

    /// Generate vector embeddings for semantic search
    Embed {
        /// Re-embed everything (ignore cache)
        #[arg(long)]
        refresh: bool,
        /// Dry run — show what would be embedded
        #[arg(long)]
        dry_run: bool,
    },

    /// Semantic search using embeddings
    Search {
        /// Search query
        query: String,
        /// Number of results to return
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Minimum similarity score (0.0-1.0)
        #[arg(long, default_value = "0.2")]
        min_score: f64,
    },

    /// Generate documentation with LLM
    Generate {
        #[command(subcommand)]
        sub: GenerateCmd,
    },

    /// Start JSON-RPC server on stdio
    Serve,

    /// List detected dependencies
    Deps {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Search with regex pattern
    Regex {
        /// Regex pattern
        pattern: String,
        /// Docs only
        #[arg(long)]
        docs: bool,
        /// Code only
        #[arg(long)]
        code: bool,
        /// Maximum results
        #[arg(long, default_value = "200")]
        max_results: usize,
        /// Context lines around match
        #[arg(long, default_value = "2")]
        context_lines: usize,
    },

    /// Generate HTML visualizations
    Visualize {
        #[command(subcommand)]
        sub: VisualizeCmd,
    },

    /// Rollback an edit snapshot
    Rollback {
        /// Snapshot ID (or prefix)
        snapshot_id: String,
        /// Preview files to restore
        #[arg(long)]
        preview: bool,
        /// Actually restore files
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Subcommand)]
enum VisualizeCmd {
    /// Generate docs graph visualization
    Docs {
        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
    /// Generate codemap visualization
    Codemap {
        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
    /// Generate call graph visualization
    Callgraph {
        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
    /// Generate blast radius visualization
    Impact {
        /// Target symbol or file
        target: String,
        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
    /// Generate docs-code bridge visualization
    Bridge {
        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
    /// Generate dependency graph visualization
    Deps {
        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
    /// Generate all visualizations
    All {
        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
enum PluginCmd {
    /// List installed plugins
    List,
    /// Install a plugin from a directory
    Install {
        /// Path to plugin directory
        path: String,
    },
    /// Uninstall a plugin by name
    Uninstall {
        /// Plugin name
        name: String,
    },
}

#[derive(Subcommand)]
enum GraphCmd {
    /// Show docs graph
    Docs {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Filter by file
        #[arg(short, long)]
        file: Option<String>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
        /// Max docs to show
        #[arg(long, default_value = "100")]
        max_notes: usize,
        /// Traversal depth
        #[arg(long, default_value = "1")]
        depth: usize,
    },
    /// Show backlinks for a file
    Backlinks {
        /// File path
        file: String,
        /// Max results
        #[arg(long, default_value = "50")]
        max_notes: usize,
    },
    /// List all tags
    Tags {
        /// Show files under a specific tag
        #[arg(long)]
        tag: Option<String>,
        /// Minimum tag count
        #[arg(long, default_value = "1")]
        min_count: usize,
    },
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Set API key for a provider
    SetKey {
        /// Provider name (openai, anthropic, cohere, ollama)
        provider: String,
        /// API key value
        key: String,
    },
    /// Show current configuration
    Show,
    /// Update a config value
    Update {
        /// Config key (e.g. depth, context.budget, search.k, defaults.k)
        key: String,
        /// New value
        value: String,
    },
}

#[derive(Subcommand)]
enum GenerateCmd {
    /// Generate docs for code symbols
    Docs {
        /// Generate for specific file
        #[arg(long)]
        file: Option<String>,
        /// Preview without writing
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate ARCHITECTURE.md
    Architecture {
        /// Preview without writing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum CodemapCmd {
    /// Show codemap summary
    Show {
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Traversal depth
        #[arg(long, default_value = "1")]
        depth: usize,
    },
    /// List symbols
    Symbols {
        /// Filter by file or name
        #[arg(short, long)]
        file: Option<String>,
        /// Filter by kind (function, class, struct, trait, etc.)
        #[arg(long)]
        kind: Option<String>,
        /// Filter by language
        #[arg(long)]
        lang: Option<String>,
        /// Show only exported symbols
        #[arg(long)]
        exported: bool,
        /// Include test symbols
        #[arg(long)]
        include_tests: bool,
        /// Minimum confidence (0-100)
        #[arg(long, default_value = "0")]
        min_confidence: f64,
        /// Max results
        #[arg(long, default_value = "100")]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show imports for a file
    Imports {
        /// File path
        file: String,
        /// Show only resolved imports
        #[arg(long)]
        resolved: bool,
        /// Minimum confidence (0-100)
        #[arg(long, default_value = "0")]
        min_confidence: f64,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show call graph for a symbol
    Calls {
        /// Symbol name
        symbol: String,
        /// Traversal depth
        #[arg(long, default_value = "1")]
        depth: usize,
        /// Minimum confidence (0-100)
        #[arg(long, default_value = "0")]
        min_confidence: f64,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show impacted files
    Impacted {
        /// File path
        file: String,
        /// Traversal depth
        #[arg(long, default_value = "3")]
        depth: usize,
    },
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init { path, name, force, minimal } => {
            commands::init::run()
        }
        Commands::Index { docs_only, code_only, include, exclude, lang, max_files } => {
            commands::index::run()
        }
        Commands::Status { json } => commands::status::run(),
        Commands::Graph { sub } => match sub {
            GraphCmd::Docs { json, file, tag: _, max_notes: _, depth: _ } => {
                commands::graph::run_docs(json, file.as_deref())
            }
            GraphCmd::Backlinks { file, max_notes: _ } => commands::graph::run_backlinks(&file),
            GraphCmd::Tags { tag: _, min_count: _ } => commands::graph::run_tags(),
        },
        Commands::Codemap { sub } => match sub {
            CodemapCmd::Show { json, depth: _ } => commands::codemap::run(json),
            CodemapCmd::Symbols { file, kind: _, lang: _, exported: _, include_tests: _, min_confidence: _, limit: _, json: _ } => {
                commands::codemap::run_symbols(file.as_deref())
            }
            CodemapCmd::Imports { file, resolved: _, min_confidence: _, json: _ } => commands::codemap::run_imports(&file),
            CodemapCmd::Calls { symbol, depth: _, min_confidence: _, json: _ } => commands::codemap::run_calls(&symbol),
            CodemapCmd::Impacted { file, depth: _ } => commands::codemap::run_impacted(&file),
        },
        Commands::Find { query, docs, code, symbol, max_results: _, context_lines: _, case_sensitive: _ } => {
            commands::find::run(&query, docs, code, symbol)
        }
        Commands::Replace { pattern, replacement, regex, preview, r#apply, include: _, exclude: _, lang: _, max_files: _, max_replacements: _ } => {
            commands::replace::run(&pattern, &replacement, regex, preview, r#apply)
        }
        Commands::Context { query, file, symbol, budget: _, max_files: _, max_symbols: _, max_notes: _, depth: _, min_confidence: _ } => {
            commands::context::run(query.as_deref(), file.as_deref(), symbol.as_deref())
        }
        Commands::AgentExport { budget: _, json: _ } => commands::context::run_export(),
        Commands::Doctor { json: _, strict: _ } => commands::doctor::run(),
        Commands::Impact { target, depth, direction: _, max_symbols: _, max_files: _, json } => commands::impact::run(&target, depth, json),
        Commands::Hotspots { limit, min_usage: _, exclude_tests: _, json } => commands::hotspots::run(limit, json),
        Commands::DeadCode { min_confidence: _, limit: _, include_exported: _, exclude_tests: _, json } => commands::deadcode::run(json),
        Commands::History { file, limit: _, json } => commands::history::run(file.as_deref(), json),
        Commands::Watch { debounce } => commands::watch::run(debounce),
        Commands::Workspaces { json: _ } => commands::workspaces::run(),
        Commands::Plugins { sub } => match sub {
            PluginCmd::List => commands::plugins_cmd::run_list(),
            PluginCmd::Install { path } => commands::plugins_cmd::run_install(&path),
            PluginCmd::Uninstall { name } => commands::plugins_cmd::run_uninstall(&name),
        },
        Commands::Config { sub } => match sub {
            ConfigCmd::SetKey { provider, key } => commands::config_cmd::run_set_key(&provider, &key),
            ConfigCmd::Show => commands::config_cmd::run_show(),
            ConfigCmd::Update { key, value } => commands::config_cmd::run_update(&key, &value),
        },
        Commands::Embed { refresh, dry_run: _ } => commands::embed::run(refresh),
        Commands::Search { query, limit: _, min_score: _ } => commands::search::run(&query, 10),
        Commands::Generate { sub } => match sub {
            GenerateCmd::Docs { file, dry_run } => commands::generate::run_docs(file.as_deref(), dry_run),
            GenerateCmd::Architecture { dry_run } => commands::generate::run_architecture(dry_run),
        },
        Commands::Serve => commands::serve::run(),
        Commands::Deps { json: _ } => commands::deps_cmd::run(),
        Commands::Regex { pattern, docs, code, max_results, context_lines: _ } => commands::regex_search::run(&pattern, docs, code, max_results),
        Commands::Visualize { sub } => match sub {
            VisualizeCmd::Docs { output } => commands::visualize::run_docs(output.as_deref()),
            VisualizeCmd::Codemap { output } => commands::visualize::run_codemap(output.as_deref()),
            VisualizeCmd::Callgraph { output } => commands::visualize::run_callgraph(output.as_deref()),
            VisualizeCmd::Impact { target, output } => commands::visualize::run_impact(&target, output.as_deref()),
            VisualizeCmd::Bridge { output } => commands::visualize::run_bridge(output.as_deref()),
            VisualizeCmd::Deps { output } => commands::visualize::run_deps(output.as_deref()),
            VisualizeCmd::All { output } => commands::visualize::run_all(output.as_deref()),
        },
        Commands::Rollback { snapshot_id, preview, apply } => commands::rollback::run(&snapshot_id, preview, apply),
    };

    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}
