pub mod args;
mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "graxus", version = "0.1.0", about = "AI-native codebase knowledge engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new graxus project
    Init,
    /// Index the project (scan files, build graph and codemap)
    Index,
    /// Show project status
    Status,
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
    /// Search the project
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
    },
    /// Export agent context
    AgentExport,
    /// Run health diagnostics
    Doctor,
    /// Show blast radius of file changes
    Impact {
        /// File to analyze
        file: String,
        /// Max traversal depth
        #[arg(long, default_value = "3")]
        depth: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show most-called symbols (hotspots)
    Hotspots {
        /// Number of results
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show potentially dead code (uncalled symbols)
    DeadCode {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show edit history
    History {
        /// Filter by file path
        #[arg(long)]
        file: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Watch for file changes and auto-reindex
    Watch {
        /// Debounce seconds
        #[arg(long, default_value = "2")]
        debounce: u64,
    },
    /// Detect and list workspaces in the project
    Workspaces,
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
    },
    /// Semantic search using embeddings
    Search {
        /// Search query
        query: String,
        /// Number of results to return
        #[arg(long, default_value = "10")]
        top_k: usize,
    },
    /// Generate documentation with LLM
    Generate {
        #[command(subcommand)]
        sub: GenerateCmd,
    },
    /// Start JSON-RPC server on stdio
    Serve,
    /// List detected dependencies
    Deps,
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
    },
    /// Show backlinks for a file
    Backlinks {
        /// File path
        file: String,
    },
    /// List all tags
    Tags,
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
        /// Config key (e.g. depth, context.budget, search.k)
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
    /// Show codemap
    Show {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// List symbols
    Symbols {
        /// Filter by file
        #[arg(short, long)]
        file: Option<String>,
        /// Minimum confidence score (0-100)
        #[arg(long, default_value = "0")]
        min_confidence: f64,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show imports for a file
    Imports {
        /// File path
        file: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show call graph for a symbol
    Calls {
        /// Symbol name
        symbol: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show impacted files
    Impacted {
        /// File path
        file: String,
    },
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => commands::init::run(),
        Commands::Index => commands::index::run(),
        Commands::Status => commands::status::run(),
        Commands::Graph { sub } => match sub {
            GraphCmd::Docs { json, file } => {
                commands::graph::run_docs(json, file.as_deref())
            }
            GraphCmd::Backlinks { file } => commands::graph::run_backlinks(&file),
            GraphCmd::Tags => commands::graph::run_tags(),
        },
        Commands::Codemap { sub } => match sub {
            CodemapCmd::Show { json } => commands::codemap::run(json),
            CodemapCmd::Symbols { file, min_confidence: _, json: _ } => {
                commands::codemap::run_symbols(file.as_deref())
            }
            CodemapCmd::Imports { file, json: _ } => commands::codemap::run_imports(&file),
            CodemapCmd::Calls { symbol, json: _ } => {
                println!("Call graph for: {}", symbol);
                Ok(())
            }
            CodemapCmd::Impacted { file } => commands::codemap::run_impacted(&file),
        },
        Commands::Find { query, docs, code, symbol } => {
            commands::find::run(&query, docs, code, symbol)
        }
        Commands::Replace { pattern, replacement, regex, preview, r#apply } => {
            commands::replace::run(&pattern, &replacement, regex, preview, r#apply)
        }
        Commands::Context { query, file, symbol } => {
            commands::context::run(query.as_deref(), file.as_deref(), symbol.as_deref())
        }
        Commands::AgentExport => commands::context::run_export(),
        Commands::Doctor => commands::doctor::run(),
        Commands::Impact { file, depth, json } => commands::impact::run(&file, depth, json),
        Commands::Hotspots { limit, json } => commands::hotspots::run(limit, json),
        Commands::DeadCode { json } => commands::deadcode::run(json),
        Commands::History { file, json } => commands::history::run(file.as_deref(), json),
        Commands::Watch { debounce } => commands::watch::run(debounce),
        Commands::Workspaces => commands::workspaces::run(),
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
        Commands::Embed { refresh } => commands::embed::run(refresh),
        Commands::Search { query, top_k } => commands::search::run(&query, top_k),
        Commands::Generate { sub } => match sub {
            GenerateCmd::Docs { file, dry_run } => commands::generate::run_docs(file.as_deref(), dry_run),
            GenerateCmd::Architecture { dry_run } => commands::generate::run_architecture(dry_run),
        },
        Commands::Serve => commands::serve::run(),
        Commands::Deps => commands::deps_cmd::run(),
        Commands::Regex { pattern, docs, code, max_results } => commands::regex_search::run(&pattern, docs, code, max_results),
        Commands::Rollback { snapshot_id, preview, apply } => commands::rollback::run(&snapshot_id, preview, apply),
    };

    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}
