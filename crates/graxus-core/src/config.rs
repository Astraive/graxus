use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Selects which parser backend `graxus-codemap` uses to extract facts from source code.
///
/// - [`Ripex`](ParserBackend::Ripex): prefer the sibling `ripex` crate (hand-written
///   recursive-descent parser). Falls back to tree-sitter per-file on failure or when a
///   language is unsupported. **This is the default.**
/// - [`TreeSitter`](ParserBackend::TreeSitter): use tree-sitter exclusively (the previous
///   behavior).
/// - [`Auto`](ParserBackend::Auto): same as [`Ripex`](ParserBackend::Ripex) — prefer ripex,
///   fall back to tree-sitter. Kept as an explicit alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParserBackend {
    #[default]
    Ripex,
    TreeSitter,
    Auto,
}

impl ParserBackend {
    /// `ripex` / `tree-sitter` / `auto`
    pub fn as_str(&self) -> &'static str {
        match self {
            ParserBackend::Ripex => "ripex",
            ParserBackend::TreeSitter => "tree-sitter",
            ParserBackend::Auto => "auto",
        }
    }

    /// The backend actually used for a concrete file. `Auto` resolves to `Ripex`.
    pub fn effective(&self) -> ParserBackend {
        match self {
            ParserBackend::Auto => ParserBackend::Ripex,
            other => *other,
        }
    }
}

impl FromStr for ParserBackend {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ripex" | "rip" => Ok(ParserBackend::Ripex),
            "tree-sitter" | "treesitter" | "tree_sitter" | "ts" => Ok(ParserBackend::TreeSitter),
            "auto" => Ok(ParserBackend::Auto),
            other => {
                anyhow::bail!("unknown parser backend: {other} (expected ripex|tree-sitter|auto)")
            }
        }
    }
}

/// Top-level configuration loaded from `graxus.yaml` with environment variable overrides.
///
/// Precedence: **CLI flags > env vars (`GRAXUS_*`) > config file > built-in defaults**.
///
/// [`load`](Self::load) automatically calls [`apply_env_overrides`](Self::apply_env_overrides)
/// after reading the config file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraxusConfig {
    #[serde(default)]
    pub project: ProjectConfig,
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub docs: DocsConfig,
    #[serde(default)]
    pub code: CodeConfig,
    #[serde(default)]
    pub index: IndexConfig,
    #[serde(default)]
    pub edit: EditConfig,
    #[serde(default)]
    pub embeddings: EmbeddingsConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub context: ContextConfig,
    #[serde(default)]
    pub impact: ImpactConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub graph: GraphConfig,
    #[serde(default)]
    pub codemap: CodemapConfig,
    #[serde(default)]
    pub doctor: DoctorConfig,
}

/// Project metadata (name and root path).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Human-readable project name. Overridable via `GRAXUS_PROJECT_NAME`.
    pub name: String,
    /// Root directory of the project. Overridable via `GRAXUS_PROJECT_ROOT`.
    pub root: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        let dir_name = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "project".into());
        Self {
            name: dir_name,
            root: ".".into(),
        }
    }
}

/// File scanning configuration (include/exclude patterns, gitignore).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    /// Whether to respect `.gitignore` rules when scanning. Overridable via `GRAXUS_SCAN_RESPECT_GITIGNORE`.
    #[serde(default = "default_true")]
    pub respect_gitignore: bool,
    #[serde(default = "default_include")]
    pub include: Vec<String>,
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            include: default_include(),
            exclude: default_exclude(),
        }
    }
}

/// Documentation generation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocsConfig {
    /// Whether documentation processing is enabled. Overridable via `GRAXUS_DOCS_ENABLED`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub obsidian_compatible: bool,
    #[serde(default = "default_doc_extensions")]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub parse: DocParseConfig,
}

impl Default for DocsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            obsidian_compatible: true,
            extensions: default_doc_extensions(),
            parse: DocParseConfig::default(),
        }
    }
}

/// Document parsing options (frontmatter, links, tags).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocParseConfig {
    #[serde(default = "default_true")]
    pub frontmatter: bool,
    #[serde(default = "default_true")]
    pub wiki_links: bool,
    #[serde(default = "default_true")]
    pub markdown_links: bool,
    #[serde(default = "default_true")]
    pub tags: bool,
    #[serde(default = "default_true")]
    pub headings: bool,
    #[serde(default = "default_true")]
    pub backlinks: bool,
}

