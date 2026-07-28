pub mod args;
mod commands;
pub mod context;
mod errors;
pub mod filters;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use args::GlobalArgs;
use context::CliContext;

/// Top-level CLI parser for the graxus command.
#[derive(Parser)]
#[command(name = "graxus", version = env!("CARGO_PKG_VERSION"), about = "AI-native codebase knowledge engine")]
struct Cli {
    #[command(flatten)]
    global: GlobalArgs,

    #[command(subcommand)]
    command: Commands,
}

/// All available subcommands for the graxus CLI.
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
        /// Codemap parser backend: ripex (default), tree-sitter, or auto
        #[arg(long, default_value = "ripex")]
        codemap_backend: String,
    },

    /// Show project status
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Incremental update — only re-index changed files
    Update {
        /// Force full re-index
        #[arg(long)]
        full: bool,
        /// Show what would change without updating
        #[arg(long)]
        dry_run: bool,
        /// Codemap parser backend: ripex (default), tree-sitter, or auto
        #[arg(long, default_value = "ripex")]
        codemap_backend: String,
    },

    /// Show what changed since last index
    Diff {
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

    /// Show indexed HTTP routes and framework endpoints
    Routes {
        /// Filter by framework
        #[arg(long)]
        framework: Option<String>,
        /// Filter by language
        #[arg(long)]
        lang: Option<String>,
        /// Max results
        #[arg(long, default_value = "100")]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show indexed symbols across the codebase
    Symbols {
        /// Filter by file path or symbol name
        #[arg(short, long)]
        file: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show type relationships and DI bindings
    Types {
        /// Filter by interface, trait, or concrete type
        #[arg(long)]
        name: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
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

    /// Index all sub-projects in a workspace
    IndexAll,

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
        /// Search mode: vector, keyword, or hybrid
        #[arg(long, default_value = "hybrid")]
        mode: String,
        /// Filter results to a specific file path
        #[arg(long)]
        file: Option<String>,
        /// Search across all workspace sub-projects
        #[arg(long)]
        workspace: bool,
    },

    /// Generate documentation with LLM
    Generate {
        #[command(subcommand)]
        sub: GenerateCmd,
    },

    /// Start JSON-RPC server on stdio
    Serve,

    /// Start LSP server on stdio (for editor integration)
    Lsp,

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

    /// Remove .graxus/ directory and all index data
    Clean {
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
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

    /// Generate shell completions
    Completions {
        /// Shell type (bash, zsh, fish, powershell)
        shell: String,
        /// Output file path (stdout if omitted)
        #[arg(short, long)]
        output: Option<String>,
    },
}

/// Subcommands for the `visualize` command.
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

/// Subcommands for the `plugins` command.
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

/// Subcommands for the `graph` command.
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
    /// Export graph data to file
    Export {
        /// Output format (json, csv, markdown)
        #[arg(long, default_value = "json")]
        format: String,
        /// Custom save path
        #[arg(short, long)]
        path: Option<String>,
        /// Save to .graxus/exports/ with auto-generated filename
        #[arg(long)]
        save: bool,
    },
}

/// Subcommands for the `config` command.
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

/// Subcommands for the `generate` command.
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
        /// Ignore cache and regenerate everything
        #[arg(long)]
        force: bool,
    },
    /// Generate ARCHITECTURE.md
    Architecture {
        /// Preview without writing
        #[arg(long)]
        dry_run: bool,
    },
}

/// Subcommands for the `codemap` command.
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
    /// Export codemap data to file
    Export {
        /// Output format (json, csv, markdown)
        #[arg(long, default_value = "json")]
        format: String,
        /// Custom save path
        #[arg(short, long)]
        path: Option<String>,
        /// Save to .graxus/exports/ with auto-generated filename
        #[arg(long)]
        save: bool,
    },
}

fn main() {
    // The graxus CLI derives a large clap command tree (30+ subcommands, several
    // with their own nested subcommand enums). On Windows debug builds the
    // unoptimized drop/check glue for `Cli::parse()` is deep enough to overflow
    // the default 1 MiB main-thread stack before any command logic runs — every
    // invocation (even `--help`) crashes. Release builds are unaffected.
    //
    // Running the real entry point on a worker thread with a larger stack fixes
    // debug builds and insulates us against future growth of the derive tree.
    let worker = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(real_main)
        .expect("failed to spawn graxus worker thread");

    let exit_code = match worker.join() {
        Ok(code) => code,
        Err(_) => {
            // The worker panicked; the panic hook already printed the message.
            101
        }
    };
    std::process::exit(exit_code);
}

