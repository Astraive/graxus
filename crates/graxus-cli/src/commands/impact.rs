use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::path::Path;

use graxus_core::workspace;

pub fn run(file: &str, depth: usize, json: bool) -> Result<()> {
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

    // Build call graph: callee -> set of callers
    let mut callers_of: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(calls) = codemap.get("calls").and_then(|c| c.as_array()) {
        for call in calls {
            let callee = call.get("callee_text").and_then(|v| v.as_str()).unwrap_or("");
            let caller_file = call.get("file").and_then(|v| v.as_str()).unwrap_or("");
            if !callee.is_empty() && !caller_file.is_empty() {
                callers_of
                    .entry(callee.to_string())
                    .or_default()
                    .push(caller_file.to_string());
            }
        }
    }

    // Find symbols defined in the target file
    let mut target_symbols: HashSet<String> = HashSet::new();
    if let Some(symbols) = codemap.get("symbols").and_then(|s| s.as_array()) {
        for sym in symbols {
            let sym_file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let sym_name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if sym_file.contains(file) {
                target_symbols.insert(sym_name.to_string());
            }
        }
    }

    // BFS to find transitive callers
    let mut visited_files: HashSet<String> = HashSet::new();
    let mut visited_symbols: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    for sym in &target_symbols {
        queue.push_back((sym.clone(), 0));
        visited_symbols.insert(sym.clone());
    }

    // Also add the file itself
    visited_files.insert(file.to_string());

    while let Some((symbol, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            continue;
        }
        if let Some(callers) = callers_of.get(&symbol) {
            for caller in callers {
                visited_files.insert(caller.clone());
                // Find symbols defined in the caller file that call our symbol
                if let Some(symbols) = codemap.get("symbols").and_then(|s| s.as_array()) {
                    for sym in symbols {
                        let sym_file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
                        let sym_name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        if sym_file == caller && !visited_symbols.contains(sym_name) {
                            visited_symbols.insert(sym_name.to_string());
                            queue.push_back((sym_name.to_string(), current_depth + 1));
                        }
                    }
                }
            }
        }
    }

    // Remove the original file from impacted set
    visited_files.remove(file);

    if json {
        let output = serde_json::json!({
            "file": file,
            "depth": depth,
            "target_symbols": target_symbols.iter().collect::<Vec<_>>(),
            "impacted_files": visited_files.iter().collect::<Vec<_>>(),
            "impacted_symbols": visited_symbols.iter().collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}", format!("=== Impact Analysis for {} ===", file).green().bold());
        println!("  Depth: {}", depth);
        println!(
            "  Symbols defined: {}",
            target_symbols.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        );
        println!();
        println!(
            "{}",
            format!("Impacted files ({}):", visited_files.len())
                .cyan()
                .bold()
        );
        let mut sorted_files: Vec<_> = visited_files.iter().collect();
        sorted_files.sort();
        for f in sorted_files {
            println!("  → {}", f);
        }
        println!();
        println!(
            "{}",
            format!("Impacted symbols ({}):", visited_symbols.len())
                .cyan()
                .bold()
        );
        let mut sorted_symbols: Vec<_> = visited_symbols.iter().collect();
        sorted_symbols.sort();
        for s in sorted_symbols {
            println!("  → {}", s);
        }
    }

    Ok(())
}
