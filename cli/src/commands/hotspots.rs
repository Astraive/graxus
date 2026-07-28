use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;

use crate::context::CliContext;

/// Show the most-called symbols (hotspots) in the codebase.
///
/// # Arguments
/// * `limit` - Maximum number of results to return
/// * `_min_usage` - Minimum usage count to include
/// * `_exclude_tests` - If true, exclude test symbols
/// * `json` - Output as JSON
pub fn run(
    ctx: &CliContext,
    limit: usize,
    _min_usage: usize,
    _exclude_tests: bool,
    json: bool,
) -> Result<()> {
    let root = ctx.resolve_root()?;

    let codemap_path = root.join(".graxus").join("code").join("codemap.json");
    if !codemap_path.exists() {
        println!("{}", "No codemap found. Run `graxus index` first.".yellow());
        return Ok(());
    }

    let content = std::fs::read_to_string(&codemap_path)?;
    let codemap: serde_json::Value = serde_json::from_str(&content)?;

    // Count how many times each symbol is called
    let mut call_counts: HashMap<String, usize> = HashMap::new();
    if let Some(calls) = codemap.get("calls").and_then(|c| c.as_array()) {
        for call in calls {
            let callee = call
                .get("callee_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !callee.is_empty() {
                *call_counts.entry(callee.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Collect symbol info with call counts
    let mut hotspots: Vec<(String, String, String, usize)> = Vec::new();
    if let Some(symbols) = codemap.get("symbols").and_then(|s| s.as_array()) {
        for sym in symbols {
            let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let sym_file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let count = call_counts.get(name).copied().unwrap_or(0);
            if count > 0 {
                hotspots.push((
                    name.to_string(),
                    kind.to_string(),
                    sym_file.to_string(),
                    count,
                ));
            }
        }
    }

    hotspots.sort_by_key(|b| std::cmp::Reverse(b.3));
    hotspots.truncate(limit);

    if json {
        let items: Vec<serde_json::Value> = hotspots
            .iter()
            .map(|(name, kind, file, count)| {
                serde_json::json!({
                    "name": name,
                    "kind": kind,
                    "file": file,
                    "call_count": count,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        println!(
            "{}",
            "=== Hotspots (Most-Called Symbols) ===".green().bold()
        );
        if hotspots.is_empty() {
            println!("  No call data found.");
        } else {
            println!("  {:<40} {:<12} {:<50}", "Symbol", "Kind", "File");
            println!("  {}", "-".repeat(120));
            for (name, kind, file, count) in &hotspots {
                let count_str = if *count > 10 {
                    count.to_string().red().to_string()
                } else {
                    count.to_string()
                };
                println!("  {:<40} {:<12} {:<50} {}", name, kind, file, count_str);
            }
        }
        println!("\n  Total: {} hotspots", hotspots.len());
    }

    Ok(())
}
