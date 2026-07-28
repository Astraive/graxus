use anyhow::Result;
use colored::Colorize;

use crate::context::CliContext;

/// Search the project for literal text matches.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `query` - Search query string
/// * `docs_only` - Search only documentation files
/// * `code_only` - Search only code files
/// * `symbol` - Search for whole-symbol matches
/// * `max_results` - Maximum number of result lines to return
/// * `context_lines` - Number of context lines around each match
/// * `case_sensitive` - If true, perform case-sensitive search
pub fn run(
    ctx: &CliContext,
    query: &str,
    docs_only: bool,
    code_only: bool,
    symbol: bool,
    max_results: usize,
    context_lines: usize,
    case_sensitive: bool,
) -> Result<()> {
    let root = ctx.resolve_root()?;
    let config = ctx.load_config(&root)?;
    let (docs, code, _config_files) = graxus_core::scanner::scan_categorized(&root, &config)?;

    let files: Vec<_> = if docs_only {
        docs.iter().collect()
    } else if code_only {
        code.iter().collect()
    } else {
        docs.iter().chain(code.iter()).collect()
    };

    // Case-fold the query once when running case-insensitively.
    let needle = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    let mut matches = 0usize;
    let mut truncated = false;

    'outer: for file in &files {
        let content = match std::fs::read_to_string(&file.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut file_matches: Vec<(usize, String)> = Vec::new();

        for (idx, line) in lines.iter().enumerate() {
            let hit = if symbol {
                // Symbol search: match whole word (always case-sensitive).
                line.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|word| word == query)
            } else if case_sensitive {
                line.contains(&needle)
            } else {
                line.to_lowercase().contains(&needle)
            };

            if hit {
                file_matches.push((idx + 1, line.to_string()));
            }
        }

        if !file_matches.is_empty() {
            if !ctx.quiet {
                println!(
                    "\n{} {}",
                    file.relative_path.cyan().bold(),
                    format!("({} matches)", file_matches.len()).dimmed()
                );
            }

            for (line_num, line) in &file_matches {
                if matches >= max_results {
                    truncated = true;
                    break 'outer;
                }

                // Print context window around the match when requested.
                if context_lines > 0 && !ctx.quiet {
                    let center = line_num - 1; // 0-based
                    let start = center.saturating_sub(context_lines);
                    let end = (center + context_lines + 1).min(lines.len());
                    for i in start..end {
                        let prefix = if i == center { ">" } else { " " };
                        let n = i + 1;
                        println!(
                            "  {} {:>4}: {}",
                            prefix.dimmed(),
                            n.to_string().dimmed(),
                            lines[i]
                        );
                    }
                    matches += 1;
                    continue;
                }

                if ctx.quiet {
                    // Quiet mode: terse `path:line:match` output, no color.
                    println!("{}:{}:{}", file.relative_path, line_num, line);
                } else {
                    let highlighted = highlight(line, query, &needle, symbol, case_sensitive);
                    println!("  {:>4}: {}", line_num.to_string().dimmed(), highlighted);
                }
                matches += 1;
            }
        }
    }

    if !ctx.quiet {
        let summary = if truncated {
            format!(
                "Found {} matches (truncated at --max-results={})",
                matches, max_results
            )
        } else {
            format!("Found {} matches across files", matches)
        };
        println!("\n{}", summary.green());
    }

    Ok(())
}

/// Highlight the query within a line, honoring the match mode.
fn highlight(line: &str, query: &str, needle: &str, symbol: bool, case_sensitive: bool) -> String {
    if symbol {
        return line.replace(query, &query.red().bold().to_string());
    }
    if case_sensitive {
        return line.replace(needle, &needle.red().bold().to_string());
    }
    // Case-insensitive highlight: walk byte offsets preserving original casing.
    let mut result = String::new();
    let mut last = 0;
    let query_len = needle.len();
    let lower = line.to_lowercase();
    let mut search_from = 0;
    while let Some(pos) = lower[search_from..].find(needle) {
        let abs_pos = search_from + pos;
        result.push_str(&line[last..abs_pos]);
        result.push_str(&line[abs_pos..abs_pos + query_len].red().bold().to_string());
        last = abs_pos + query_len;
        search_from = abs_pos + query_len;
    }
    result.push_str(&line[last..]);
    result
}
