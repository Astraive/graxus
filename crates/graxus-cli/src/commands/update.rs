use anyhow::{Context, Result};
use colored::Colorize;
use graxus_core::{config::GraxusConfig, scanner, workspace};
use std::env;
use std::path::Path;

use crate::commands::index;

pub fn run(dry_run: bool, full: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let config = GraxusConfig::load(&root)?;
    let graxus_dir = root.join(".graxus");

    println!("{}", "=== Graxus Update ===".green().bold());

    // Scan current files
    let current_files = scanner::scan(&root, &config)?;

    if full {
        println!("  Full re-index requested.");
        if dry_run {
            println!("  Would re-index {} files.", current_files.len());
        } else {
            index::run()?;
            scanner::save_saved_files(&graxus_dir, &current_files)?;
        }
        return Ok(());
    }

    // Load previous scan
    let previous = scanner::load_saved_files(&graxus_dir);

    let diff = match &previous {
        Some(old) => scanner::compute_diff(old, &current_files),
        None => {
            println!("  No previous index found. Running full index.");
            if dry_run {
                println!("  Would index {} files.", current_files.len());
            } else {
                index::run()?;
                scanner::save_saved_files(&graxus_dir, &current_files)?;
            }
            return Ok(());
        }
    };

    let total_changes = diff.added.len() + diff.modified.len() + diff.deleted.len();

    if total_changes == 0 {
        println!("  Everything up to date. No changes detected.");
        return Ok(());
    }

    println!("  Changes detected:");
    if !diff.added.is_empty() {
        println!("    {} {} new files", "+".green(), diff.added.len());
    }
    if !diff.modified.is_empty() {
        println!("    {} {} modified files", "~".yellow(), diff.modified.len());
    }
    if !diff.deleted.is_empty() {
        println!("    {} {} deleted files", "-".red(), diff.deleted.len());
    }

    if dry_run {
        println!("\n{}", "Changes that would be applied:".cyan().bold());
        for f in &diff.added {
            println!("    + {}", f.relative_path);
        }
        for f in &diff.modified {
            println!("    ~ {}", f.relative_path);
        }
        for f in &diff.deleted {
            println!("    - {}", f);
        }
        println!("\n  Run without --dry-run to apply.");
        return Ok(());
    }

    // Run full re-index (for now — incremental graph updates are complex)
    // The key value is: we KNOW what changed and can report it
    println!("\n  Re-indexing...");
    index::run()?;

    // Save new file list
    scanner::save_saved_files(&graxus_dir, &current_files)?;

    println!("\n{}", "Update complete.".green().bold());
    println!("  {} files scanned", current_files.len());
    println!("  {} changes applied", total_changes);

    Ok(())
}
