use anyhow::{Context, Result};
use colored::Colorize;
use graxus_core::{config::GraxusConfig, scanner, workspace};
use std::env;
use std::path::Path;

pub fn run(query: &str, docs_only: bool, code_only: bool, symbol: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let config = GraxusConfig::load(&root)?;
    let (docs, code, _config_files) = scanner::scan_categorized(&root, &config)?;

    let files: Vec<_> = if docs_only {
        docs.iter().collect()
    } else if code_only {
        code.iter().collect()
    } else {
        docs.iter().chain(code.iter()).collect()
    };

    let query_lower = query.to_lowercase();
    let mut matches = 0;

    for file in &files {
        let content = match std::fs::read_to_string(&file.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut file_matches = Vec::new();
        for (line_num, line) in content.lines().enumerate() {
            let matches_line = if symbol {
                // Symbol search: match whole word
                line.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|word| word == query)
            } else {
                line.to_lowercase().contains(&query_lower)
            };

            if matches_line {
                file_matches.push((line_num + 1, line));
            }
        }

        if !file_matches.is_empty() {
            println!(
                "\n{} {}",
                file.relative_path.cyan().bold(),
                format!("({} matches)", file_matches.len()).dimmed()
            );
            for (line_num, line) in &file_matches {
                // Highlight the match in the line
                let highlighted = if symbol {
                    line.replace(query, &query.red().bold().to_string())
                } else {
                    // Case-insensitive highlight
                    let lower = line.to_lowercase();
                    let mut result = String::new();
                    let mut last = 0;
                    let query_len = query.len();
                    let mut search_from = 0;
                    while let Some(pos) = lower[search_from..].find(&query_lower) {
                        let abs_pos = search_from + pos;
                        result.push_str(&line[last..abs_pos]);
                        result.push_str(&line[abs_pos..abs_pos + query_len].red().bold().to_string());
                        last = abs_pos + query_len;
                        search_from = abs_pos + query_len;
                    }
                    result.push_str(&line[last..]);
                    result
                };
                println!("  {:>4}: {}", line_num.to_string().dimmed(), highlighted);
            }
            matches += file_matches.len();
        }
    }

    println!(
        "\n{}",
        format!("Found {} matches across files", matches).green()
    );
    Ok(())
}
