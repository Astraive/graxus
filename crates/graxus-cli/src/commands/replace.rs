use anyhow::{Context, Result};
use colored::Colorize;
use graxus_core::{config::GraxusConfig, scanner, workspace};
use std::env;
use std::path::Path;

pub fn run(
    pattern: &str,
    replacement: &str,
    is_regex: bool,
    preview: bool,
    apply: bool,
) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let config = GraxusConfig::load(&root)?;

    if !apply && !preview {
        println!("{}", "Specify --preview or --apply".yellow());
        println!("  Use --preview to see what would change");
        println!("  Use --apply to make the changes");
        return Ok(());
    }

    let (docs, code, _config_files) = scanner::scan_categorized(&root, &config)?;
    let files: Vec<_> = docs.iter().chain(code.iter()).collect();

    let regex = if is_regex {
        Some(regex::Regex::new(pattern).context("Invalid regex pattern")?)
    } else {
        None
    };

    // Collect all changes
    let mut changes: Vec<(String, Vec<(usize, String, String)>)> = Vec::new();
    let mut total_replacements = 0;

    for file in &files {
        let content = match std::fs::read_to_string(&file.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut file_changes = Vec::new();
        for (line_num, line) in content.lines().enumerate() {
            let new_line = if let Some(ref re) = regex {
                re.replace_all(line, replacement).to_string()
            } else {
                line.replace(pattern, replacement)
            };

            if new_line != line {
                file_changes.push((line_num + 1, line.to_string(), new_line));
                total_replacements += 1;
            }
        }

        if !file_changes.is_empty() {
            changes.push((file.relative_path.clone(), file_changes));
        }
    }

    if changes.is_empty() {
        println!("{}", "No matches found.".yellow());
        return Ok(());
    }

    // Show preview
    println!("{}", "=== Replace Preview ===".green().bold());
    println!("  Pattern:     {}", pattern);
    println!("  Replacement: {}", replacement);
    if is_regex {
        println!("  Mode:        regex");
    }
    println!("  Files affected: {}", changes.len());
    println!("  Total replacements: {}", total_replacements);

    for (path, file_changes) in &changes {
        println!("\n  {}", path.cyan().bold());
        for (line_num, old, new) in file_changes {
            println!("    Line {}:", line_num);
            println!("      - {}", old.red());
            println!("      + {}", new.green());
        }
    }

    // Apply changes
    if apply {
        // Check max files limit
        if changes.len() > config.edit.max_files_per_operation {
            anyhow::bail!(
                "Too many files ({}) — max is {}. Use --force to override.",
                changes.len(),
                config.edit.max_files_per_operation
            );
        }

        // Create snapshot if configured
        if config.edit.create_snapshots {
            let snapshot_id = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
            let snapshot_dir = workspace::snapshots_dir(&root).join(&snapshot_id);
            std::fs::create_dir_all(&snapshot_dir)?;

            for (path, _) in &changes {
                let file_path = root.join(path);
                if file_path.exists() {
                    let backup_path = snapshot_dir.join(path.replace('/', "_"));
                    std::fs::copy(&file_path, &backup_path)?;
                }
            }
            println!("\n  Snapshot saved to .graxus/snapshots/{}", snapshot_id);
        }

        // Apply changes
        for (path, file_changes) in &changes {
            let file_path = root.join(path);
            let content = std::fs::read_to_string(&file_path)?;
            let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

            for (line_num, _, new_line) in file_changes {
                let idx = line_num - 1;
                if idx < lines.len() {
                    lines[idx] = new_line.clone();
                }
            }

            std::fs::write(&file_path, lines.join("\n"))?;
        }

        println!("\n{}", format!("Applied {} replacements across {} files", total_replacements, changes.len()).green().bold());
    }

    Ok(())
}
