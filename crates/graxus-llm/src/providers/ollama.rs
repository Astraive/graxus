//! Ollama local inference API provider.
//!
//! Connects to a local or remote Ollama instance for self-hosted model inference.
//! Supports any model available in the Ollama model library.

use anyhow::Context;
use async_trait::async_trait;

use crate::provider::{LlmProvider, LlmRequest, LlmResponse};

/// LLM provider backed by the Ollama local inference API.
///
/// Connects to a configurable endpoint (default `http://localhost:11434`)
/// and supports any model loaded in the Ollama instance.
pub struct OllamaProvider {
    client: reqwest::Client,
    endpoint: String,
    model: String,
}

impl OllamaProvider {
    /// Create a new Ollama provider with the given endpoint and model.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Base URL of the Ollama instance (e.g. "http://localhost:11434")
    /// * `model` - Model name (e.g. "llama3", "codellama")
    pub fn new(endpoint: &str, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: endpoint.to_string(),
            model,
        }
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn max_context_tokens(&self) -> usize {
        32_768
    }

    async fn complete(&self, request: LlmRequest) -> anyhow::Result<LlmResponse> {
        let body = serde_json::json!({
            "model": self.model,
            "prompt": request.user,
            "system": request.system,
            "stream": false,
            "options": {
                "num_predict": request.max_tokens,
                "temperature": request.temperature,
            },
        });

        let resp = self
            .client
            .post(format!("{}/api/generate", self.endpoint))
            .json(&body)
            .send()
            .await
            .context("Ollama request failed")?
            .error_for_status()
            .context("Ollama returned error")?;

        let v: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        Ok(LlmResponse {
            content: v["response"].as_str().unwrap_or("").to_string(),
            input_tokens: v["prompt_eval_count"].as_u64().unwrap_or(0) as usize,
            output_tokens: v["eval_count"].as_u64().unwrap_or(0) as usize,
            model: self.model.clone(),
        })
    }
}
