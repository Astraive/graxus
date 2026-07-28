use anyhow::{Context, Result};
use colored::Colorize;

use crate::context::CliContext;
use graxus_index::IndexStore;

/// Rollback file changes to a previous snapshot.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args; `--root` selects project)
/// * `snapshot_id` - Snapshot ID (or prefix) to restore
/// * `preview` - If true, show what would be restored
/// * `apply` - If true, actually restore the files
pub fn run(ctx: &CliContext, snapshot_id: &str, preview: bool, apply: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

    let store = IndexStore::new(root.join(".graxus"));
    let snapshots = store.list_snapshots()?;

    let meta = snapshots
        .iter()
        .find(|s| s.id.starts_with(snapshot_id))
        .context(format!(
            "Snapshot '{}' not found. Run `graxus history` to see available snapshots.",
            snapshot_id
        ))?;

    // Load full snapshot
    let snapshot_dir = root.join(".graxus").join("snapshots").join(&meta.id);
    let meta_path = snapshot_dir.join("meta.json");
    let content = std::fs::read_to_string(&meta_path)?;
    let snapshot: graxus_index::Snapshot = serde_json::from_str(&content)?;

    println!(
        "{}",
        format!("=== Snapshot: {} ===", snapshot.label)
            .green()
            .bold()
    );
    println!("  ID: {}", snapshot.id);
    println!("  Created: {}", snapshot.created_at);
    println!("  Files: {}", snapshot.files.len());

    println!("\n{}", "Files to restore:".cyan().bold());
    for file in &snapshot.files {
        let exists = file.backup_path.exists();
        let status = if exists { "✓".green() } else { "✗".red() };
        println!("  {} {}", status, file.original_path.display());
    }

    if apply {
        store.rollback_snapshot(&snapshot)?;
        println!("\n{}", "Rollback complete!".green().bold());
        println!("  Restored {} files.", snapshot.files.len());
    } else if preview {
        println!("\n{}", "Use --apply to restore these files.".yellow());
    } else {
        println!(
            "\n{}",
            "Use --preview to see files, --apply to restore.".yellow()
        );
    }

    Ok(())
}
