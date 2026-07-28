use std::io::{Cursor, Read, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Magic bytes identifying the binary vector store format: "GRVX".
const MAGIC: &[u8; 4] = b"GRVX";

/// Current binary format version.
const FORMAT_VERSION: u32 = 1;

/// A single embedding record containing the source id, kind, text, vector, and content hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    /// Unique identifier for the source item (e.g. "file:src/main.rs:fn_name").
    pub id: String,
    /// Semantic kind of the record (e.g. "function", "struct", "doc").
    pub kind: String,
    /// The original text that was embedded.
    pub text: String,
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// SHA-256 hash of `text`, used for deduplication.
    pub content_hash: String,
}

/// In-memory store of embedding vectors with search and serialization support.
///
/// Stores records in memory and supports efficient binary serialization on disk.
/// On load, automatically migrates legacy JSON files to binary format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStore {
    records: Vec<VectorRecord>,
}

impl Default for VectorStore {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorStore {
    /// Create an empty vector store.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Add a record to the store.
    pub fn add(&mut self, record: VectorRecord) {
        self.records.push(record);
    }

    /// Return the number of records in the store.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Return true if the store contains no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Borrow all records in the store.
    pub fn records(&self) -> &[VectorRecord] {
        &self.records
    }

    /// Search for the `top_k` most similar vectors to the query using cosine similarity.
    ///
    /// Returns `(record, score)` pairs sorted by descending similarity.
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<(&VectorRecord, f32)> {
        let mut scored: Vec<(&VectorRecord, f32)> = self
            .records
            .iter()
            .map(|r| (r, cosine_similarity(query, &r.vector)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(top_k).collect()
    }

    /// Save the store to disk in binary format.
    ///
    /// Creates parent directories if needed. See [`VectorStore::load`] for the
    /// corresponding reader which also handles legacy JSON files.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = self.to_binary()?;
        std::fs::write(path, &bytes).context("Failed to write vector store")?;
        Ok(())
    }

    /// Load a store from disk.
    ///
    /// Automatically detects and loads both the current binary format (v1) and
    /// legacy JSON-serialized stores. JSON files are parsed transparently;
    /// re-saving with [`VectorStore::save`] will persist them in binary format.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).context("Failed to read vector store")?;

        // Binary format: starts with "GRVX" magic bytes.
        if bytes.len() >= 4 && bytes[..4] == *MAGIC {
            return Self::from_binary(&bytes);
        }

        // Legacy JSON migration: treat the entire file as UTF-8 JSON.
        let content = std::str::from_utf8(&bytes)
            .context("Vector store file is neither valid binary nor valid UTF-8")?;
        let store: Self =
            serde_json::from_str(content).context("Failed to parse vector store as JSON")?;
        Ok(store)
    }

    /// Serialize the store into the binary wire format.
    ///
    /// Layout: `MAGIC (4) | version (u32 LE) | count (u32 LE)`
    /// followed by per-record: `id | kind | text | content_hash | vector_len | vector_data`,
    /// where each string is `len (u32 LE) | bytes` and vectors are raw `f32` little-endian.
    fn to_binary(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // Header
        buf.write_all(MAGIC)?;
        buf.write_all(&FORMAT_VERSION.to_le_bytes())?;
        let record_count: u32 = self
            .records
            .len()
            .try_into()
            .context("Too many records for binary format (max 4,294,967,295)")?;
        buf.write_all(&record_count.to_le_bytes())?;

        // Records
        for record in &self.records {
            write_string(&mut buf, &record.id)?;
            write_string(&mut buf, &record.kind)?;
            write_string(&mut buf, &record.text)?;
            write_string(&mut buf, &record.content_hash)?;

            let vec_len: u32 = record
                .vector
                .len()
                .try_into()
                .context("Vector too large for binary format (max 4,294,967,295 elements)")?;
            buf.write_all(&vec_len.to_le_bytes())?;
            for &f in &record.vector {
                buf.write_all(&f.to_le_bytes())?;
            }
        }

        Ok(buf)
    }

