use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::env;
use std::path::Path;

use graxus_core::workspace;

pub fn run(json: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let codemap_path = root.join(".graxus").join("code").join("codemap.json");
    if !codemap_path.exists() {
        println!("{}", "No codemap found. Run `graxus index` first.".yellow());
        return Ok(());
    }

    let content = std::fs::read_to_string(&codemap_path)?;
    let codemap: serde_json::Value = serde_json::from_str(&content)?;

    // Count calls to each symbol
    let mut call_counts: HashMap<String, usize> = HashMap::new();
    if let Some(calls) = codemap.get("calls").and_then(|c| c.as_array()) {
        for call in calls {
            let callee = call.get("callee_text").and_then(|v| v.as_str()).unwrap_or("");
            if !callee.is_empty() {
                *call_counts.entry(callee.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Find symbols with zero calls (excluding test functions and main)
    let mut dead: Vec<(String, String, String, usize)> = Vec::new();
    if let Some(symbols) = codemap.get("symbols").and_then(|s| s.as_array()) {
        for sym in symbols {
            let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let sym_file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let line = sym.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0);

            // Skip test functions, main, and lib/module entry points
            if name == "main" || name.starts_with("test_") || name.starts_with("Test") {
                continue;
            }
            // Skip items in test files
            if sym_file.contains("/test") || sym_file.contains("_test.") || sym_file.contains("test_") {
                continue;
            }

            let count = call_counts.get(name).copied().unwrap_or(0);
            if count == 0 {
                dead.push((
                    name.to_string(),
                    kind.to_string(),
                    sym_file.to_string(),
                    line as usize,
                ));
            }
        }
    }

    dead.sort_by(|a, b| a.2.cmp(&b.2).then(a.3.cmp(&b.3)));

    if json {
        let items: Vec<serde_json::Value> = dead
            .iter()
            .map(|(name, kind, file, line)| {
                serde_json::json!({
                    "name": name,
                    "kind": kind,
                    "file": file,
                    "line": line,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        println!("{}", "=== Potentially Dead Code ===".green().bold());
        if dead.is_empty() {
            println!("  No uncalled symbols found.");
        } else {
            for (name, kind, file, line) in &dead {
                println!(
                    "  {} {} {} {}:{}",
                    "⚠".yellow(),
                    kind,
                    name.cyan(),
                    file,
                    line
                );
            }
        }
        println!("\n  Total: {} potentially unused symbols", dead.len());
        println!(
            "  {} Note: This is a heuristic. Some symbols may be used via reflection, macros, or dynamic dispatch.",
            "⚠".yellow()
        );
    }

    Ok(())
}