/// Code analysis configuration (parser, languages).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeConfig {
    /// Whether code analysis is enabled. Overridable via `GRAXUS_CODE_ENABLED`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_parser")]
    pub parser: String,
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
}

impl Default for CodeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            parser: default_parser(),
            languages: default_languages(),
        }
    }
}

/// Index storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    #[serde(default = "default_storage")]
    pub storage: String,
    #[serde(default = "default_index_path")]
    pub path: String,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            storage: default_storage(),
            path: default_index_path(),
        }
    }
}

/// File editing safety configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditConfig {
    #[serde(default = "default_true")]
    pub create_snapshots: bool,
    #[serde(default = "default_true")]
    pub require_preview_for_replace: bool,
    /// Maximum files touched per replace operation. Overridable via `GRAXUS_EDIT_MAX_FILES`.
    #[serde(default = "default_max_files")]
    pub max_files_per_operation: usize,
}

impl Default for EditConfig {
    fn default() -> Self {
        Self {
            create_snapshots: true,
            require_preview_for_replace: true,
            max_files_per_operation: default_max_files(),
        }
    }
}

/// Embedding provider configuration (OpenAI, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsConfig {
    /// Whether embeddings are enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Embedding provider name. Overridable via `GRAXUS_EMBED_PROVIDER`.
    #[serde(default = "default_embed_provider")]
    pub provider: String,
    /// Embedding model name. Overridable via `GRAXUS_EMBED_MODEL`.
    #[serde(default = "default_embed_model")]
    pub model: String,
    /// Name of the environment variable that holds the API key.
    /// When `GRAXUS_EMBED_API_KEY` is set, this is overwritten to
    /// `"GRAXUS_EMBED_API_KEY"` so that [`api_key`](Self::api_key) returns it.
    #[serde(default)]
    pub api_key_env: String,
    #[serde(default = "default_embed_dims")]
    pub dimensions: usize,
    #[serde(default = "default_embed_batch")]
    pub batch_size: usize,
    #[serde(default)]
    pub endpoint: Option<String>,
}

impl EmbeddingsConfig {
    /// Retrieve the API key from the environment variable specified by `api_key_env`.
    pub fn api_key(&self) -> Option<String> {
        if self.api_key_env.is_empty() {
            return None;
        }
        std::env::var(&self.api_key_env).ok()
    }
}

impl Default for EmbeddingsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_embed_provider(),
            model: default_embed_model(),
            api_key_env: String::new(),
            dimensions: default_embed_dims(),
            batch_size: default_embed_batch(),
            endpoint: None,
        }
    }
}

/// LLM provider configuration (model, tokens, temperature).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Whether LLM integration is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// LLM provider name. Overridable via `GRAXUS_LLM_PROVIDER`.
    #[serde(default = "default_llm_provider")]
    pub provider: String,
    /// LLM model name. Overridable via `GRAXUS_LLM_MODEL`.
    #[serde(default = "default_llm_model")]
    pub model: String,
    /// Name of the environment variable that holds the API key.
    /// When `GRAXUS_LLM_API_KEY` is set, this is overwritten to
    /// `"GRAXUS_LLM_API_KEY"` so that [`api_key`](Self::api_key) returns it.
    #[serde(default)]
    pub api_key_env: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_max_cost")]
    pub max_cost_per_run: f64,
    #[serde(default)]
    pub endpoint: Option<String>,
}

