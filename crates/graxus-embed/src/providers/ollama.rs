use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;

use crate::provider::EmbeddingProvider;

/// Embedding provider backed by a local Ollama instance's `/api/embed` endpoint.
///
/// Defaults to `http://localhost:11434` and the `nomic-embed-text` model
/// (768 dimensions) when no values are specified.
pub struct OllamaProvider {
    client: reqwest::Client,
    endpoint: String,
    model: String,
    dimensions: usize,
}

impl OllamaProvider {
    /// Create a new Ollama provider.
    ///
    /// - `endpoint` defaults to `"http://localhost:11434"` when `None`.
    /// - `model` defaults to `"nomic-embed-text"` when `None`.
    pub fn new(endpoint: Option<String>, model: Option<String>) -> Self {
        let endpoint = endpoint.unwrap_or_else(|| "http://localhost:11434".to_string());
        let model = model.unwrap_or_else(|| "nomic-embed-text".to_string());
        Self {
            client: reqwest::Client::new(),
            endpoint,
            model,
            dimensions: 768,
        }
    }
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embeddings: Vec<Vec<f32>>,
}

#[async_trait]
impl EmbeddingProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn max_batch_size(&self) -> usize {
        64
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let resp: EmbeddingResponse = self
            .client
            .post(format!("{}/api/embed", self.endpoint))
            .json(&serde_json::json!({
                "model": self.model,
                "input": texts,
            }))
            .send()
            .await
            .context("Ollama embedding request failed")?
            .error_for_status()
            .context("Ollama embedding request returned error")?
            .json()
            .await
            .context("Failed to parse Ollama embedding response")?;

        Ok(resp.embeddings)
    }
}
