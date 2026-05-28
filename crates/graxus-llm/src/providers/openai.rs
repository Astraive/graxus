use anyhow::Context;
use async_trait::async_trait;

use crate::provider::{LlmProvider, LlmRequest, LlmResponse};

pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenAiProvider {
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
    fn name(&self) -> &str { "openai" }
    fn model(&self) -> &str { &self.model }
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

        let resp = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send().await
            .context("OpenAI request failed")?
            .error_for_status()
            .context("OpenAI returned error")?;

        let v: serde_json::Value = resp.json().await
            .context("Failed to parse OpenAI response")?;

        Ok(LlmResponse {
            content: v["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string(),
            input_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as usize,
            output_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0) as usize,
            model: self.model.clone(),
        })
    }
}