impl LlmConfig {
    /// Retrieve the API key from the environment variable specified by `api_key_env`.
    pub fn api_key(&self) -> Option<String> {
        if self.api_key_env.is_empty() {
            return None;
        }
        std::env::var(&self.api_key_env).ok()
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_llm_provider(),
            model: default_llm_model(),
            api_key_env: String::new(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            max_cost_per_run: default_max_cost(),
            endpoint: None,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_include() -> Vec<String> {
    vec![
        "**/*.md".into(),
        "**/*.mdx".into(),
        "**/*.txt".into(),
        "**/*.rs".into(),
        "**/*.ts".into(),
        "**/*.tsx".into(),
        "**/*.js".into(),
        "**/*.jsx".into(),
        "**/*.go".into(),
        "**/*.py".into(),
        "**/*.c".into(),
        "**/*.h".into(),
        "**/*.cpp".into(),
        "**/*.hpp".into(),
        "**/*.cs".into(),
        "**/*.java".into(),
        "**/*.kt".into(),
        "**/*.kts".into(),
        "**/*.swift".into(),
    ]
}

fn default_exclude() -> Vec<String> {
    vec![
        "node_modules/**".into(),
        "target/**".into(),
        "dist/**".into(),
        "build/**".into(),
        ".git/**".into(),
        ".graxus/**".into(),
    ]
}

fn default_doc_extensions() -> Vec<String> {
    vec![".md".into(), ".mdx".into()]
}

fn default_parser() -> String {
    "tree-sitter".into()
}

fn default_languages() -> Vec<String> {
    vec![
        "rust".into(),
        "typescript".into(),
        "javascript".into(),
        "go".into(),
        "python".into(),
        "c".into(),
        "cpp".into(),
        "csharp".into(),
        "java".into(),
        "kotlin".into(),
        "swift".into(),
    ]
}

fn default_storage() -> String {
    "json".into()
}

fn default_index_path() -> String {
    ".graxus/index.db".into()
}

fn default_max_files() -> usize {
    100
}

fn default_embed_provider() -> String {
    "openai".into()
}

fn default_embed_model() -> String {
    "text-embedding-3-small".into()
}

fn default_embed_dims() -> usize {
    1536
}

fn default_embed_batch() -> usize {
    64
}

fn default_llm_provider() -> String {
    "openai".into()
}

fn default_llm_model() -> String {
    "gpt-4o-mini".into()
}

fn default_max_tokens() -> usize {
    4096
}

fn default_temperature() -> f64 {
    0.3
}

fn default_max_cost() -> f64 {
    1.0
}

fn default_d_depth() -> usize {
    2
}

fn default_d_max_depth() -> usize {
    4
}

fn default_d_k() -> usize {
    20
}

fn default_d_max_files() -> usize {
    100
}

fn default_d_max_symbols() -> usize {
    500
}

fn default_d_max_notes() -> usize {
    30
}

fn default_d_max_nodes() -> usize {
    500
}

fn default_d_max_edges() -> usize {
    1000
}

fn default_d_min_confidence() -> f64 {
    50.0
}

fn default_d_context_budget() -> usize {
    12000
}

fn default_d_max_chars_per_file() -> usize {
    8000
}

// ── v0.3 Config Sections ──────────────────────────────────────────────

/// Default parameter values for all graxus commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsConfig {
    #[serde(default = "default_d_depth")]
    pub depth: usize,
    #[serde(default = "default_d_max_depth")]
    pub max_depth: usize,
    #[serde(default = "default_d_k")]
    pub k: usize,
    /// Default max files. Overridable via `GRAXUS_INDEX_MAX_FILES`.
    #[serde(default = "default_d_max_files")]
    pub max_files: usize,
    #[serde(default = "default_d_max_symbols")]
    pub max_symbols: usize,
    #[serde(default = "default_d_max_notes")]
    pub max_notes: usize,
    #[serde(default = "default_d_max_nodes")]
    pub max_nodes: usize,
    #[serde(default = "default_d_max_edges")]
    pub max_edges: usize,
    #[serde(default = "default_d_min_confidence")]
    pub min_confidence: f64,
    /// Default context token budget. Overridable via `GRAXUS_CONTEXT_MAX_TOKENS` or `GRAXUS_CONTEXT_BUDGET`.
    #[serde(default = "default_d_context_budget")]
    pub context_budget: usize,
    #[serde(default = "default_d_max_chars_per_file")]
    pub max_chars_per_file: usize,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            depth: default_d_depth(),
            max_depth: default_d_max_depth(),
            k: default_d_k(),
            max_files: default_d_max_files(),
            max_symbols: default_d_max_symbols(),
            max_notes: default_d_max_notes(),
            max_nodes: default_d_max_nodes(),
            max_edges: default_d_max_edges(),
            min_confidence: default_d_min_confidence(),
            context_budget: default_d_context_budget(),
            max_chars_per_file: default_d_max_chars_per_file(),
        }
    }
}

