use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::cost::CostTracker;
use crate::provider::{LlmProvider, LlmRequest};
use crate::prompts;
use crate::rate_limit::RateLimiter;

/// Report from a generation run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenerationReport {
    pub files_generated: usize,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub errors: Vec<String>,
}

/// Orchestrates LLM-powered documentation generation.
pub struct DocGenerator {
    provider: Box<dyn LlmProvider>,
    cost_tracker: CostTracker,
    rate_limiter: RateLimiter,
    output_dir: PathBuf,
}

impl DocGenerator {
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
        }
    }

    /// Generate documentation for a single module file.
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
        let response = self.provider.complete(request).await?;
        self.cost_tracker
            .record(response.input_tokens, response.output_tokens, self.provider.model())?;

        Ok(response.content)
    }

    /// Generate documentation for a single function.
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
        let response = self.provider.complete(request).await?;
        self.cost_tracker
            .record(response.input_tokens, response.output_tokens, self.provider.model())?;

        Ok(response.content)
    }

    /// Generate architecture overview.
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
        let response = self.provider.complete(request).await?;
        self.cost_tracker
            .record(response.input_tokens, response.output_tokens, self.provider.model())?;

        Ok(response.content)
    }

    /// Generate a stale doc check.
    pub async fn check_stale_doc(
        &mut self,
        doc_content: &str,
        code_state: &str,
    ) -> Result<String> {
        let (system, user) = prompts::stale_check_prompt(doc_content, code_state);

        self.rate_limiter.wait().await;
        let request = LlmRequest {
            system,
            user,
            max_tokens: 4096,
            temperature: 0.2,
        };
        let response = self.provider.complete(request).await?;
        self.cost_tracker
            .record(response.input_tokens, response.output_tokens, self.provider.model())?;

        Ok(response.content)
    }

    /// Save generated content to the output directory.
    pub fn save_output(&self, relative_path: &str, content: &str) -> Result<()> {
        let path = self.output_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Get cost summary.
    pub fn cost_summary(&self) -> crate::cost::CostSummary {
        self.cost_tracker.summary()
    }
}

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