/// Real entry point: parses the CLI, initializes tracing (honoring `--verbose`),
/// dispatches the command, and maps the result into a process exit code.
fn real_main() -> i32 {
    let cli = Cli::parse();

    // Initialize tracing. `RUST_LOG` wins if set; otherwise `--verbose` selects
    // `debug` and the default is `warn` (only warnings/errors from the engine).
    let default_level = if cli.global.verbose { "debug" } else { "warn" };
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();

    // Build the shared runtime context from global args. This wires
    // --root / --config / --quiet / --verbose / --no-color / --timeout
    // into every command instead of letting them be parsed-and-dropped.
    let ctx = CliContext::from_global(&cli.global);

    let result = dispatch(cli.command, &ctx);

    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {:#}", e);
            errors::classify_error(&e)
        }
    }
}

/// Dispatch a parsed subcommand using the shared runtime context.
fn dispatch(command: Commands, ctx: &CliContext) -> anyhow::Result<()> {
    match command {
        Commands::Init {
            path,
            name,
            force,
            minimal,
        } => commands::init::run(ctx, &path, name.as_deref(), force, minimal),
        Commands::Index {
            docs_only,
            code_only,
            include,
            exclude,
            lang,
            max_files,
            codemap_backend,
        } => commands::index::run(
            ctx,
            docs_only,
            code_only,
            include,
            exclude,
            lang,
            max_files,
            codemap_backend,
        ),
        Commands::Status { json } => commands::status::run(ctx, json),
        Commands::Update {
            full,
            dry_run,
            codemap_backend,
        } => commands::update::run(ctx, dry_run, full, codemap_backend),
        Commands::Diff { json } => commands::diff::run(ctx, json),
        Commands::Graph { sub } => match sub {
            GraphCmd::Docs {
                json,
                file,
                tag,
                max_notes,
                depth: _,
            } => commands::graph::run_docs(ctx, json, file.as_deref(), tag.as_deref(), max_notes),
            GraphCmd::Backlinks { file, max_notes } => {
                commands::graph::run_backlinks(ctx, &file, max_notes)
            }
            GraphCmd::Tags { tag, min_count } => {
                commands::graph::run_tags(ctx, tag.as_deref(), min_count)
            }
            GraphCmd::Export { format, path, save } => {
                commands::graph::run_export(ctx, &format, path.as_deref(), save)
            }
        },
        Commands::Codemap { sub } => match sub {
            CodemapCmd::Show { json, depth: _ } => commands::codemap::run(ctx, json),
            CodemapCmd::Symbols {
                file,
                kind,
                lang,
                exported,
                include_tests,
                min_confidence,
                limit,
                json,
            } => commands::codemap::run_symbols(
                ctx,
                &commands::codemap::SymbolFilter {
                    file,
                    kind,
                    lang,
                    exported,
                    include_tests,
                    _min_confidence: min_confidence,
                    limit,
                    json,
                },
            ),
            CodemapCmd::Imports {
                file,
                resolved,
                min_confidence,
                json,
            } => commands::codemap::run_imports(ctx, &file, resolved, min_confidence, json),
            CodemapCmd::Calls {
                symbol,
                depth,
                min_confidence,
                json,
            } => commands::codemap::run_calls(ctx, &symbol, depth, min_confidence, json),
            CodemapCmd::Impacted { file, depth } => {
                commands::codemap::run_impacted(ctx, &file, depth)
            }
            CodemapCmd::Export { format, path, save } => {
                commands::codemap::run_export(ctx, &format, path.as_deref(), save)
            }
        },
        Commands::Find {
            query,
            docs,
            code,
            symbol,
            max_results,
            context_lines,
            case_sensitive,
        } => commands::find::run(
            ctx,
            &query,
            docs,
            code,
            symbol,
            max_results,
            context_lines,
            case_sensitive,
        ),
        Commands::Routes {
            framework,
            lang,
            limit,
            json,
        } => commands::routes::run(ctx, framework.as_deref(), lang.as_deref(), limit, json),
        Commands::Symbols { file, json } => commands::symbols::run(ctx, file.as_deref(), json),
        Commands::Types { name, json } => commands::types::run(ctx, name.as_deref(), json),
        Commands::Replace {
            pattern,
            replacement,
            regex,
            preview,
            r#apply,
            include,
            exclude,
            lang,
            max_files,
            max_replacements,
        } => commands::replace::run(
            ctx,
            &pattern,
            &replacement,
            regex,
            preview,
            r#apply,
            include,
            exclude,
            lang,
            max_files,
            max_replacements,
        ),
        Commands::Context {
            query,
            file,
            symbol,
            budget,
            max_files,
            max_symbols,
            max_notes,
            depth,
            min_confidence,
        } => commands::context::run(
            ctx,
            query.as_deref(),
            file.as_deref(),
            symbol.as_deref(),
            budget,
            max_files,
            max_symbols,
            max_notes,
            depth,
            min_confidence,
        ),
        Commands::AgentExport { budget: _, json: _ } => commands::context::run_export(ctx),
        Commands::Doctor { json, strict } => commands::doctor::run(ctx, json, strict),
        Commands::Impact {
            target,
            depth,
            direction,
            max_symbols,
            max_files,
            json,
        } => commands::impact::run(
            ctx,
            &target,
            depth,
            &direction,
            max_symbols,
            max_files,
            json,
        ),
        Commands::Hotspots {
            limit,
            min_usage,
            exclude_tests,
            json,
        } => commands::hotspots::run(ctx, limit, min_usage, exclude_tests, json),
        Commands::DeadCode {
            min_confidence,
            limit,
            include_exported,
            exclude_tests,
            json,
        } => commands::deadcode::run(
            ctx,
            min_confidence,
            limit,
            include_exported,
            exclude_tests,
            json,
        ),
        Commands::History { file, limit, json } => {
            commands::history::run(ctx, file.as_deref(), limit, json)
        }
        Commands::Watch { debounce } => commands::watch::run(ctx, debounce),
        Commands::Workspaces { json } => commands::workspaces::run(ctx, json),
        Commands::IndexAll => commands::workspaces::run_index_all_cli(ctx),
        Commands::Plugins { sub } => match sub {
            PluginCmd::List => commands::plugins_cmd::run_list(ctx),
            PluginCmd::Install { path } => commands::plugins_cmd::run_install(ctx, &path),
            PluginCmd::Uninstall { name } => commands::plugins_cmd::run_uninstall(ctx, &name),
        },
        Commands::Config { sub } => match sub {
            ConfigCmd::SetKey { provider, key } => {
                commands::config_cmd::run_set_key(ctx, &provider, &key)
            }
            ConfigCmd::Show => commands::config_cmd::run_show(ctx),
            ConfigCmd::Update { key, value } => commands::config_cmd::run_update(ctx, &key, &value),
        },
        Commands::Embed { refresh, dry_run } => commands::embed::run(ctx, refresh, dry_run),
        Commands::Search {
            query,
            limit,
            min_score,
            mode,
            file,
            workspace,
        } => {
            if workspace {
                commands::search::run_workspace(ctx, &query, limit, min_score)
            } else {
                commands::search::run(ctx, &query, limit, min_score, &mode, file.as_deref())
            }
        }
        Commands::Generate { sub } => match sub {
            GenerateCmd::Docs {
                file,
                dry_run,
                force,
            } => commands::generate::run_docs(ctx, file.as_deref(), dry_run, force),
            GenerateCmd::Architecture { dry_run } => {
                commands::generate::run_architecture(ctx, dry_run)
            }
        },
        Commands::Serve => commands::serve::run(ctx),
        Commands::Lsp => commands::serve::run_lsp(ctx),
        Commands::Deps { json } => commands::deps_cmd::run(ctx, json),
        Commands::Regex {
            pattern,
            docs,
            code,
            max_results,
            context_lines,
        } => commands::regex_search::run(ctx, &pattern, docs, code, max_results, context_lines),
        Commands::Visualize { sub } => match sub {
            VisualizeCmd::Docs { output } => commands::visualize::run_docs(ctx, output.as_deref()),
            VisualizeCmd::Codemap { output } => {
                commands::visualize::run_codemap(ctx, output.as_deref())
            }
            VisualizeCmd::Callgraph { output } => {
                commands::visualize::run_callgraph(ctx, output.as_deref())
            }
            VisualizeCmd::Impact { target, output } => {
                commands::visualize::run_impact(ctx, &target, output.as_deref())
            }
            VisualizeCmd::Bridge { output } => {
                commands::visualize::run_bridge(ctx, output.as_deref())
            }
            VisualizeCmd::Deps { output } => commands::visualize::run_deps(ctx, output.as_deref()),
            VisualizeCmd::All { output } => commands::visualize::run_all(ctx, output.as_deref()),
        },
        Commands::Clean { force } => commands::clean::run(ctx, force),
        Commands::Rollback {
            snapshot_id,
            preview,
            apply,
        } => commands::rollback::run(ctx, &snapshot_id, preview, apply),
        Commands::Completions { shell, output } => {
            commands::completions::run(ctx, &shell, output.as_deref())
        }
    }
}
