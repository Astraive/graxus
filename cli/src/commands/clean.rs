use anyhow::{Context, Result};
use colored::Colorize;

use crate::context::CliContext;

/// Remove the `.graxus/` directory and all index data.
///
/// # Arguments
/// * `force` - Skip confirmation prompt
pub fn run(ctx: &CliContext, force: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

    let graxus_dir = root.join(".graxus");
    if !graxus_dir.exists() {
        println!(
            "{}",
            "No .graxus/ directory found. Nothing to clean.".yellow()
        );
        return Ok(());
    }

    // Count what will be removed
    let mut file_count = 0;
    let mut dir_count = 0;
    if let Ok(entries) = std::fs::read_dir(&graxus_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                dir_count += 1;
            } else {
                file_count += 1;
            }
        }
    }

    println!("{}", "=== Clean ===".green().bold());
    println!("  Directory: {}", graxus_dir.display());
    println!("  Files:     {}", file_count);
    println!("  Subdirs:   {}", dir_count);

    if !force {
        print!("\n  {} Remove all index data? [y/N] ", "?".yellow());
        use std::io::{self, Write};
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("  Cancelled.");
            return Ok(());
        }
    }

    std::fs::remove_dir_all(&graxus_dir)
        .with_context(|| format!("Failed to remove {}", graxus_dir.display()))?;

    println!("\n  {} Cleaned successfully.", "Done.".green().bold());
    println!("  Run `graxus index` to rebuild.");

    Ok(())
}
