//! Documentation generation orchestration.
//!
//! The [`DocGenerator`] ties together an LLM provider, cost tracking, rate limiting,
//! and retry logic to produce documentation from code analysis data.

use anyhow::Result;
use std::cmp::min;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cost::CostTracker;
use crate::prompts;
use crate::provider::{LlmProvider, LlmRequest};
use crate::rate_limit::RateLimiter;

/// Configuration for retry behavior with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts after the initial request.
    pub max_retries: u32,
    /// Base delay between retries in milliseconds.
    pub base_delay_ms: u64,
    /// Maximum delay between retries in milliseconds.
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30_000,
        }
    }
}

/// Report from a generation run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenerationReport {
    /// Number of documentation files generated.
    pub files_generated: usize,
    /// Total input tokens consumed across all requests.
    pub total_input_tokens: usize,
    /// Total output tokens generated across all requests.
    pub total_output_tokens: usize,
    /// Error messages encountered during generation.
    pub errors: Vec<String>,
}

/// Orchestrates LLM-powered documentation generation.
///
/// Combines an [`LlmProvider`] with cost tracking, rate limiting, and retry logic
/// to produce documentation for code modules, functions, and architectures.
pub struct DocGenerator {
    provider: Box<dyn LlmProvider>,
    cost_tracker: CostTracker,
    rate_limiter: RateLimiter,
    output_dir: PathBuf,
    retry_config: RetryConfig,
}

impl DocGenerator {
    /// Create a new generator with default retry configuration (3 retries, 1s base delay).
    pub fn new(
        provider: Box<dyn LlmProvider>,
        max_cost: f64,
        rpm: u32,
        output_dir: PathBuf,
    ) -> Self {
        Self {
            provider,
            cost_tracker: CostTracker::new(max_cost),
            rate_limiter: RateLimiter::new(rpm),
            output_dir,
            retry_config: RetryConfig::default(),
        }
    }

    /// Set a custom retry configuration.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Generate documentation for a single module file.
    ///
    /// Uses the module summary prompt template with the file's language, symbols,
    /// and imports to produce markdown documentation.
    pub async fn generate_module_doc(
        &mut self,
        file_path: &str,
        symbols: &str,
        imports: &str,
    ) -> Result<String> {
        let language = detect_language(file_path);
        let (system, user) = prompts::module_summary_prompt(file_path, &language, symbols, imports);

        self.rate_limiter.wait().await;
        let request = LlmRequest {
            system,
            user,
            max_tokens: self.provider.max_context_tokens().min(4096),
            temperature: 0.3,
        };
        let response = retry_request(&*self.provider, request, &self.retry_config).await?;
        self.cost_tracker.record(
            response.input_tokens,
            response.output_tokens,
            self.provider.model(),
        )?;

        Ok(response.content)
    }

    /// Generate documentation for a single function.
    ///
    /// Produces a docstring-style description including parameters, return value,
    /// and side effects based on the function's source code and callers.
    pub async fn generate_function_doc(
        &mut self,
        name: &str,
        source: &str,
        callers: &str,
    ) -> Result<String> {
        let (system, user) = prompts::function_doc_prompt(name, source, callers);

        self.rate_limiter.wait().await;
        let request = LlmRequest {
            system,
            user,
            max_tokens: 2048,
            temperature: 0.2,
        };
        let response = retry_request(&*self.provider, request, &self.retry_config).await?;
        self.cost_tracker.record(
            response.input_tokens,
            response.output_tokens,
            self.provider.model(),
        )?;

        Ok(response.content)
    }

    /// Generate architecture overview documentation.
    ///
    /// Produces an ARCHITECTURE.md-style document describing high-level structure,
    /// module responsibilities, data flow, and design decisions.
    pub async fn generate_architecture(
        &mut self,
        project_name: &str,
        file_count: usize,
        symbol_count: usize,
        languages: &str,
    ) -> Result<String> {
        let (system, user) =
            prompts::architecture_prompt(project_name, file_count, symbol_count, languages);

        self.rate_limiter.wait().await;
        let request = LlmRequest {
            system,
            user,
            max_tokens: 8192,
            temperature: 0.3,
        };
        let response = retry_request(&*self.provider, request, &self.retry_config).await?;
        self.cost_tracker.record(
            response.input_tokens,
            response.output_tokens,
            self.provider.model(),
        )?;

        Ok(response.content)
    }

