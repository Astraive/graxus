//! Concrete LLM provider implementations.
//!
//! Each provider wraps an HTTP client configured for a specific LLM API:
//!
//! - [`openai::OpenAiProvider`] -- OpenAI Chat Completions API
//! - [`anthropic::AnthropicProvider`] -- Anthropic Messages API
//! - [`ollama::OllamaProvider`] -- Ollama local inference API

pub mod anthropic;
pub mod ollama;
pub mod openai;
