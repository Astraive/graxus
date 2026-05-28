use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub system: String,
    pub user: String,
    pub max_tokens: usize,
    pub temperature: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub model: String,
}

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    fn max_context_tokens(&self) -> usize;
    async fn complete(&self, request: LlmRequest) -> anyhow::Result<LlmResponse>;
}