    /// Check whether existing documentation is stale relative to the current code.
    ///
    /// Compares the documentation content against the current code state and returns
    /// suggested minimal updates.
    pub async fn check_stale_doc(&mut self, doc_content: &str, code_state: &str) -> Result<String> {
        let (system, user) = prompts::stale_check_prompt(doc_content, code_state);

        self.rate_limiter.wait().await;
        let request = LlmRequest {
            system,
            user,
            max_tokens: 4096,
            temperature: 0.2,
        };
        let response = retry_request(&*self.provider, request, &self.retry_config).await?;
        self.cost_tracker.record(
            response.input_tokens,
            response.output_tokens,
            self.provider.model(),
        )?;

        Ok(response.content)
    }

    /// Save generated content to a file under the output directory.
    ///
    /// Creates parent directories as needed. The `relative_path` is joined
    /// with the configured output directory.
    pub fn save_output(&self, relative_path: &str, content: &str) -> Result<()> {
        let path = self.output_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Get a snapshot of the current cost and usage summary.
    pub fn cost_summary(&self) -> crate::cost::CostSummary {
        self.cost_tracker.summary()
    }
}

/// Execute an LLM request with exponential backoff retry.
///
/// Retries transient failures up to `config.max_retries` times with
/// exponential backoff capped at `config.max_delay_ms`.
async fn retry_request(
    provider: &dyn LlmProvider,
    request: LlmRequest,
    config: &RetryConfig,
) -> Result<crate::provider::LlmResponse> {
    let mut last_err = None;
    for attempt in 0..=config.max_retries {
        match provider.complete(request.clone()).await {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                if attempt < config.max_retries {
                    let delay = min(
                        config
                            .base_delay_ms
                            .saturating_mul(2u64.saturating_pow(attempt)),
                        config.max_delay_ms,
                    );
                    tracing::warn!(
                        "LLM request failed (attempt {}/{}), retrying in {}ms: {}",
                        attempt + 1,
                        config.max_retries + 1,
                        delay,
                        e
                    );
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    last_err = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }
    // Unreachable in practice, but satisfies the compiler.
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("retry exhausted with no error captured")))
}

/// Detect the programming language from a file path's extension.
fn detect_language(path: &str) -> String {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("rs") => "Rust".into(),
        Some("ts") | Some("tsx") | Some("mts") | Some("cts") => "TypeScript".into(),
        Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => "JavaScript".into(),
        Some("go") => "Go".into(),
        Some("py") | Some("pyi") => "Python".into(),
        Some("md") | Some("mdx") => "Markdown".into(),
        _ => "Unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{LlmProvider, LlmRequest, LlmResponse};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A mock LLM provider for testing.
    struct MockProvider {
        response: LlmResponse,
        fail_count: Arc<AtomicUsize>,
        call_count: Arc<AtomicUsize>,
    }

    impl MockProvider {
        fn new(content: &str) -> Self {
            Self {
                response: LlmResponse {
                    content: content.into(),
                    input_tokens: 100,
                    output_tokens: 50,
                    model: "gpt-4o-mini".into(),
                },
                fail_count: Arc::new(AtomicUsize::new(0)),
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn with_failures(content: &str, fail_count: usize) -> Self {
            Self {
                response: LlmResponse {
                    content: content.into(),
                    input_tokens: 100,
                    output_tokens: 50,
                    model: "gpt-4o-mini".into(),
                },
                fail_count: Arc::new(AtomicUsize::new(fail_count)),
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn call_count(&self) -> Arc<AtomicUsize> {
            self.call_count.clone()
        }
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }
        fn model(&self) -> &str {
            &self.response.model
        }
        fn max_context_tokens(&self) -> usize {
            4096
        }

        async fn complete(&self, _request: LlmRequest) -> anyhow::Result<LlmResponse> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            if count < self.fail_count.load(Ordering::SeqCst) {
                anyhow::bail!("simulated failure");
            }
            Ok(self.response.clone())
        }
    }

    #[test]
    fn detect_language_known_extensions() {
        assert_eq!(detect_language("main.rs"), "Rust");
        assert_eq!(detect_language("app.ts"), "TypeScript");
        assert_eq!(detect_language("component.tsx"), "TypeScript");
        assert_eq!(detect_language("index.js"), "JavaScript");
        assert_eq!(detect_language("server.mjs"), "JavaScript");
        assert_eq!(detect_language("main.go"), "Go");
        assert_eq!(detect_language("script.py"), "Python");
        assert_eq!(detect_language("types.pyi"), "Python");
        assert_eq!(detect_language("README.md"), "Markdown");
        assert_eq!(detect_language("page.mdx"), "Markdown");
    }

    #[test]
    fn detect_language_unknown_extension() {
        assert_eq!(detect_language("Makefile"), "Unknown");
        assert_eq!(detect_language("data.json"), "Unknown");
        assert_eq!(detect_language("style.css"), "Unknown");
    }

    #[test]
    fn retry_config_default_values() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30_000);
    }

    #[tokio::test]
    async fn doc_generator_new_sets_defaults() {
        let provider = MockProvider::new("test");
        let gen = DocGenerator::new(Box::new(provider), 10.0, 60, PathBuf::from("/tmp"));
        assert_eq!(gen.cost_tracker.max_cost_usd, 10.0);
        assert_eq!(gen.retry_config.max_retries, 3);
    }

    #[tokio::test]
    async fn generate_module_doc_success() {
        let provider = MockProvider::new("# Module docs");
        let mut gen = DocGenerator::new(Box::new(provider), 100.0, 6000, PathBuf::from("/tmp"));

        let result = gen
            .generate_module_doc("main.rs", "fn main", "use std")
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "# Module docs");
        assert_eq!(gen.cost_tracker.total_requests, 1);
    }

