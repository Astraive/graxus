use async_trait::async_trait;

/// Trait for embedding providers (OpenAI, Cohere, Ollama, etc.).
///
/// Each implementation wraps a remote (or local) embedding API behind a
/// uniform interface so that [`EmbeddingPipeline`](crate::EmbeddingPipeline)
/// can work with any provider.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Provider name (e.g. `"openai"`, `"cohere"`, `"ollama"`).
    fn name(&self) -> &str;

    /// Model name (e.g. `"text-embedding-3-small"`).
    fn model(&self) -> &str;

    /// Embedding dimensions produced by this provider/model combination.
    fn dimensions(&self) -> usize;

    /// Maximum number of texts that can be sent in a single batch request.
    fn max_batch_size(&self) -> usize;

    /// Embed a batch of texts into vectors.
    ///
    /// The returned vector must have the same length as `texts`, with each
    /// inner vector having exactly [`dimensions`](Self::dimensions) elements.
    async fn embed(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>>;
}