impl DefaultsConfig {
    /// Resolve depth: CLI value > section value > global default.
    pub fn resolve_depth(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.depth)
    }
    /// Resolve k (top-K results): CLI value > section value > global default.
    pub fn resolve_k(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.k)
    }
    /// Resolve max files: CLI value > section value > global default.
    pub fn resolve_max_files(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_files)
    }
    /// Resolve max symbols: CLI value > section value > global default.
    pub fn resolve_max_symbols(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_symbols)
    }
    /// Resolve max notes: CLI value > section value > global default.
    pub fn resolve_max_notes(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_notes)
    }
    /// Resolve max nodes: CLI value > section value > global default.
    pub fn resolve_max_nodes(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_nodes)
    }
    /// Resolve max edges: CLI value > section value > global default.
    pub fn resolve_max_edges(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_edges)
    }
    /// Resolve min confidence: CLI value > section value > global default.
    pub fn resolve_min_confidence(&self, cli: Option<f64>, section: Option<f64>) -> f64 {
        cli.or(section).unwrap_or(self.min_confidence)
    }
    /// Resolve context budget: CLI value > section value > global default.
    pub fn resolve_context_budget(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.context_budget)
    }
    /// Resolve max chars per file: CLI value > section value > global default.
    pub fn resolve_max_chars_per_file(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_chars_per_file)
    }
}

/// Per-command overrides for the `context` command.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextConfig {
    pub depth: Option<usize>,
    /// Token budget for context assembly. Overridable via `GRAXUS_CONTEXT_BUDGET`.
    pub budget: Option<usize>,
    pub k: Option<usize>,
    pub max_files: Option<usize>,
    pub max_symbols: Option<usize>,
    pub max_notes: Option<usize>,
    pub max_edges: Option<usize>,
    pub max_calls: Option<usize>,
    pub max_imports: Option<usize>,
    pub max_chars_per_file: Option<usize>,
    pub include_code: Option<bool>,
    pub include_docs: Option<bool>,
    pub include_graph: Option<bool>,
    pub include_tests: Option<bool>,
    pub min_confidence: Option<f64>,
}

/// Per-command overrides for the `impact` command.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImpactConfig {
    pub depth: Option<usize>,
    pub max_files: Option<usize>,
    pub max_symbols: Option<usize>,
    pub max_notes: Option<usize>,
    pub min_confidence: Option<f64>,
    pub include_docs: Option<bool>,
    pub include_tests: Option<bool>,
}

/// Per-command overrides for the `search` command.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchConfig {
    /// Top-K results to return. Overridable via `GRAXUS_SEARCH_K`.
    pub k: Option<usize>,
    pub max_results: Option<usize>,
    pub min_score: Option<f64>,
    pub hybrid: Option<bool>,
    pub include_snippets: Option<bool>,
}

/// Per-command overrides for the `graph` command.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphConfig {
    pub depth: Option<usize>,
    pub max_notes: Option<usize>,
    pub max_nodes: Option<usize>,
    pub max_edges: Option<usize>,
    pub include_backlinks: Option<bool>,
    pub include_orphans: Option<bool>,
}

/// Selects which parser backend `graxus-codemap` uses when extracting
/// symbols/imports/calls/variables from source files.
///
/// - `Ripex`      — use the external `ripex` hand-written parser (default).
/// - `TreeSitter` — use the built-in tree-sitter extractors (fallback).
/// - `Auto`        — ripex where supported, tree-sitter otherwise.
///
/// ripex covers 8 languages (js/ts, python, go, rust, c, cpp, csharp);
/// tree-sitter is always used as a runtime fallback on parse failure or for
/// unsupported languages (java/kotlin/swift, markdown, etc.).

/// Per-command overrides for the `codemap` command.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodemapConfig {
    pub depth: Option<usize>,
    pub max_files: Option<usize>,
    pub max_symbols: Option<usize>,
    pub max_imports: Option<usize>,
    pub max_calls: Option<usize>,
    pub include_tests: Option<bool>,
    pub min_confidence: Option<f64>,
    /// Parser backend for codemap extraction. `None` => default (`ripex`).
    #[serde(default)]
    pub parser_backend: Option<String>,
}

/// Per-command overrides for the `doctor` command.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DoctorConfig {
    pub min_import_resolution: Option<f64>,
    pub min_call_resolution: Option<f64>,
    pub max_parse_errors: Option<usize>,
    pub max_broken_links: Option<usize>,
}

// ── Tests for v0.3 config ─────────────────────────────────────────────