    /// Deserialize a store from the binary wire format produced by [`to_binary`].
    fn from_binary(bytes: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(bytes);

        let mut magic = [0u8; 4];
        cursor.read_exact(&mut magic)?;
        if magic != *MAGIC {
            anyhow::bail!("Invalid magic bytes in vector store");
        }

        let version = read_u32(&mut cursor)?;
        if version != FORMAT_VERSION {
            anyhow::bail!("Unsupported vector store format version: {}", version);
        }

        let record_count = read_u32(&mut cursor)? as usize;
        // Safety limit: prevent DoS via crafted files with huge record counts
        const MAX_RECORDS: usize = 10_000_000;
        if record_count > MAX_RECORDS {
            anyhow::bail!(
                "Vector store has too many records: {} (max {})",
                record_count,
                MAX_RECORDS
            );
        }

        let mut records = Vec::with_capacity(record_count);

        for _ in 0..record_count {
            let id = read_string(&mut cursor)?;
            let kind = read_string(&mut cursor)?;
            let text = read_string(&mut cursor)?;
            let content_hash = read_string(&mut cursor)?;

            let vector_len = read_u32(&mut cursor)? as usize;
            // Safety limit: prevent DoS via crafted files with huge vectors
            const MAX_VECTOR_LEN: usize = 100_000;
            if vector_len > MAX_VECTOR_LEN {
                anyhow::bail!(
                    "Vector too long: {} elements (max {})",
                    vector_len,
                    MAX_VECTOR_LEN
                );
            }
            let mut vector = Vec::with_capacity(vector_len);
            for _ in 0..vector_len {
                let mut buf = [0u8; 4];
                cursor.read_exact(&mut buf)?;
                vector.push(f32::from_le_bytes(buf));
            }

            records.push(VectorRecord {
                id,
                kind,
                text,
                vector,
                content_hash,
            });
        }

        Ok(Self { records })
    }
}

// ---------------------------------------------------------------------------
// Binary helpers
// ---------------------------------------------------------------------------

/// Write a length-prefixed UTF-8 string (u32 LE length + raw bytes).
fn write_string(buf: &mut Vec<u8>, s: &str) -> Result<()> {
    let bytes = s.as_bytes();
    let len: u32 = bytes
        .len()
        .try_into()
        .context("String too long for binary format (max 4,294,967,295 bytes)")?;
    buf.write_all(&len.to_le_bytes())?;
    buf.write_all(bytes)?;
    Ok(())
}

/// Read a length-prefixed UTF-8 string from a cursor.
fn read_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
    let len = read_u32(cursor)? as usize;
    let mut bytes = vec![0u8; len];
    cursor.read_exact(&mut bytes)?;
    String::from_utf8(bytes).context("Invalid UTF-8 in vector store string field")
}

