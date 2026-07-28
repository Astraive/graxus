use anyhow::{Context, Result};
use colored::Colorize;
use graxus_core::scanner;
use graxus_index::IndexStore;

use crate::context::CliContext;
use crate::filters::{apply_filters, build_glob_set};

/// Replace text across the project with preview and apply modes.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `pattern` - Text pattern to find
/// * `replacement` - Replacement text
/// * `is_regex` - Treat pattern as a regular expression
/// * `preview` - Show what would change without applying
/// * `apply` - Actually apply the changes
/// * `include` - Include glob patterns for file filtering
/// * `exclude` - Exclude glob patterns for file filtering
/// * `lang` - Filter by programming language
/// * `max_files` - Maximum files to modify (0 = no explicit cap; falls back to config)
/// * `max_replacements` - Maximum number of replacements (0 = unlimited)
#[allow(clippy::too_many_arguments)]
pub fn run(
    ctx: &CliContext,
    pattern: &str,
    replacement: &str,
    is_regex: bool,
    preview: bool,
    apply: bool,
    include: Vec<String>,
    exclude: Vec<String>,
    lang: Vec<String>,
    max_files: usize,
    max_replacements: usize,
) -> Result<()> {
    let root = ctx.resolve_root()?;
    let config = ctx.load_config(&root)?;

    if !apply && !preview {
        println!("{}", "Specify --preview or --apply".yellow());
        println!("  Use --preview to see what would change");
        println!("  Use --apply to make the changes");
        return Ok(());
    }

    // Apply user-supplied --include / --exclude / --lang filters.
    let include_set = build_glob_set(&include)?;
    let exclude_set = build_glob_set(&exclude)?;

    let (mut docs, mut code, _config_files) = scanner::scan_categorized(&root, &config)?;
    apply_filters(&mut docs, &include_set, &exclude_set, &lang);
    apply_filters(&mut code, &include_set, &exclude_set, &lang);
    let files: Vec<_> = docs.iter().chain(code.iter()).collect();

    let regex = if is_regex {
        Some(regex::Regex::new(pattern).context("Invalid regex pattern")?)
    } else {
        None
    };

    // Collect all changes, honoring the --max-replacements cap (0 = unlimited).
    #[allow(clippy::type_complexity)]
    let mut changes: Vec<(String, Vec<(usize, String, String)>)> = Vec::new();
    let mut total_replacements = 0usize;
    let mut hit_replacement_cap = false;

    'outer: for file in &files {
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
                if max_replacements > 0 && total_replacements >= max_replacements {
                    hit_replacement_cap = true;
                    break 'outer;
                }
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
    if !include.is_empty() {
        println!("  Include:     {}", include.join(", "));
    }
    if !exclude.is_empty() {
        println!("  Exclude:     {}", exclude.join(", "));
    }
    if !lang.is_empty() {
        println!("  Languages:   {}", lang.join(", "));
    }
    println!("  Files affected: {}", changes.len());
    println!("  Total replacements: {}", total_replacements);
    if hit_replacement_cap {
        println!(
            "  {}",
            format!("Stopped at --max-replacements={}", max_replacements).yellow()
        );
    }

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
        // File-count safety cap. Explicit --max-files (when > 0) overrides the
        // config default; either way, exceeding it aborts before any mutation.
        let file_cap = if max_files > 0 {
            max_files
        } else {
            config.edit.max_files_per_operation
        };
        if changes.len() > file_cap {
            anyhow::bail!(
                "Too many files ({}) — max is {}. Narrow the filters or raise --max-files.",
                changes.len(),
                file_cap
            );
        }

        // Create snapshot if configured. We use the shared IndexStore snapshot
        // system (UUID id + meta.json) so the result is visible to `graxus
        // history` and restorable via `graxus rollback <id>`, matching the
        // contract documented in docs/SAFETY.md.
        let snapshot = if config.edit.create_snapshots {
            // Collect absolute paths of files that will be modified, so the
            // snapshot captures their pre-edit state.
            let targets: Vec<std::path::PathBuf> =
                changes.iter().map(|(rel, _)| root.join(rel)).collect();

            let store = IndexStore::new(root.join(".graxus"));
            match store.create_snapshot("replace", &targets) {
                Ok(s) => {
                    println!(
                        "\n  Snapshot saved (id {}). Roll back with: graxus rollback {} --apply",
                        s.id, s.id
                    );
                    Some(s)
                }
                Err(e) => {
                    // Snapshot failure is a safety red flag — refuse to mutate.
                    anyhow::bail!(
                        "Failed to create pre-edit snapshot: {}. No files were modified.",
                        e
                    );
                }
            }
        } else {
            None
        };
        // Suppress unused warning when snapshots are disabled.
        let _ = &snapshot;

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

        println!(
            "\n{}",
            format!(
                "Applied {} replacements across {} files",
                total_replacements,
                changes.len()
            )
            .green()
            .bold()
        );
    }

    Ok(())
}