impl GraxusConfig {
    /// Load config from `graxus.yaml` in the given directory, then apply
    /// `GRAXUS_*` environment variable overrides.
    ///
    /// If no `graxus.yaml` exists, built-in defaults are used.
    pub fn load(root: &Path) -> Result<Self> {
        let config_path = root.join("graxus.yaml");
        let mut config = if !config_path.exists() {
            tracing::debug!("No graxus.yaml found, using defaults");
            Self::default()
        } else {
            let contents = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read {}", config_path.display()))?;
            serde_yaml::from_str(&contents)
                .with_context(|| format!("Failed to parse {}", config_path.display()))?
        };
        config.apply_env_overrides();
        Ok(config)
    }

    /// Resolve the parser backend used for codemap extraction.
    ///
    /// Precedence: config file `codemap.parser_backend` (already merged with the
    /// `GRAXUS_CODEMAP_PARSER_BACKEND` env override) > built-in default (`ripex`).
    /// `Auto` is normalized to `ripex`.
    pub fn codemap_backend(&self) -> ParserBackend {
        match self.codemap.parser_backend.as_deref() {
            Some(s) => s.parse::<ParserBackend>().unwrap_or_default(),
            None => ParserBackend::default(),
        }
        .effective()
    }

    /// Save config to `graxus.yaml` in the given directory.
    pub fn save(&self, root: &Path) -> Result<()> {
        let config_path = root.join("graxus.yaml");
        let contents = serde_yaml::to_string(self).context("Failed to serialize config")?;
        std::fs::write(&config_path, contents)
            .with_context(|| format!("Failed to write {}", config_path.display()))?;
        Ok(())
    }

    /// Get the `.graxus` directory path relative to root.
    pub fn graxus_dir(&self, root: &Path) -> PathBuf {
        root.join(".graxus")
    }

    /// Apply `GRAXUS_*` environment variable overrides to this config.
    ///
    /// Only fields whose corresponding env var is set are overridden.
    /// `GRAXUS_LOG_LEVEL` is intentionally not handled here -- it is consumed
    /// directly by the tracing subscriber at startup.
    pub fn apply_env_overrides(&mut self) {
        // String fields
        if let Ok(v) = std::env::var("GRAXUS_PROJECT_NAME") {
            self.project.name = v;
        }
        if let Ok(v) = std::env::var("GRAXUS_PROJECT_ROOT") {
            self.project.root = v;
        }
        if let Ok(v) = std::env::var("GRAXUS_EMBED_PROVIDER") {
            self.embeddings.provider = v;
        }
        if let Ok(v) = std::env::var("GRAXUS_EMBED_MODEL") {
            self.embeddings.model = v;
        }
        if let Ok(v) = std::env::var("GRAXUS_LLM_PROVIDER") {
            self.llm.provider = v;
        }
        if let Ok(v) = std::env::var("GRAXUS_LLM_MODEL") {
            self.llm.model = v;
        }

        // API keys -- store the env var name so api_key() reads the value at runtime
        if std::env::var("GRAXUS_EMBED_API_KEY").is_ok() {
            self.embeddings.api_key_env = "GRAXUS_EMBED_API_KEY".into();
        }
        if std::env::var("GRAXUS_LLM_API_KEY").is_ok() {
            self.llm.api_key_env = "GRAXUS_LLM_API_KEY".into();
        }

        // Bool fields
        if let Ok(v) = std::env::var("GRAXUS_SCAN_RESPECT_GITIGNORE") {
            self.scan.respect_gitignore = parse_env_bool(&v);
        }
        if let Ok(v) = std::env::var("GRAXUS_DOCS_ENABLED") {
            self.docs.enabled = parse_env_bool(&v);
        }
        if let Ok(v) = std::env::var("GRAXUS_CODE_ENABLED") {
            self.code.enabled = parse_env_bool(&v);
        }
        if let Ok(v) = std::env::var("GRAXUS_CODEMAP_PARSER_BACKEND") {
            match v.parse::<ParserBackend>() {
                Ok(backend) => {
                    self.codemap.parser_backend = Some(backend.as_str().to_string());
                }
                Err(_) => {
                    tracing::warn!(
                        "GRAXUS_CODEMAP_PARSER_BACKEND={v:?} is not a valid backend; ignoring"
                    );
                }
            }
        }

        // Numeric fields
        if let Ok(v) = std::env::var("GRAXUS_INDEX_MAX_FILES") {
            if let Ok(n) = v.parse::<usize>() {
                self.defaults.max_files = n;
            }
        }
        if let Ok(v) = std::env::var("GRAXUS_EDIT_MAX_FILES") {
            if let Ok(n) = v.parse::<usize>() {
                self.edit.max_files_per_operation = n;
            }
        }
        if let Ok(v) = std::env::var("GRAXUS_CONTEXT_MAX_TOKENS") {
            if let Ok(n) = v.parse::<usize>() {
                self.defaults.context_budget = n;
            }
        }
        if let Ok(v) = std::env::var("GRAXUS_CONTEXT_BUDGET") {
            if let Ok(n) = v.parse::<usize>() {
                self.defaults.context_budget = n;
            }
        }
        if let Ok(v) = std::env::var("GRAXUS_SEARCH_K") {
            if let Ok(n) = v.parse::<usize>() {
                self.search.k = Some(n);
            }
        }
    }