/// Read a little-endian u32 from a cursor.
fn read_u32(cursor: &mut Cursor<&[u8]>) -> Result<u32> {
    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

// ---------------------------------------------------------------------------
// Public utility functions
// ---------------------------------------------------------------------------

/// Compute cosine similarity between two equal-length vectors.
///
/// Returns `0.0` for empty or mismatched-length inputs.
#[must_use]
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

/// Compute SHA-256 hash of a string, returned as lowercase hex.
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
        let h1 = content_hash("goodbye world");
        let h2 = content_hash("goodbye world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_content_hash_different() {
        let h1 = content_hash("goodbye");
        let h2 = content_hash("world");
        assert_ne!(h1, h2);
    }

    // ------------------------------------------------------------------
    // Binary format tests
    // ------------------------------------------------------------------

    #[test]
    fn test_binary_save_load() -> Result<()> {
        let mut store = VectorStore::new();
        store.add(VectorRecord {
            id: "test:1".into(),
            kind: "doc".into(),
            text: "goodbye world".into(),
            vector: vec![1.0, 0.0, 0.5],
            content_hash: "abc123".into(),
        });
        store.add(VectorRecord {
            id: "test:2".into(),
            kind: "fn".into(),
            text: "goodbye".into(),
            vector: vec![0.0, 1.0, -0.5],
            content_hash: "def456".into(),
        });

        let dir = std::env::temp_dir().join("graxus_test_binary_save_load");
        let path = dir.join("vectors.bin");
        store.save(&path)?;

        let loaded = VectorStore::load(&path)?;
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.records()[0].id, "test:1");
        assert_eq!(loaded.records()[0].kind, "doc");
        assert_eq!(loaded.records()[0].text, "goodbye world");
        assert_eq!(loaded.records()[0].vector, vec![1.0, 0.0, 0.5]);
        assert_eq!(loaded.records()[0].content_hash, "abc123");
        assert_eq!(loaded.records()[1].id, "test:2");
        assert_eq!(loaded.records()[1].vector, vec![0.0, 1.0, -0.5]);

        // Verify the file starts with magic bytes (not JSON).
        let raw = std::fs::read(&path)?;
        assert_eq!(&raw[..4], b"GRVX");

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn test_json_migration() -> Result<()> {
        // Simulate a legacy JSON-serialized store file.
        let mut store = VectorStore::new();
        store.add(VectorRecord {
            id: "legacy:1".into(),
            kind: "doc".into(),
            text: "migrated".into(),
            vector: vec![0.25, 0.75],
            content_hash: "hash_legacy".into(),
        });

        let dir = std::env::temp_dir().join("graxus_test_json_migration");
        let path = dir.join("vectors.json");
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(&store)?;
        std::fs::write(&path, &json)?;

        // Load should succeed via JSON migration path.
        let loaded = VectorStore::load(&path)?;
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.records()[0].id, "legacy:1");
        assert_eq!(loaded.records()[0].vector, vec![0.25, 0.75]);

        // Re-save in binary format.
        loaded.save(&path)?;

        // Verify it's now binary.
        let raw = std::fs::read(&path)?;
        assert_eq!(&raw[..4], b"GRVX");

        // Load again -- should still work.
        let reloaded = VectorStore::load(&path)?;
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded.records()[0].id, "legacy:1");

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn test_search_ordering() -> Result<()> {
        let mut store = VectorStore::new();
        store.add(VectorRecord {
            id: "a".into(),
            kind: "doc".into(),
            text: "orthogonal".into(),
            vector: vec![0.0, 1.0, 0.0],
            content_hash: "a".into(),
        });
        store.add(VectorRecord {
            id: "b".into(),
            kind: "doc".into(),
            text: "best match".into(),
            vector: vec![1.0, 0.0, 0.0],
            content_hash: "b".into(),
        });
        store.add(VectorRecord {
            id: "c".into(),
            kind: "doc".into(),
            text: "partial match".into(),
            vector: vec![0.7, 0.7, 0.0],
            content_hash: "c".into(),
        });

        let query = vec![1.0, 0.0, 0.0];
        let results = store.search(&query, 3);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0.id, "b");
        assert_eq!(results[1].0.id, "c");
        assert_eq!(results[2].0.id, "a");
        // Scores must be in descending order.
        assert!(results[0].1 >= results[1].1);
        assert!(results[1].1 >= results[2].1);

        Ok(())
    }

    #[test]
    fn test_search_top_k_limit() -> Result<()> {
        let mut store = VectorStore::new();
        for i in 0..10 {
            store.add(VectorRecord {
                id: format!("r:{}", i),
                kind: "doc".into(),
                text: format!("text {}", i),
                vector: vec![i as f32, 0.0, 0.0],
                content_hash: format!("h:{}", i),
            });
        }

        let query = vec![1.0, 0.0, 0.0];
        let results = store.search(&query, 3);
        assert_eq!(results.len(), 3);

        Ok(())
    }

    #[test]
    fn test_dedup_by_content_hash() -> Result<()> {
        let mut store = VectorStore::new();
        store.add(VectorRecord {
            id: "a".into(),
            kind: "doc".into(),
            text: "same content".into(),
            vector: vec![1.0, 0.0],
            content_hash: "duplicate_hash".into(),
        });
        store.add(VectorRecord {
            id: "b".into(),
            kind: "doc".into(),
            text: "same content".into(),
            vector: vec![1.0, 0.0],
            content_hash: "duplicate_hash".into(),
        });

        // Deduplicate by collecting unique content hashes.
        let mut seen = std::collections::HashSet::new();
        let unique: Vec<_> = store
            .records()
            .iter()
            .filter(|r| seen.insert(r.content_hash.clone()))
            .collect();
        assert_eq!(unique.len(), 1);

        Ok(())
    }

    #[test]
    fn test_empty_store_binary_roundtrip() -> Result<()> {
        let store = VectorStore::new();
        let dir = std::env::temp_dir().join("graxus_test_empty_roundtrip");
        let path = dir.join("vectors.bin");
        store.save(&path)?;

        let loaded = VectorStore::load(&path)?;
        assert_eq!(loaded.len(), 0);
        assert!(loaded.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn test_unicode_text_roundtrip() -> Result<()> {
        let mut store = VectorStore::new();
        store.add(VectorRecord {
            id: "uni:1".into(),
            kind: "doc".into(),
            text: "Héllo Wörld 日本語".into(),
            vector: vec![1.0, 2.0],
            content_hash: "uni".into(),
        });

        let dir = std::env::temp_dir().join("graxus_test_unicode");
        let path = dir.join("vectors.bin");
        store.save(&path)?;

        let loaded = VectorStore::load(&path)?;
        assert_eq!(loaded.records()[0].text, "Héllo Wörld 日本語");

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}