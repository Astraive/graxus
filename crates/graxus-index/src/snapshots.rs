//! Snapshot types for point-in-time file backups.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A complete snapshot containing the backed-up file list and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Short identifier (first 8 chars of a UUID).
    pub id: String,
    /// Human-readable label for this snapshot.
    pub label: String,
    /// RFC 3339 timestamp of when the snapshot was created.
    pub created_at: String,
    /// List of files included in this snapshot.
    pub files: Vec<SnapshotFile>,
}

/// Mapping between an original file and its backup copy within a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotFile {
    /// The original path of the file before backup.
    pub original_path: PathBuf,
    /// The path where the backup copy is stored.
    pub backup_path: PathBuf,
    /// SHA-256 hex digest of the backup file content, used for integrity verification.
    #[serde(default)]
    pub checksum: String,
}

/// Lightweight summary of a snapshot (without full file list).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    /// Short identifier.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// RFC 3339 timestamp.
    pub created_at: String,
    /// Number of files in the snapshot.
    pub file_count: usize,
}
