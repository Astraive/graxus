//! Shared CLI argument structs used across multiple subcommands.

use clap::Args;

/// Global arguments that apply to all commands (project root, config, output format).
#[derive(Args, Debug, Clone)]
pub struct GlobalArgs {
    /// Project root override
    #[arg(long, short = 'C')]
    pub root: Option<std::path::PathBuf>,
    /// Config file path
    #[arg(long, short = 'c')]
    pub config: Option<std::path::PathBuf>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
    /// Suppress non-essential output
    #[arg(long, short)]
    pub quiet: bool,
    /// Verbose output
    #[arg(long, short)]
    pub verbose: bool,
    /// Disable colors
    #[arg(long)]
    pub no_color: bool,
    /// Command timeout in seconds
    #[arg(long)]
    pub timeout: Option<u64>,
}

/// File filtering arguments (include/exclude globs, language, type filters).
#[derive(Args, Debug, Clone)]
pub struct FileFilterArgs {
    /// Include glob pattern (repeatable)
    #[arg(long)]
    pub include: Vec<String>,
    /// Exclude glob pattern (repeatable)
    #[arg(long)]
    pub exclude: Vec<String>,
    /// Filter by language
    #[arg(long)]
    pub lang: Vec<String>,
    /// Docs only
    #[arg(long)]
    pub docs: bool,
    /// Code only
    #[arg(long)]
    pub code: bool,
    /// Only changed files
    #[arg(long)]
    pub changed: bool,
    /// Maximum files to process
    #[arg(long)]
    pub max_files: Option<usize>,
}

/// Graph traversal arguments (depth, node/edge limits, test filtering).
#[derive(Args, Debug, Clone)]
pub struct TraversalArgs {
    /// Traversal depth
    #[arg(long, default_value = "1")]
    pub depth: usize,
    /// Max nodes returned
    #[arg(long, default_value = "200")]
    pub max_nodes: usize,
    /// Max edges returned
    #[arg(long, default_value = "500")]
    pub max_edges: usize,
    /// Include test symbols
    #[arg(long)]
    pub include_tests: bool,
    /// Exclude test symbols
    #[arg(long)]
    pub exclude_tests: bool,
}

/// Context assembly arguments (budget, limits, confidence thresholds).
#[derive(Args, Debug, Clone)]
pub struct ContextArgs {
    /// Token budget
    #[arg(long, default_value = "12000")]
    pub budget: usize,
    /// Max files in context
    #[arg(long, default_value = "20")]
    pub max_files: usize,
    /// Max symbols in context
    #[arg(long, default_value = "100")]
    pub max_symbols: usize,
    /// Max docs/notes in context
    #[arg(long, default_value = "20")]
    pub max_notes: usize,
    /// Max chars per file snippet
    #[arg(long, default_value = "8000")]
    pub max_chars_per_file: usize,
    /// Include code
    #[arg(long, default_value = "true")]
    pub include_code: bool,
    /// Include docs
    #[arg(long, default_value = "true")]
    pub include_docs: bool,
}

/// Confidence filtering and display arguments.
#[derive(Args, Debug, Clone)]
pub struct ConfidenceArgs {
    /// Minimum confidence percentage (0-100)
    #[arg(long, default_value = "0")]
    pub min_confidence: f64,
    /// Show confidence in output
    #[arg(long)]
    pub show_confidence: bool,
    /// Hide unresolved items
    #[arg(long)]
    pub hide_unresolved: bool,
}

/// Output format and destination arguments.
#[derive(Args, Debug, Clone)]
pub struct OutputArgs {
    /// Output format (table, json, markdown)
    #[arg(long, default_value = "table")]
    pub format: String,
    /// Output to file
    #[arg(long)]
    pub output: Option<std::path::PathBuf>,
}

/// Safety arguments for mutation operations (preview, apply, snapshot).
#[derive(Args, Debug, Clone)]
pub struct SafetyArgs {
    /// Show preview without applying
    #[arg(long)]
    pub preview: bool,
    /// Actually apply changes
    #[arg(long)]
    pub apply: bool,
    /// Create snapshot before changes
    #[arg(long, default_value = "true")]
    pub snapshot: bool,
}

/// LLM/embedding provider arguments (provider, model, endpoint, key).
#[derive(Args, Debug, Clone)]
pub struct ProviderArgs {
    /// Provider name (openai, anthropic, cohere, ollama)
    #[arg(long)]
    pub provider: Option<String>,
    /// Model name
    #[arg(long)]
    pub model: Option<String>,
    /// API endpoint
    #[arg(long)]
    pub endpoint: Option<String>,
    /// API key env var name
    #[arg(long)]
    pub api_key_env: Option<String>,
}
