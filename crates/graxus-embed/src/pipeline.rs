use std::path::Path;

use anyhow::Result;

use crate::provider::EmbeddingProvider;
use crate::store::{content_hash, VectorRecord, VectorStore};

pub struct EmbeddingPipeline {
    provider: Box<dyn EmbeddingProvider>,
    store: VectorStore,
    batch_size: usize,
}

#[derive(Debug)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub text: String,
}

#[derive(Debug)]
pub struct EmbedStats {
    pub total: usize,
    pub embedded: usize,
    pub skipped: usize,
    pub errors: usize,
}

impl EmbeddingPipeline {
    pub fn new(provider: Box<dyn EmbeddingProvider>, batch_size: usize) -> Self {
        Self {
            provider,
            store: VectorStore::new(),
            batch_size,
        }
    }

    /// Embed a list of (id, kind, text) items and add to the store.
    /// Returns stats about what was embedded vs skipped.
    pub async fn embed_texts(&mut self, items: Vec<(String, String, String)>) -> Result<EmbedStats> {
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
                    for ((id, kind, text), vector) in chunk.iter().zip(vectors.into_iter()) {
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

    /// Embed a single query string for search.
    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let vectors = self.provider.embed(&[query.to_string()]).await?;
        vectors
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Empty embedding response"))
    }

    /// Search the store for similar vectors.
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

    /// Full search: embed query then search store.
    pub async fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchResult>> {
        let query_vector = self.embed_query(query).await?;
        Ok(self.search_vectors(&query_vector, top_k))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.store.save(path)
    }

    pub fn load(path: &Path, provider: Box<dyn EmbeddingProvider>) -> Result<Self> {
        let store = VectorStore::load(path)?;
        let batch_size = provider.max_batch_size();
        Ok(Self {
            provider,
            store,
            batch_size,
        })
    }

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
        fn name(&self) -> &str { "mock" }
        fn model(&self) -> &str { "mock-model" }
        fn dimensions(&self) -> usize { 3 }
        fn max_batch_size(&self) -> usize { 10 }
        async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
        }
    }

    #[tokio::test]
    async fn test_pipeline_embed_and_search() {
        let provider = Box::new(MockProvider);
        let mut pipeline = EmbeddingPipeline::new(provider, 10);

        let items = vec![
            ("doc:1".into(), "doc".into(), "hello world".into()),
            ("doc:2".into(), "doc".into(), "goodbye world".into()),
        ];

        let stats = pipeline.embed_texts(items).await.unwrap();
        assert_eq!(stats.embedded, 2);
        assert_eq!(stats.skipped, 0);

        let results = pipeline.search("hello", 1).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_pipeline_skip_duplicates() {
        let provider = Box::new(MockProvider);
        let mut pipeline = EmbeddingPipeline::new(provider, 10);

        let items = vec![("doc:1".into(), "doc".into(), "hello".into())];
        pipeline.embed_texts(items).await.unwrap();

        let items2 = vec![("doc:1".into(), "doc".into(), "hello".into())];
        let stats = pipeline.embed_texts(items2).await.unwrap();
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.embedded, 0);
    }
}
