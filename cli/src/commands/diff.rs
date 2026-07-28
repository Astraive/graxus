use anyhow::Result;
use colored::Colorize;
use graxus_core::scanner;

use crate::context::CliContext;

/// Show what changed since last index.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `json` - Output as JSON
pub fn run(ctx: &CliContext, json: bool) -> Result<()> {
    let root = ctx.resolve_root()?;
    let config = ctx.load_config(&root)?;
    let graxus_dir = root.join(".graxus");

    // Scan current files
    let current_files = scanner::scan(&root, &config)?;

    // Load previous scan
    let previous = scanner::load_saved_files(&graxus_dir);

    let diff = match &previous {
        Some(old) => scanner::compute_diff(old, &current_files),
        None => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "no_previous_index",
                        "message": "No previous index found. Run `graxus index` first.",
                        "current_files": current_files.len(),
                    })
                );
            } else {
                println!(
                    "{}",
                    "No previous index found. Run `graxus index` first.".yellow()
                );
            }
            return Ok(());
        }
    };

    let total_changes = diff.added.len() + diff.modified.len() + diff.deleted.len();

    if json {
        let result = serde_json::json!({
            "added": diff.added.iter().map(|f| &f.relative_path).collect::<Vec<_>>(),
            "modified": diff.modified.iter().map(|f| &f.relative_path).collect::<Vec<_>>(),
            "deleted": diff.deleted,
            "total_changes": total_changes,
            "current_files": current_files.len(),
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", "=== Changes Since Last Index ===".green().bold());
        println!("  Current files: {}", current_files.len());
        println!("  Changes:       {}", total_changes);
        println!();

        if diff.added.is_empty() && diff.modified.is_empty() && diff.deleted.is_empty() {
            println!("  No changes detected. Index is up to date.");
        } else {
            if !diff.added.is_empty() {
                println!("  {} New files:", "+".green().bold());
                for f in &diff.added {
                    println!("    {} {}", "+".green(), f.relative_path);
                }
                println!();
            }
            if !diff.modified.is_empty() {
                println!("  {} Modified files:", "~".yellow().bold());
                for f in &diff.modified {
                    println!("    {} {}", "~".yellow(), f.relative_path);
                }
                println!();
            }
            if !diff.deleted.is_empty() {
                println!("  {} Deleted files:", "-".red().bold());
                for f in &diff.deleted {
                    println!("    {} {}", "-".red(), f);
                }
                println!();
            }

            println!(
                "  Run {} to apply changes.",
                "graxus update".cyan().bold()
            );
        }
    }

    Ok(())
}
