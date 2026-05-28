use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::snapshots::{Snapshot, SnapshotFile, SnapshotMeta};

pub struct IndexStore {
    base_dir: PathBuf,
}

impl IndexStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

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

    pub fn load_json<T: for<'de> Deserialize<'de>>(&self, relative_path: &str) -> Result<T> {
        let path = self.base_dir.join(relative_path);
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let data: T = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON from {}", path.display()))?;
        Ok(data)
    }

    pub fn exists(&self, relative_path: &str) -> bool {
        self.base_dir.join(relative_path).exists()
    }

    pub fn create_snapshot(&self, label: &str, files: &[PathBuf]) -> Result<Snapshot> {
        let id = uuid::Uuid::new_v4().to_string();
        let id = &id[..8];
        let snapshot_dir = self.base_dir.join("snapshots").join(id);
        std::fs::create_dir_all(&snapshot_dir)?;

        let mut snapshot_files = Vec::new();
        for file_path in files {
            let backup_name = file_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".into());
            let backup_path = snapshot_dir.join(&backup_name);
            if file_path.exists() {
                std::fs::copy(file_path, &backup_path).with_context(|| {
                    format!("Failed to backup {} to {}", file_path.display(), backup_path.display())
                })?;
            }
            snapshot_files.push(SnapshotFile {
                original_path: file_path.clone(),
                backup_path,
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

    pub fn rollback_snapshot(&self, snapshot: &Snapshot) -> Result<()> {
        for file in &snapshot.files {
            if file.backup_path.exists() {
                if let Some(parent) = file.original_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&file.backup_path, &file.original_path).with_context(|| {
                    format!("Failed to restore {} from {}", file.original_path.display(), file.backup_path.display())
                })?;
            }
        }
        tracing::info!("Rolled back snapshot '{}'", snapshot.label);
        Ok(())
    }

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
