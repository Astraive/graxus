use std::path::Path;

use anyhow::Result;

use crate::provider::EmbeddingProvider;
use crate::store::{content_hash, VectorRecord, VectorStore};

/// High-level pipeline that pairs an [`EmbeddingProvider`] with a [`VectorStore`].
///
/// Handles batching, deduplication by content hash, and search.
pub struct EmbeddingPipeline {
    provider: Box<dyn EmbeddingProvider>,
    store: VectorStore,
    batch_size: usize,
}

/// A single search result returned by [`EmbeddingPipeline::search`].
#[derive(Debug)]
pub struct SearchResult {
    /// Unique identifier of the matched record.
    pub id: String,
    /// Cosine similarity score in `[-1.0, 1.0]`.
    pub score: f32,
    /// The original text that was embedded.
    pub text: String,
}

/// Statistics returned by [`EmbeddingPipeline::embed_texts`].
#[derive(Debug)]
pub struct EmbedStats {
    /// Total items submitted.
    pub total: usize,
    /// Items that were successfully embedded and stored.
    pub embedded: usize,
    /// Items skipped because their content hash already existed in the store.
    pub skipped: usize,
    /// Items that failed due to provider errors.
    pub errors: usize,
}

impl EmbeddingPipeline {
    /// Create a new pipeline with the given provider and batch size.
    ///
    /// The batch size controls how many texts are sent to the provider per
    /// request; it is typically set to the provider's [`EmbeddingProvider::max_batch_size`].
    pub fn new(provider: Box<dyn EmbeddingProvider>, batch_size: usize) -> Self {
        Self {
            provider,
            store: VectorStore::new(),
            batch_size,
        }
    }

    /// Embed a list of `(id, kind, text)` items and add them to the store.
    ///
    /// Items whose content hash already exists in the store are skipped.
    /// Returns statistics about what was embedded vs skipped.
    pub async fn embed_texts(
        &mut self,
        items: Vec<(String, String, String)>,
    ) -> Result<EmbedStats> {
        let mut stats = EmbedStats {
            total: items.len(),
            embedded: 0,
            skipped: 0,
            errors: 0,
        };

        // Filter to items not already embedded (by content hash)
        let existing_hashes: std::collections::HashSet<String> = self
            .store
            .records()
            .iter()
            .map(|r| r.content_hash.clone())
            .collect();

        let new_items: Vec<_> = items
            .into_iter()
            .filter(|(_, _, text)| {
                let hash = content_hash(text);
                if existing_hashes.contains(&hash) {
                    stats.skipped += 1;
                    false
                } else {
                    true
                }
            })
            .collect();

        // Process in batches
        for chunk in new_items.chunks(self.batch_size) {
            let texts: Vec<String> = chunk.iter().map(|(_, _, t)| t.clone()).collect();

            match self.provider.embed(&texts).await {
                Ok(vectors) => {
                    for ((id, kind, text), vector) in chunk.iter().zip(vectors) {
                        self.store.add(VectorRecord {
                            id: id.clone(),
                            kind: kind.clone(),
                            text: text.clone(),
                            vector,
                            content_hash: content_hash(text),
                        });
                        stats.embedded += 1;
                    }
                }
                Err(e) => {
                    tracing::error!("Embedding batch failed: {}", e);
                    stats.errors += chunk.len();
                }
            }
        }

        Ok(stats)
    }

    /// Embed a single query string for use in vector search.
    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let vectors = self.provider.embed(&[query.to_string()]).await?;
        vectors
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Empty embedding response"))
    }

    /// Search the store for vectors most similar to `query_vector`.
    pub fn search_vectors(&self, query_vector: &[f32], top_k: usize) -> Vec<SearchResult> {
        self.store
            .search(query_vector, top_k)
            .into_iter()
            .map(|(record, score)| SearchResult {
                id: record.id.clone(),
                score,
                text: record.text.clone(),
            })
            .collect()
    }

    /// Embed `query` then search the store for the `top_k` most similar records.
    pub async fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchResult>> {
        let query_vector = self.embed_query(query).await?;
        Ok(self.search_vectors(&query_vector, top_k))
    }

    /// Persist the underlying vector store to disk in binary format.
    pub fn save(&self, path: &Path) -> Result<()> {
        self.store.save(path)
    }

    /// Load a pipeline from a previously saved store file.
    ///
    /// Supports both binary and legacy JSON store files (see [`VectorStore::load`]).
    pub fn load(path: &Path, provider: Box<dyn EmbeddingProvider>) -> Result<Self> {
        let store = VectorStore::load(path)?;
        let batch_size = provider.max_batch_size();
        Ok(Self {
            provider,
            store,
            batch_size,
        })
    }

    /// Borrow the underlying [`VectorStore`].
    pub fn store(&self) -> &VectorStore {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct MockProvider;

    #[async_trait]
    impl EmbeddingProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }
        fn model(&self) -> &str {
            "mock-model"
        }
        fn dimensions(&self) -> usize {
            3
        }
        fn max_batch_size(&self) -> usize {
            10
        }
        async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
        }
    }

    #[tokio::test]
    async fn test_pipeline_embed_and_search() -> Result<()> {
        let provider = Box::new(MockProvider);
        let mut pipeline = EmbeddingPipeline::new(provider, 10);

        let items = vec![
            ("doc:1".into(), "doc".into(), "goodbye world".into()),
            ("doc:2".into(), "doc".into(), "goodbye world".into()),
        ];

        let stats = pipeline.embed_texts(items).await?;
        assert_eq!(stats.embedded, 2);
        assert_eq!(stats.skipped, 0);

        let results = pipeline.search("goodbye", 1).await?;
        assert_eq!(results.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_pipeline_skip_duplicates() -> Result<()> {
        let provider = Box::new(MockProvider);
        let mut pipeline = EmbeddingPipeline::new(provider, 10);

        let items = vec![("doc:1".into(), "doc".into(), "goodbye".into())];
        pipeline.embed_texts(items).await?;

        let items2 = vec![("doc:1".into(), "doc".into(), "goodbye".into())];
        let stats = pipeline.embed_texts(items2).await?;
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.embedded, 0);
        Ok(())
    }
}