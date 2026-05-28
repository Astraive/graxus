use anyhow::{Context, Result};
use colored::Colorize;
use graxus_core::{config::GraxusConfig, scanner, workspace};
use std::env;
use std::path::Path;

pub fn run(pattern: &str, docs_only: bool, code_only: bool, max_results: usize) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let re = regex::Regex::new(pattern)
        .context("Invalid regex pattern")?;

    let config = GraxusConfig::load(&root)?;
    let (docs, code, _) = scanner::scan_categorized(&root, &config)?;

    let files: Vec<_> = if docs_only {
        docs.iter().collect()
    } else if code_only {
        code.iter().collect()
    } else {
        docs.iter().chain(code.iter()).collect()
    };

    let mut total_matches = 0;

    println!("{}", format!("=== Regex Search: /{}/ ===", pattern).green().bold());

    for file in &files {
        let content = match std::fs::read_to_string(&file.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut file_matches = Vec::new();
        for (line_num, line) in content.lines().enumerate() {
            if re.is_match(line) {
                file_matches.push((line_num + 1, line));
            }
        }

        if !file_matches.is_empty() {
            println!("\n  {} ({} matches)", file.relative_path.cyan(), file_matches.len());
            for (line_num, line) in &file_matches {
                let highlighted = re.replace_all(line, |caps: &regex::Captures| {
                    caps[0].red().bold().to_string()
                });
                println!("    {:>4}: {}", line_num.to_string().dimmed(), highlighted);
            }
            total_matches += file_matches.len();
        }

        if total_matches >= max_results {
            println!("\n  {} Reached max results limit ({}). Use --max-results to increase.", "Stopped:".yellow(), max_results);
            break;
        }
    }

    println!("\n{}", format!("Found {} regex matches", total_matches).green());
    Ok(())
}
