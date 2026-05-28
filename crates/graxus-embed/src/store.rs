use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub vector: Vec<f32>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStore {
    records: Vec<VectorRecord>,
}

impl VectorStore {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn add(&mut self, record: VectorRecord) {
        self.records.push(record);
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn records(&self) -> &[VectorRecord] {
        &self.records
    }

    /// Search for the top_k most similar vectors to the query.
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<(&VectorRecord, f32)> {
        let mut scored: Vec<(&VectorRecord, f32)> = self
            .records
            .iter()
            .map(|r| (r, cosine_similarity(query, &r.vector)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(top_k).collect()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json).context("Failed to write vector store")?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).context("Failed to read vector store")?;
        let store: Self = serde_json::from_str(&content).context("Failed to parse vector store")?;
        Ok(store)
    }
}

/// Compute cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Compute SHA-256 hash of a string, returned as hex.
pub fn content_hash(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_content_hash_different() {
        let h1 = content_hash("hello");
        let h2 = content_hash("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_vector_store_roundtrip() {
        let mut store = VectorStore::new();
        store.add(VectorRecord {
            id: "test:1".into(),
            kind: "doc".into(),
            text: "hello".into(),
            vector: vec![1.0, 0.0, 0.0],
            content_hash: "abc".into(),
        });

        let dir = std::env::temp_dir().join("graxus_test_store");
        let path = dir.join("vectors.json");
        store.save(&path).unwrap();

        let loaded = VectorStore::load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.records()[0].id, "test:1");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_vector_store_search() {
        let mut store = VectorStore::new();
        store.add(VectorRecord {
            id: "a".into(),
            kind: "doc".into(),
            text: "hello".into(),
            vector: vec![1.0, 0.0, 0.0],
            content_hash: "a".into(),
        });
        store.add(VectorRecord {
            id: "b".into(),
            kind: "doc".into(),
            text: "world".into(),
            vector: vec![0.0, 1.0, 0.0],
            content_hash: "b".into(),
        });

        let query = vec![1.0, 0.0, 0.0];
        let results = store.search(&query, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, "a");
        assert!((results[0].1 - 1.0).abs() < 1e-6);
    }
}
