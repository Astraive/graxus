use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;

use crate::provider::EmbeddingProvider;

pub struct OpenAIProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dimensions: usize,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: Option<String>) -> Self {
        let model = model.unwrap_or_else(|| "text-embedding-3-small".to_string());
        let dimensions = match model.as_str() {
            "text-embedding-3-large" => 3072,
            "text-embedding-3-small" => 1536,
            "text-embedding-ada-002" => 1536,
            _ => 1536,
        };
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            dimensions,
        }
    }
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[async_trait]
impl EmbeddingProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn max_batch_size(&self) -> usize {
        2048
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let resp: EmbeddingResponse = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "input": texts,
                "model": self.model,
            }))
            .send()
            .await
            .context("OpenAI embedding request failed")?
            .error_for_status()
            .context("OpenAI embedding request returned error")?
            .json()
            .await
            .context("Failed to parse OpenAI embedding response")?;

        Ok(resp.data.into_iter().map(|d| d.embedding).collect())
    }
}
