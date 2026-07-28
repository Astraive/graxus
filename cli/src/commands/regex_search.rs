use anyhow::{Context, Result};
use colored::Colorize;
use graxus_core::scanner;

use crate::context::CliContext;

/// Search the project with a regex pattern.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `pattern` - Regex pattern to search for
/// * `docs_only` - Search only documentation files
/// * `code_only` - Search only code files
/// * `max_results` - Maximum number of results to return
/// * `context_lines` - Number of context lines printed around each match
pub fn run(
    ctx: &CliContext,
    pattern: &str,
    docs_only: bool,
    code_only: bool,
    max_results: usize,
    context_lines: usize,
) -> Result<()> {
    let root = ctx.resolve_root()?;

    let re = regex::Regex::new(pattern).context("Invalid regex pattern")?;

    let config = ctx.load_config(&root)?;
    let (docs, code, _) = scanner::scan_categorized(&root, &config)?;

    let files: Vec<_> = if docs_only {
        docs.iter().collect()
    } else if code_only {
        code.iter().collect()
    } else {
        docs.iter().chain(code.iter()).collect()
    };

    let mut total_matches = 0;

    println!(
        "{}",
        format!("=== Regex Search: /{}/ ===", pattern)
            .green()
            .bold()
    );

    'outer: for file in &files {
        let content = match std::fs::read_to_string(&file.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut file_matches: Vec<(usize, &str)> = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                file_matches.push((idx + 1, line));
            }
        }

        if !file_matches.is_empty() {
            println!(
                "\n  {} ({} matches)",
                file.relative_path.cyan(),
                file_matches.len()
            );
            for (line_num, line) in &file_matches {
                let highlighted = re.replace_all(line, |caps: &regex::Captures| {
                    caps[0].red().bold().to_string()
                });

                if context_lines > 0 {
                    let center = line_num - 1; // 0-based
                    let start = center.saturating_sub(context_lines);
                    let end = (center + context_lines + 1).min(lines.len());
                    for (i, context_line) in lines.iter().enumerate().take(end).skip(start) {
                        let prefix = if i == center { ">" } else { " " };
                        let n = i + 1;
                        if i == center {
                            println!(
                                "    {} {:>4}: {}",
                                prefix.dimmed(),
                                n.to_string().dimmed(),
                                highlighted
                            );
                        } else {
                            println!(
                                "    {} {:>4}: {}",
                                prefix.dimmed(),
                                n.to_string().dimmed(),
                                context_line
                            );
                        }
                    }
                } else {
                    println!("    {:>4}: {}", line_num.to_string().dimmed(), highlighted);
                }

                total_matches += 1;
                if total_matches >= max_results {
                    println!(
                        "\n  {} Reached max results limit ({}). Use --max-results to increase.",
                        "Stopped:".yellow(),
                        max_results
                    );
                    break 'outer;
                }
            }
        }
    }

    println!(
        "\n{}",
        format!("Found {} regex matches", total_matches).green()
    );
    Ok(())
}
