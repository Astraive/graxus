//! File-system based JSON storage with snapshot support.
//!
//! [`IndexStore`] writes serialisable data to JSON files under a base directory,
//! and supports creating/restoring snapshots of those files.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::snapshots::{Snapshot, SnapshotFile, SnapshotMeta};

/// File-system backed JSON store with snapshot/rollback support.
///
/// All paths passed to the store's methods are relative to the configured
/// `base_dir`. Parent directories are created automatically on write.
pub struct IndexStore {
    base_dir: PathBuf,
}

impl IndexStore {
    /// Create a new store rooted at `base_dir`.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Return the base directory of this store.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Save `data` as pretty-printed JSON at `relative_path` under the base directory.
    pub fn save_json<T: Serialize>(&self, relative_path: &str, data: &T) -> Result<()> {
        let path = self.base_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(data)?;
        std::fs::write(&path, json)?;
        tracing::debug!("Saved JSON to {}", path.display());
        Ok(())
    }

    /// Save `data` as compact (single-line) JSON at `relative_path`.
    ///
    /// Use this for internal storage files that humans rarely read directly.
    pub fn save_json_compact<T: Serialize>(&self, relative_path: &str, data: &T) -> Result<()> {
        let path = self.base_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(data)?;
        std::fs::write(&path, json)?;
        tracing::debug!("Saved compact JSON to {}", path.display());
        Ok(())
    }

    /// Load and deserialize JSON from `relative_path` under the base directory.
    pub fn load_json<T: for<'de> Deserialize<'de>>(&self, relative_path: &str) -> Result<T> {
        let path = self.base_dir.join(relative_path);
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let data: T = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON from {}", path.display()))?;
        Ok(data)
    }

    /// Check whether a file exists at `relative_path`.
    pub fn exists(&self, relative_path: &str) -> bool {
        self.base_dir.join(relative_path).exists()
    }

    /// Create a named snapshot by copying the given files into a snapshot directory.
    ///
    /// Returns the [`Snapshot`] metadata describing what was backed up.
    pub fn create_snapshot(&self, label: &str, files: &[PathBuf]) -> Result<Snapshot> {
        let id = uuid::Uuid::new_v4().to_string();
        let id = &id[..12];
        let snapshot_dir = self.base_dir.join("snapshots").join(id);
        std::fs::create_dir_all(&snapshot_dir)?;

        let mut snapshot_files = Vec::new();
        for file_path in files {
            // Use a sanitized version of the full path to avoid name collisions
            // when two files in different directories have the same filename.
            let backup_name = file_path.to_string_lossy().replace(['\\', '/', ':'], "_");
            let backup_path = snapshot_dir.join(&backup_name);
            let mut checksum = String::new();
            if file_path.exists() {
                std::fs::copy(file_path, &backup_path).with_context(|| {
                    format!(
                        "Failed to backup {} to {}",
                        file_path.display(),
                        backup_path.display()
                    )
                })?;
                // Compute SHA-256 checksum of the backup for integrity verification
                use sha2::{Digest, Sha256};
                let contents = std::fs::read(&backup_path)?;
                let hash = Sha256::digest(&contents);
                checksum = format!("{:x}", hash);
            }
            snapshot_files.push(SnapshotFile {
                original_path: file_path.clone(),
                backup_path,
                checksum,
            });
        }

        let snapshot = Snapshot {
            id: id.to_string(),
            label: label.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            files: snapshot_files,
        };

        let meta_path = snapshot_dir.join("meta.json");
        let meta_json = serde_json::to_string_pretty(&snapshot)?;
        std::fs::write(meta_path, meta_json)?;

        tracing::info!("Created snapshot '{}' with {} files", label, files.len());
        Ok(snapshot)
    }

