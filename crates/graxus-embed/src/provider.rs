use async_trait::async_trait;

/// Trait for embedding providers (OpenAI, Cohere, Ollama, etc.)
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Provider name (e.g. "openai", "cohere", "ollama")
    fn name(&self) -> &str;

    /// Model name (e.g. "text-embedding-3-small")
    fn model(&self) -> &str;

    /// Embedding dimensions
    fn dimensions(&self) -> usize;

    /// Max texts per batch request
    fn max_batch_size(&self) -> usize;

    /// Embed a batch of texts into vectors
    async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>>;
}
