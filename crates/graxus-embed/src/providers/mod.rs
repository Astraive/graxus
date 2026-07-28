//! Concrete [`EmbeddingProvider`](crate::EmbeddingProvider) implementations.

pub mod cohere;
pub mod ollama;
pub mod openai;

pub use cohere::CohereProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAIProvider;
