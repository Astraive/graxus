//! OpenAI Chat Completions API provider.
//!
//! Supports GPT-4o, GPT-4o-mini, GPT-4-turbo, and GPT-3.5-turbo models.

use anyhow::Context;
use async_trait::async_trait;

use crate::provider::{LlmProvider, LlmRequest, LlmResponse};

/// LLM provider backed by the OpenAI Chat Completions API.
///
/// Uses `https://api.openai.com/v1/chat/completions` as the endpoint.
/// Supports all standard OpenAI chat models.
pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider with the given API key and model.
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key (should be loaded from environment, not hardcoded)
    /// * `model` - Model identifier (e.g. "gpt-4o", "gpt-4o-mini")
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn max_context_tokens(&self) -> usize {
        match self.model.as_str() {
            "gpt-4o" | "gpt-4o-mini" | "gpt-4-turbo" => 128_000,
            "gpt-3.5-turbo" => 16_385,
            _ => 128_000,
        }
    }

    async fn complete(&self, request: LlmRequest) -> anyhow::Result<LlmResponse> {
        let mut messages = Vec::new();
        if !request.system.is_empty() {
            messages.push(serde_json::json!({"role": "system", "content": request.system}));
        }
        messages.push(serde_json::json!({"role": "user", "content": request.user}));

        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": request.max_tokens,
            "temperature": request.temperature,
        });

        let resp = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("OpenAI request failed")?;

        // Handle rate limit (429) specifically
        if resp.status().as_u16() == 429 {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);
            anyhow::bail!(
                "OpenAI rate limit exceeded. Retry after {} seconds.",
                retry_after
            );
        }

        let resp = resp
            .error_for_status()
            .context("OpenAI returned error")?;

        let v: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        Ok(LlmResponse {
            content: v["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            input_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as usize,
            output_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0) as usize,
            model: self.model.clone(),
        })
    }
}
