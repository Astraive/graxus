//! Embedding pipeline for graxus.
//!
//! Provides vector embedding generation (via pluggable providers) and
//! efficient binary vector storage with cosine-similarity search.

pub mod pipeline;
pub mod provider;
pub mod providers;
pub mod store;

pub use pipeline::{EmbedStats, EmbeddingPipeline, SearchResult};
pub use provider::EmbeddingProvider;
pub use store::{VectorRecord, VectorStore};