    /// Restore all files from a previously created snapshot.
    ///
    /// Verifies backup file integrity using stored SHA-256 checksums when available.
    pub fn rollback_snapshot(&self, snapshot: &Snapshot) -> Result<()> {
        for file in &snapshot.files {
            if file.backup_path.exists() {
                // Verify checksum if one was stored during snapshot creation
                if !file.checksum.is_empty() {
                    use sha2::{Digest, Sha256};
                    let contents = std::fs::read(&file.backup_path).with_context(|| {
                        format!(
                            "Failed to read backup {} for checksum verification",
                            file.backup_path.display()
                        )
                    })?;
                    let hash = format!("{:x}", Sha256::digest(&contents));
                    if hash != file.checksum {
                        anyhow::bail!(
                            "Backup integrity check failed for {}: expected {} but got {}. \
                             The backup file may have been corrupted.",
                            file.original_path.display(),
                            file.checksum,
                            hash
                        );
                    }
                }

                if let Some(parent) = file.original_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&file.backup_path, &file.original_path).with_context(|| {
                    format!(
                        "Failed to restore {} from {}",
                        file.original_path.display(),
                        file.backup_path.display()
                    )
                })?;
            }
        }
        tracing::info!("Rolled back snapshot '{}'", snapshot.label);
        Ok(())
    }

    /// List all snapshots stored under the base directory.
    pub fn list_snapshots(&self) -> Result<Vec<SnapshotMeta>> {
        let snapshots_dir = self.base_dir.join("snapshots");
        if !snapshots_dir.exists() {
            return Ok(Vec::new());
        }
        let mut metas = Vec::new();
        for entry in std::fs::read_dir(&snapshots_dir)? {
            let entry = entry?;
            let meta_path = entry.path().join("meta.json");
            if meta_path.exists() {
                let content = std::fs::read_to_string(&meta_path)?;
                let snapshot: Snapshot = serde_json::from_str(&content)?;
                metas.push(SnapshotMeta {
                    id: snapshot.id,
                    label: snapshot.label,
                    created_at: snapshot.created_at,
                    file_count: snapshot.files.len(),
                });
            }
        }
        Ok(metas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("graxus-store-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        name: String,
        count: usize,
    }

    #[test]
    fn test_save_and_load_json() {
        let base = temp_dir();
        let store = IndexStore::new(base.clone());

        let data = TestData {
            name: "test".into(),
            count: 42,
        };
        store.save_json("data/test.json", &data).unwrap();

        assert!(store.exists("data/test.json"));

        let loaded: TestData = store.load_json("data/test.json").unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_save_json_compact() {
        let base = temp_dir();
        let store = IndexStore::new(base.clone());

        let data = TestData {
            name: "compact".into(),
            count: 7,
        };
        store.save_json_compact("data/compact.json", &data).unwrap();

        let raw = std::fs::read_to_string(base.join("data/compact.json")).unwrap();
        // Compact JSON should be a single line (no newlines in the data)
        assert!(!raw.contains('\n'));
        assert!(raw.contains("\"name\":\"compact\""));

        let loaded: TestData = store.load_json("data/compact.json").unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_save_json_pretty_has_newlines() {
        let base = temp_dir();
        let store = IndexStore::new(base.clone());

        let data = TestData {
            name: "pretty".into(),
            count: 1,
        };
        store.save_json("data/pretty.json", &data).unwrap();

        let raw = std::fs::read_to_string(base.join("data/pretty.json")).unwrap();
        assert!(raw.contains('\n'));
    }

    #[test]
    fn test_exists_false_for_missing() {
        let base = temp_dir();
        let store = IndexStore::new(base);
        assert!(!store.exists("nonexistent.json"));
    }

    #[test]
    fn test_create_snapshot_and_rollback() {
        let base = temp_dir();
        let store = IndexStore::new(base.clone());

        // Create source files
        let src_dir = base.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let file_a = src_dir.join("a.txt");
        let file_b = src_dir.join("b.txt");
        std::fs::write(&file_a, "original_a").unwrap();
        std::fs::write(&file_b, "original_b").unwrap();

        // Snapshot
        let snapshot = store
            .create_snapshot("before-edit", &[file_a.clone(), file_b.clone()])
            .unwrap();
        assert_eq!(snapshot.files.len(), 2);
        assert_eq!(snapshot.label, "before-edit");

        // Modify files
        std::fs::write(&file_a, "modified_a").unwrap();
        std::fs::write(&file_b, "modified_b").unwrap();

        // Rollback
        store.rollback_snapshot(&snapshot).unwrap();

        assert_eq!(std::fs::read_to_string(&file_a).unwrap(), "original_a");
        assert_eq!(std::fs::read_to_string(&file_b).unwrap(), "original_b");
    }

    #[test]
    fn test_list_snapshots() {
        let base = temp_dir();
        let store = IndexStore::new(base.clone());

        let file = base.join("test.txt");
        std::fs::write(&file, "content").unwrap();

        store
            .create_snapshot("snap1", std::slice::from_ref(&file))
            .unwrap();
        store.create_snapshot("snap2", &[file]).unwrap();

        let metas = store.list_snapshots().unwrap();
        assert_eq!(metas.len(), 2);
        let labels: Vec<&str> = metas.iter().map(|m| m.label.as_str()).collect();
        assert!(labels.contains(&"snap1"));
        assert!(labels.contains(&"snap2"));
    }

    #[test]
    fn test_list_snapshots_empty() {
        let base = temp_dir();
        let store = IndexStore::new(base);
        let metas = store.list_snapshots().unwrap();
        assert!(metas.is_empty());
    }
}
