//! Graxus edit — Safe find/replace engine with preview and snapshot-based rollback.

pub mod find;
pub mod replace;
pub mod safety;

use anyhow::Result;
use graxus_core::ScannedFile;
use graxus_index::IndexStore;
use graxus_index::Snapshot;

pub use find::{SearchHit, SearchMode};
pub use replace::{FileChange, ReplaceMode, ReplacePreview};

/// High-level edit engine combining search, replace, and safety.
pub struct EditEngine {
    store: IndexStore,
    max_files: usize,
}

impl EditEngine {
    pub fn new(store: IndexStore, max_files: usize) -> Self {
        Self { store, max_files }
    }

    /// Search for a pattern across scanned files.
    pub fn find(
        &self,
        pattern: &str,
        files: &[ScannedFile],
        mode: SearchMode,
    ) -> Result<Vec<SearchHit>> {
        find::search(pattern, files, &mode)
    }

    /// Preview a replace operation without modifying files.
    pub fn preview_replace(
        &self,
        old: &str,
        new: &str,
        files: &[ScannedFile],
        mode: ReplaceMode,
    ) -> Result<ReplacePreview> {
        replace::preview_replace(old, new, files, &mode, self.max_files)
    }

    /// Apply a replace operation with a snapshot for rollback.
    pub fn apply_replace(
        &self,
        preview: &ReplacePreview,
        label: &str,
    ) -> Result<Snapshot> {
        // Collect files to snapshot
        let files: Vec<std::path::PathBuf> = preview
            .affected_files
            .iter()
            .map(|c| std::path::PathBuf::from(&c.file))
            .collect();

        // Create snapshot before mutation
        let snapshot = self.store.create_snapshot(label, &files)?;

        // Apply the replacement
        replace::apply_replace(preview)?;

        Ok(snapshot)
    }

    /// Rollback a snapshot to undo a replace operation.
    pub fn rollback(&self, snapshot: &Snapshot) -> Result<()> {
        self.store.rollback_snapshot(snapshot)
    }
}
