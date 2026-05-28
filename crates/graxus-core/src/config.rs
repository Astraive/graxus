use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraxusConfig {
    pub project: ProjectConfig,
    pub scan: ScanConfig,
    pub docs: DocsConfig,
    pub code: CodeConfig,
    pub index: IndexConfig,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    #[serde(default = "default_true")]
    pub respect_gitignore: bool,
    #[serde(default = "default_include")]
    pub include: Vec<String>,
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub obsidian_compatible: bool,
    #[serde(default = "default_doc_extensions")]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub parse: DocParseConfig,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_parser")]
    pub parser: String,
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    #[serde(default = "default_storage")]
    pub storage: String,
    #[serde(default = "default_index_path")]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditConfig {
    #[serde(default = "default_true")]
    pub create_snapshots: bool,
    #[serde(default = "default_true")]
    pub require_preview_for_replace: bool,
    #[serde(default = "default_max_files")]
    pub max_files_per_operation: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_embed_provider")]
    pub provider: String,
    #[serde(default = "default_embed_model")]
    pub model: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_llm_provider")]
    pub provider: String,
    #[serde(default = "default_llm_model")]
    pub model: String,
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

impl Default for GraxusConfig {
    fn default() -> Self {
        let dir_name = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "project".into());

        Self {
            project: ProjectConfig {
                name: dir_name,
                root: ".".into(),
            },
            scan: ScanConfig {
                respect_gitignore: true,
                include: default_include(),
                exclude: default_exclude(),
            },
            docs: DocsConfig {
                enabled: true,
                obsidian_compatible: true,
                extensions: default_doc_extensions(),
                parse: DocParseConfig::default(),
            },
            code: CodeConfig {
                enabled: true,
                parser: default_parser(),
                languages: default_languages(),
            },
            index: IndexConfig {
                storage: default_storage(),
                path: default_index_path(),
            },
            edit: EditConfig {
                create_snapshots: true,
                require_preview_for_replace: true,
                max_files_per_operation: default_max_files(),
            },
            embeddings: EmbeddingsConfig::default(),
            llm: LlmConfig::default(),
            defaults: DefaultsConfig::default(),
            context: ContextConfig::default(),
            impact: ImpactConfig::default(),
            search: SearchConfig::default(),
            graph: GraphConfig::default(),
            codemap: CodemapConfig::default(),
            doctor: DoctorConfig::default(),
        }
    }
}

// ── v0.3 Config Sections ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsConfig {
    #[serde(default = "default_d_depth")]
    pub depth: usize,
    #[serde(default = "default_d_max_depth")]
    pub max_depth: usize,
    #[serde(default = "default_d_k")]
    pub k: usize,
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
    pub fn resolve_depth(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.depth)
    }
    pub fn resolve_k(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.k)
    }
    pub fn resolve_max_files(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_files)
    }
    pub fn resolve_max_symbols(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_symbols)
    }
    pub fn resolve_max_notes(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_notes)
    }
    pub fn resolve_max_nodes(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_nodes)
    }
    pub fn resolve_max_edges(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_edges)
    }
    pub fn resolve_min_confidence(&self, cli: Option<f64>, section: Option<f64>) -> f64 {
        cli.or(section).unwrap_or(self.min_confidence)
    }
    pub fn resolve_context_budget(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.context_budget)
    }
    pub fn resolve_max_chars_per_file(&self, cli: Option<usize>, section: Option<usize>) -> usize {
        cli.or(section).unwrap_or(self.max_chars_per_file)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextConfig {
    pub depth: Option<usize>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchConfig {
    pub k: Option<usize>,
    pub max_results: Option<usize>,
    pub min_score: Option<f64>,
    pub hybrid: Option<bool>,
    pub include_snippets: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphConfig {
    pub depth: Option<usize>,
    pub max_notes: Option<usize>,
    pub max_nodes: Option<usize>,
    pub max_edges: Option<usize>,
    pub include_backlinks: Option<bool>,
    pub include_orphans: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodemapConfig {
    pub depth: Option<usize>,
    pub max_files: Option<usize>,
    pub max_symbols: Option<usize>,
    pub max_imports: Option<usize>,
    pub max_calls: Option<usize>,
    pub include_tests: Option<bool>,
    pub min_confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DoctorConfig {
    pub min_import_resolution: Option<f64>,
    pub min_call_resolution: Option<f64>,
    pub max_parse_errors: Option<usize>,
    pub max_broken_links: Option<usize>,
}

// ── Tests for v0.3 config ─────────────────────────────────────────────

impl GraxusConfig {
    /// Load config from graxus.yaml in the given directory.
    pub fn load(root: &Path) -> Result<Self> {
        let config_path = root.join("graxus.yaml");
        if !config_path.exists() {
            tracing::debug!("No graxus.yaml found, using defaults");
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;
        let config: GraxusConfig = serde_yaml::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", config_path.display()))?;
        Ok(config)
    }

    /// Save config to graxus.yaml in the given directory.
    pub fn save(&self, root: &Path) -> Result<()> {
        let config_path = root.join("graxus.yaml");
        let contents = serde_yaml::to_string(self).context("Failed to serialize config")?;
        std::fs::write(&config_path, contents)
            .with_context(|| format!("Failed to write {}", config_path.display()))?;
        Ok(())
    }

    /// Get the .graxus directory path relative to root.
    pub fn graxus_dir(&self, root: &Path) -> PathBuf {
        root.join(".graxus")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
        let dir = tempdir().unwrap();
        let config = GraxusConfig::default();
        config.save(dir.path()).unwrap();
        let loaded = GraxusConfig::load(dir.path()).unwrap();
        assert_eq!(loaded.project.name, config.project.name);
        assert_eq!(loaded.index.storage, config.index.storage);
    }
}