    /// Resolve the effective codemap parser backend.
    ///
    /// Precedence: explicit `override_backend` (CLI) > configured string >
    /// `GRAXUS_CODEMAP_PARSER_BACKEND` (already folded into `codemap.parser_backend`
    /// by `apply_env_overrides`) > built-in default (`ParserBackend::Ripex`).
    pub fn effective_codemap_backend(
        &self,
        override_backend: Option<ParserBackend>,
    ) -> ParserBackend {
        if let Some(b) = override_backend {
            return b;
        }
        if let Some(s) = &self.codemap.parser_backend {
            if let Ok(b) = s.parse::<ParserBackend>() {
                return b;
            }
        }
        ParserBackend::default()
    }
}

/// Parse a boolean string from an environment variable.
///
/// Accepts `true`, `1`, `yes` (case-insensitive) as truthy; everything else is falsy.
fn parse_env_bool(s: &str) -> bool {
    matches!(s.to_ascii_lowercase().as_str(), "true" | "1" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    /// Serialize tests that mutate GRAXUS_* env vars (process-global, not thread-safe).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_config() {
        let config = GraxusConfig::default();
        assert!(!config.project.name.is_empty());
        assert_eq!(config.index.storage, "json");
        assert!(!config.embeddings.enabled);
        assert!(!config.llm.enabled);
    }

    #[test]
    fn test_embeddings_config_default() {
        let config = EmbeddingsConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.provider, "openai");
        assert_eq!(config.model, "text-embedding-3-small");
        assert_eq!(config.dimensions, 1536);
        assert_eq!(config.batch_size, 64);
    }

    #[test]
    fn test_llm_config_default() {
        let config = LlmConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.provider, "openai");
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.max_tokens, 4096);
    }

    #[test]
    fn test_api_key_from_env() {
        let config = EmbeddingsConfig {
            api_key_env: "GRAXUS_TEST_KEY_123".into(),
            ..Default::default()
        };
        assert!(config.api_key().is_none());
        std::env::set_var("GRAXUS_TEST_KEY_123", "sk-test");
        assert_eq!(config.api_key(), Some("sk-test".to_string()));
        std::env::remove_var("GRAXUS_TEST_KEY_123");
    }

    #[test]
    fn test_config_save_and_load() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();

        let dir = tempdir().unwrap();
        let config = GraxusConfig::default();
        config.save(dir.path()).unwrap();
        let loaded = GraxusConfig::load(dir.path()).unwrap();
        assert_eq!(loaded.project.name, config.project.name);
        assert_eq!(loaded.index.storage, config.index.storage);
    }

    // ── Env override tests ────────────────────────────────────────────

    /// Remove all GRAXUS_* env vars to prevent parallel test interference.
    fn clear_graxus_env_vars() {
        let vars = [
            "GRAXUS_PROJECT_NAME",
            "GRAXUS_PROJECT_ROOT",
            "GRAXUS_SCAN_RESPECT_GITIGNORE",
            "GRAXUS_DOCS_ENABLED",
            "GRAXUS_CODE_ENABLED",
            "GRAXUS_INDEX_MAX_FILES",
            "GRAXUS_EDIT_MAX_FILES",
            "GRAXUS_CONTEXT_MAX_TOKENS",
            "GRAXUS_CONTEXT_BUDGET",
            "GRAXUS_SEARCH_K",
            "GRAXUS_EMBED_PROVIDER",
            "GRAXUS_EMBED_MODEL",
            "GRAXUS_EMBED_API_KEY",
            "GRAXUS_LLM_PROVIDER",
            "GRAXUS_LLM_MODEL",
            "GRAXUS_LLM_API_KEY",
        ];
        for var in &vars {
            std::env::remove_var(var);
        }
    }

    #[test]
    fn test_env_override_string_field() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        std::env::set_var("GRAXUS_PROJECT_NAME", "env-project");
        let mut config = GraxusConfig::default();
        config.apply_env_overrides();
        assert_eq!(config.project.name, "env-project");
        std::env::remove_var("GRAXUS_PROJECT_NAME");
    }

    #[test]
    fn test_env_override_bool_field() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        assert!(config.scan.respect_gitignore); // default is true
        std::env::set_var("GRAXUS_SCAN_RESPECT_GITIGNORE", "false");
        config.apply_env_overrides();
        assert!(!config.scan.respect_gitignore);
        std::env::remove_var("GRAXUS_SCAN_RESPECT_GITIGNORE");
    }

    #[test]
    fn test_env_override_bool_truthy_values() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        config.docs.enabled = false;

        std::env::set_var("GRAXUS_DOCS_ENABLED", "true");
        config.apply_env_overrides();
        assert!(config.docs.enabled);
        std::env::remove_var("GRAXUS_DOCS_ENABLED");

        config.docs.enabled = false;
        std::env::set_var("GRAXUS_DOCS_ENABLED", "1");
        config.apply_env_overrides();
        assert!(config.docs.enabled);
        std::env::remove_var("GRAXUS_DOCS_ENABLED");

        config.docs.enabled = false;
        std::env::set_var("GRAXUS_DOCS_ENABLED", "yes");
        config.apply_env_overrides();
        assert!(config.docs.enabled);
        std::env::remove_var("GRAXUS_DOCS_ENABLED");
    }

    #[test]
    fn test_env_override_numeric_field() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        assert_eq!(config.defaults.max_files, 100);
        std::env::set_var("GRAXUS_INDEX_MAX_FILES", "250");
        config.apply_env_overrides();
        assert_eq!(config.defaults.max_files, 250);
        std::env::remove_var("GRAXUS_INDEX_MAX_FILES");
    }

    #[test]
    fn test_env_override_edit_max_files() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        assert_eq!(config.edit.max_files_per_operation, 100);
        std::env::set_var("GRAXUS_EDIT_MAX_FILES", "50");
        config.apply_env_overrides();
        assert_eq!(config.edit.max_files_per_operation, 50);
        std::env::remove_var("GRAXUS_EDIT_MAX_FILES");
    }

    #[test]
    fn test_env_override_search_k() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        assert!(config.search.k.is_none());
        std::env::set_var("GRAXUS_SEARCH_K", "10");
        config.apply_env_overrides();
        assert_eq!(config.search.k, Some(10));
        std::env::remove_var("GRAXUS_SEARCH_K");
    }

    #[test]
    fn test_env_override_context_budget() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        assert_eq!(config.defaults.context_budget, 12000);
        std::env::set_var("GRAXUS_CONTEXT_BUDGET", "8000");
        config.apply_env_overrides();
        assert_eq!(config.defaults.context_budget, 8000);
        std::env::remove_var("GRAXUS_CONTEXT_BUDGET");
    }

    #[test]
    fn test_env_override_context_max_tokens_alias() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        std::env::set_var("GRAXUS_CONTEXT_MAX_TOKENS", "6000");
        config.apply_env_overrides();
        assert_eq!(config.defaults.context_budget, 6000);
        std::env::remove_var("GRAXUS_CONTEXT_MAX_TOKENS");
    }

    #[test]
    fn test_env_override_embed_api_key() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        assert!(config.embeddings.api_key_env.is_empty());
        std::env::set_var("GRAXUS_EMBED_API_KEY", "sk-test-override");
        config.apply_env_overrides();
        assert_eq!(config.embeddings.api_key_env, "GRAXUS_EMBED_API_KEY");
        assert_eq!(
            config.embeddings.api_key(),
            Some("sk-test-override".to_string())
        );
        std::env::remove_var("GRAXUS_EMBED_API_KEY");
    }

    #[test]
    fn test_env_override_llm_api_key() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        assert!(config.llm.api_key_env.is_empty());
        std::env::set_var("GRAXUS_LLM_API_KEY", "llm-key-test");
        config.apply_env_overrides();
        assert_eq!(config.llm.api_key_env, "GRAXUS_LLM_API_KEY");
        assert_eq!(config.llm.api_key(), Some("llm-key-test".to_string()));
        std::env::remove_var("GRAXUS_LLM_API_KEY");
    }

    #[test]
    fn test_env_override_embed_provider() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        assert_eq!(config.embeddings.provider, "openai");
        std::env::set_var("GRAXUS_EMBED_PROVIDER", "voyage");
        config.apply_env_overrides();
        assert_eq!(config.embeddings.provider, "voyage");
        std::env::remove_var("GRAXUS_EMBED_PROVIDER");
    }

    #[test]
    fn test_env_override_llm_provider_and_model() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        std::env::set_var("GRAXUS_LLM_PROVIDER", "anthropic");
        std::env::set_var("GRAXUS_LLM_MODEL", "claude-sonnet-4-20250514");
        config.apply_env_overrides();
        assert_eq!(config.llm.provider, "anthropic");
        assert_eq!(config.llm.model, "claude-sonnet-4-20250514");
        std::env::remove_var("GRAXUS_LLM_PROVIDER");
        std::env::remove_var("GRAXUS_LLM_MODEL");
    }

    #[test]
    fn test_missing_env_vars_dont_change_defaults() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();

        let before = GraxusConfig::default();
        let mut after = GraxusConfig::default();
        after.apply_env_overrides();

        assert_eq!(before.project.name, after.project.name);
        assert_eq!(before.project.root, after.project.root);
        assert_eq!(before.scan.respect_gitignore, after.scan.respect_gitignore);
        assert_eq!(before.docs.enabled, after.docs.enabled);
        assert_eq!(before.code.enabled, after.code.enabled);
        assert_eq!(before.defaults.max_files, after.defaults.max_files);
        assert_eq!(
            before.edit.max_files_per_operation,
            after.edit.max_files_per_operation
        );
        assert_eq!(
            before.defaults.context_budget,
            after.defaults.context_budget
        );
        assert_eq!(before.search.k, after.search.k);
        assert_eq!(before.embeddings.provider, after.embeddings.provider);
        assert_eq!(before.embeddings.model, after.embeddings.model);
        assert_eq!(before.embeddings.api_key_env, after.embeddings.api_key_env);
        assert_eq!(before.llm.provider, after.llm.provider);
        assert_eq!(before.llm.model, after.llm.model);
        assert_eq!(before.llm.api_key_env, after.llm.api_key_env);
    }

    #[test]
    fn test_precedence_env_over_config_file() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let dir = tempdir().unwrap();
        // Save a complete config with project.name = "from-file"
        let mut file_config = GraxusConfig::default();
        file_config.project.name = "from-file".into();
        file_config.save(dir.path()).unwrap();

        // Set env override
        std::env::set_var("GRAXUS_PROJECT_NAME", "from-env");

        let loaded = GraxusConfig::load(dir.path()).unwrap();
        // Env should win over file
        assert_eq!(loaded.project.name, "from-env");

        std::env::remove_var("GRAXUS_PROJECT_NAME");
    }

    #[test]
    fn test_env_override_code_enabled() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        assert!(config.code.enabled);
        std::env::set_var("GRAXUS_CODE_ENABLED", "false");
        config.apply_env_overrides();
        assert!(!config.code.enabled);
        std::env::remove_var("GRAXUS_CODE_ENABLED");
    }

    #[test]
    fn test_env_override_project_root() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        std::env::set_var("GRAXUS_PROJECT_ROOT", "/custom/path");
        config.apply_env_overrides();
        assert_eq!(config.project.root, "/custom/path");
        std::env::remove_var("GRAXUS_PROJECT_ROOT");
    }

    #[test]
    fn test_env_override_numeric_ignores_invalid() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_graxus_env_vars();
        let mut config = GraxusConfig::default();
        let original = config.defaults.max_files;
        std::env::set_var("GRAXUS_INDEX_MAX_FILES", "not-a-number");
        config.apply_env_overrides();
        // Invalid values should be silently ignored
        assert_eq!(config.defaults.max_files, original);
        std::env::remove_var("GRAXUS_INDEX_MAX_FILES");
    }
}
