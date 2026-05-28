use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;

use crate::provider::EmbeddingProvider;

pub struct CohereProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dimensions: usize,
}

impl CohereProvider {
    pub fn new(api_key: String, model: Option<String>) -> Self {
        let model = model.unwrap_or_else(|| "embed-english-v3.0".to_string());
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            dimensions: 1024,
        }
    }
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embeddings: Vec<Vec<f32>>,
}

#[async_trait]
impl EmbeddingProvider for CohereProvider {
    fn name(&self) -> &str {
        "cohere"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn max_batch_size(&self) -> usize {
        96
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let resp: EmbeddingResponse = self
            .client
            .post("https://api.cohere.ai/v1/embed")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "texts": texts,
                "model": self.model,
                "input_type": "search_document",
            }))
            .send()
            .await
            .context("Cohere embedding request failed")?
            .error_for_status()
            .context("Cohere embedding request returned error")?
            .json()
            .await
            .context("Failed to parse Cohere embedding response")?;

        Ok(resp.embeddings)
    }
}
