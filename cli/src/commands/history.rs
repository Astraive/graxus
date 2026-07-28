use anyhow::Result;
use colored::Colorize;

use crate::context::CliContext;

/// Show edit history (snapshots) for the project.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args; `--root` selects project)
/// * `file` - Optional file path to filter snapshots by
/// * `_limit` - Maximum number of entries to show
/// * `json` - Output as JSON
pub fn run(ctx: &CliContext, file: Option<&str>, _limit: usize, json: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

    let snapshots_dir = root.join(".graxus").join("snapshots");
    if !snapshots_dir.exists() {
        println!(
            "{}",
            "No snapshots found. No edits have been made.".yellow()
        );
        return Ok(());
    }

    let mut snapshots: Vec<serde_json::Value> = Vec::new();

    for entry in std::fs::read_dir(&snapshots_dir)? {
        let entry = entry?;
        let meta_path = entry.path().join("meta.json");
        if !meta_path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&meta_path)?;
        let snapshot: serde_json::Value = serde_json::from_str(&content)?;

        // Filter by file if specified
        if let Some(f) = file {
            let files = snapshot
                .get("files")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let has_file = files.iter().any(|sf| {
                sf.get("original_path")
                    .and_then(|v| v.as_str())
                    .map(|p| p.contains(f))
                    .unwrap_or(false)
            });
            if !has_file {
                continue;
            }
        }

        snapshots.push(snapshot);
    }

    // Sort by created_at
    snapshots.sort_by(|a, b| {
        let a_time = a.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
        let b_time = b.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
        b_time.cmp(a_time) // newest first
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&snapshots)?);
    } else {
        println!("{}", "=== Edit History ===".green().bold());
        if snapshots.is_empty() {
            println!("  No snapshots found.");
        } else {
            for snap in &snapshots {
                let id = snap.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let label = snap.get("label").and_then(|v| v.as_str()).unwrap_or("?");
                let created = snap
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let file_count = snap
                    .get("files")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);

                println!(
                    "\n  {} {} — {} ({} files)",
                    "snapshot".cyan(),
                    id.yellow(),
                    label,
                    file_count
                );
                println!("    Created: {}", created);

                if file.is_some() {
                    if let Some(files) = snap.get("files").and_then(|v| v.as_array()) {
                        for f in files {
                            let path = f
                                .get("original_path")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            println!("    → {}", path);
                        }
                    }
                }
            }
        }
        println!("\n  Total: {} snapshots", snapshots.len());
    }

    Ok(())
}
