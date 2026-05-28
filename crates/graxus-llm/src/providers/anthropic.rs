use anyhow::Context;
use async_trait::async_trait;

use crate::provider::{LlmProvider, LlmRequest, LlmResponse};

pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str { "anthropic" }
    fn model(&self) -> &str { &self.model }
    fn max_context_tokens(&self) -> usize {
        match self.model.as_str() {
            "claude-3-5-sonnet-20241022" | "claude-3-5-sonnet-latest" => 200_000,
            "claude-3-opus-20240229" | "claude-3-opus-latest" => 200_000,
            "claude-3-haiku-20240307" | "claude-3-haiku-latest" => 200_000,
            _ => 200_000,
        }
    }

    async fn complete(&self, request: LlmRequest) -> anyhow::Result<LlmResponse> {
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": request.max_tokens,
            "messages": [{"role": "user", "content": request.user}],
        });
        if !request.system.is_empty() {
            body["system"] = serde_json::json!(request.system);
        }

        let resp = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send().await
            .context("Anthropic request failed")?
            .error_for_status()
            .context("Anthropic returned error")?;

        let v: serde_json::Value = resp.json().await
            .context("Failed to parse Anthropic response")?;

        Ok(LlmResponse {
            content: v["content"][0]["text"].as_str().unwrap_or("").to_string(),
            input_tokens: v["usage"]["input_tokens"].as_u64().unwrap_or(0) as usize,
            output_tokens: v["usage"]["output_tokens"].as_u64().unwrap_or(0) as usize,
            model: self.model.clone(),
        })
    }
}