    #[tokio::test]
    async fn generate_function_doc_success() {
        let provider = MockProvider::new("/// A function");
        let mut gen = DocGenerator::new(Box::new(provider), 100.0, 6000, PathBuf::from("/tmp"));

        let result = gen.generate_function_doc("foo", "fn foo() {}", "bar").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "/// A function");
    }

    #[tokio::test]
    async fn generate_architecture_success() {
        let provider = MockProvider::new("# Architecture");
        let mut gen = DocGenerator::new(Box::new(provider), 100.0, 6000, PathBuf::from("/tmp"));

        let result = gen.generate_architecture("proj", 10, 50, "Rust").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "# Architecture");
    }

    #[tokio::test]
    async fn check_stale_doc_success() {
        let provider = MockProvider::new("Updated docs");
        let mut gen = DocGenerator::new(Box::new(provider), 100.0, 6000, PathBuf::from("/tmp"));

        let result = gen.check_stale_doc("old doc", "new code").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Updated docs");
    }

    #[tokio::test]
    async fn retry_succeeds_after_transient_failures() {
        let provider = MockProvider::with_failures("ok after retry", 2);
        let call_count = provider.call_count();
        let config = RetryConfig {
            max_retries: 3,
            base_delay_ms: 10, // Fast for tests
            max_delay_ms: 50,
        };

        let result = retry_request(
            &provider,
            LlmRequest {
                system: String::new(),
                user: "test".into(),
                max_tokens: 100,
                temperature: 0.5,
            },
            &config,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "ok after retry");
        // 2 failures + 1 success = 3 calls
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_exhausts_and_returns_error() {
        let provider = MockProvider::with_failures("never reached", 10);
        let config = RetryConfig {
            max_retries: 2,
            base_delay_ms: 10,
            max_delay_ms: 50,
        };

        let result = retry_request(
            &provider,
            LlmRequest {
                system: String::new(),
                user: "test".into(),
                max_tokens: 100,
                temperature: 0.5,
            },
            &config,
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("simulated failure"));
    }

    #[test]
    fn save_output_creates_file() {
        let tmp = std::env::temp_dir().join("graxus_llm_test_save");
        let provider = MockProvider::new("test");
        let gen = DocGenerator::new(Box::new(provider), 100.0, 60, tmp.clone());

        let result = gen.save_output("sub/test.md", "# Hello");
        assert!(result.is_ok());
        let written = std::fs::read_to_string(tmp.join("sub/test.md")).unwrap();
        assert_eq!(written, "# Hello");

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn cost_summary_after_operations() {
        let provider = MockProvider::new("test");
        let gen = DocGenerator::new(Box::new(provider), 100.0, 60, PathBuf::from("/tmp"));
        let summary = gen.cost_summary();
        assert_eq!(summary.total_requests, 0);
        assert_eq!(summary.total_input_tokens, 0);
    }
}
