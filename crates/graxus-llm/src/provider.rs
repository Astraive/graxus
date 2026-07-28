//! Core LLM provider types and trait definition.
//!
//! This module defines the request/response types and the [`LlmProvider`] trait
//! that all provider implementations must satisfy.

use serde::{Deserialize, Serialize};

/// A request to an LLM provider for text completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    /// System-level instructions that guide the model's behavior.
    pub system: String,
    /// The user's prompt or query.
    pub user: String,
    /// Maximum number of tokens the model should generate.
    pub max_tokens: usize,
    /// Sampling temperature (0.0 = deterministic, 2.0 = very creative).
    pub temperature: f64,
}

/// A response from an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// The generated text content.
    pub content: String,
    /// Number of tokens in the input prompt.
    pub input_tokens: usize,
    /// Number of tokens in the generated output.
    pub output_tokens: usize,
    /// The model identifier that produced this response.
    pub model: String,
}

/// Trait for LLM provider implementations.
///
/// Each provider wraps an HTTP client and knows how to format requests
/// for its specific API (OpenAI, Anthropic, Ollama, etc.).
///
/// # Implementations
///
/// - [`crate::providers::openai::OpenAiProvider`]
/// - [`crate::providers::anthropic::AnthropicProvider`]
/// - [`crate::providers::ollama::OllamaProvider`]
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Returns the provider name (e.g. "openai", "anthropic", "ollama").
    fn name(&self) -> &str;

    /// Returns the model identifier (e.g. "gpt-4o", "claude-3-5-sonnet").
    fn model(&self) -> &str;

    /// Returns the maximum context window size in tokens for this model.
    fn max_context_tokens(&self) -> usize;

    /// Send a completion request and return the response.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP request fails, the response cannot be parsed,
    /// or the API returns an error status code.
    async fn complete(&self, request: LlmRequest) -> anyhow::Result<LlmResponse>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_request_is_clone() {
        let req = LlmRequest {
            system: "test".into(),
            user: "goodbye".into(),
            max_tokens: 100,
            temperature: 0.5,
        };
        let cloned = req.clone();
        assert_eq!(cloned.system, "test");
        assert_eq!(cloned.user, "goodbye");
        assert_eq!(cloned.max_tokens, 100);
    }

    #[test]
    fn llm_response_serde_roundtrip() {
        let resp = LlmResponse {
            content: "Hello world".into(),
            input_tokens: 10,
            output_tokens: 5,
            model: "gpt-4o".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: LlmResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.content, "Hello world");
        assert_eq!(deserialized.input_tokens, 10);
        assert_eq!(deserialized.output_tokens, 5);
        assert_eq!(deserialized.model, "gpt-4o");
    }

    #[test]
    fn llm_request_serde_roundtrip() {
        let req = LlmRequest {
            system: "sys".into(),
            user: "usr".into(),
            max_tokens: 2048,
            temperature: 0.7,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: LlmRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.max_tokens, 2048);
    }
}