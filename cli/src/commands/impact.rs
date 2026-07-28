use anyhow::Result;
use colored::Colorize;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::context::CliContext;

/// Show the blast radius (impact analysis) for a file or symbol.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `file` - File or symbol to analyze
/// * `depth` - Maximum traversal depth
/// * `direction` - Direction of analysis: `callers` (who calls the target),
///   `callees` (what the target calls), `both`, or `importers` (files that
///   import the target). Unknown values fall back to `callers`.
/// * `max_symbols` - Maximum number of symbols to return (0 = unlimited)
/// * `max_files` - Maximum number of files to return (0 = unlimited)
/// * `json` - Output as JSON
pub fn run(
    ctx: &CliContext,
    file: &str,
    depth: usize,
    direction: &str,
    max_symbols: usize,
    max_files: usize,
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

    let dir = direction.trim().to_ascii_lowercase();
    let want_callers = matches!(dir.as_str(), "" | "callers" | "both");
    let want_callees = matches!(dir.as_str(), "callees" | "both");
    let want_importers = dir == "importers";

    // call graph: callee -> [(caller_file, caller_symbol)]
    let mut callers_of: HashMap<String, Vec<(String, String)>> = HashMap::new();
    // reverse: caller_file -> [callee]
    let mut callees_of: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(calls) = codemap.get("calls").and_then(|c| c.as_array()) {
        for call in calls {
            let callee = call
                .get("callee_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let caller_file = call.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let caller_sym = call
                .get("caller_symbol")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !callee.is_empty() && !caller_file.is_empty() {
                callers_of
                    .entry(callee.to_string())
                    .or_default()
                    .push((caller_file.to_string(), caller_sym.clone()));
                callees_of
                    .entry(caller_file.to_string())
                    .or_default()
                    .push(callee.to_string());
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

    let mut visited_files: HashSet<String> = HashSet::new();
    let mut visited_symbols: HashSet<String> = HashSet::new();

    if want_importers {
        // Files that import the target.
        if let Some(imports) = codemap.get("imports").and_then(|i| i.as_array()) {
            for imp in imports {
                let source = imp.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let imp_file = imp.get("file").and_then(|v| v.as_str()).unwrap_or("");
                if source.contains(file) {
                    visited_files.insert(imp_file.to_string());
                }
            }
        }
    }

    // BFS for transitive callers.
    if want_callers {
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        let mut seen: HashSet<String> = HashSet::new();
        for sym in &target_symbols {
            queue.push_back((sym.clone(), 0));
            seen.insert(sym.clone());
        }
        visited_files.insert(file.to_string());
        while let Some((symbol, current_depth)) = queue.pop_front() {
            if current_depth >= depth {
                continue;
            }
            if let Some(callers) = callers_of.get(&symbol) {
                for (caller_file, caller_sym) in callers {
                    visited_files.insert(caller_file.clone());
                    if let Some(symbols) = codemap.get("symbols").and_then(|s| s.as_array()) {
                        for sym in symbols {
                            let sym_file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
                            let sym_name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            if sym_file == caller_file && !seen.contains(sym_name) {
                                seen.insert(sym_name.to_string());
                                visited_symbols.insert(sym_name.to_string());
                                queue.push_back((sym_name.to_string(), current_depth + 1));
                            }
                        }
                    }
                    let _ = caller_sym;
                }
            }
        }
        visited_files.remove(file);
    }

    // BFS for transitive callees (what the target's symbols call).
    if want_callees {
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        let mut seen: HashSet<String> = HashSet::new();
        for sym in &target_symbols {
            queue.push_back((sym.clone(), 0));
            seen.insert(sym.clone());
        }
        while let Some((symbol, current_depth)) = queue.pop_front() {
            if current_depth >= depth {
                continue;
            }
            if let Some(callees) = callees_of.get(&symbol) {
                for callee in callees {
                    if !seen.contains(callee) {
                        seen.insert(callee.clone());
                        visited_symbols.insert(callee.clone());
                        queue.push_back((callee.clone(), current_depth + 1));
                    }
                }
            }
            // Also resolve callee names back to the files that define them.
            if let Some(symbols) = codemap.get("symbols").and_then(|s| s.as_array()) {
                for sym in symbols {
                    let sym_name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let sym_file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
                    if sym_name == symbol {
                        visited_files.insert(sym_file.to_string());
                    }
                }
            }
        }
        visited_files.remove(file);
    }

    // Apply --max-symbols / --max-files caps (0 = unlimited), tracking whether
    // we truncated so the output can say so.
    let mut symbols_truncated = false;
    let mut files_truncated = false;
    let mut impacted_symbols: Vec<String> = visited_symbols.into_iter().collect();
    impacted_symbols.sort();
    if max_symbols > 0 && impacted_symbols.len() > max_symbols {
        impacted_symbols.truncate(max_symbols);
        symbols_truncated = true;
    }
    let mut impacted_files: Vec<String> = visited_files.into_iter().collect();
    impacted_files.sort();
    if max_files > 0 && impacted_files.len() > max_files {
        impacted_files.truncate(max_files);
        files_truncated = true;
    }

    if json {
        let output = serde_json::json!({
            "file": file,
            "depth": depth,
            "direction": dir,
            "target_symbols": target_symbols.iter().collect::<Vec<_>>(),
            "impacted_files": impacted_files,
            "impacted_symbols": impacted_symbols,
            "symbols_truncated": symbols_truncated,
            "files_truncated": files_truncated,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "{}",
            format!("=== Impact Analysis for {} ===", file)
                .green()
                .bold()
        );
        println!("  Depth: {}", depth);
        println!(
            "  Direction: {}",
            if dir.is_empty() { "callers" } else { &dir }
        );
        println!(
            "  Symbols defined: {}",
            target_symbols
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!();
        let file_label = if files_truncated {
            format!(
                "Impacted files ({} shown, capped at --max-files={}):",
                impacted_files.len(),
                max_files
            )
        } else {
            format!("Impacted files ({}):", impacted_files.len())
        };
        println!("{}", file_label.cyan().bold());
        for f in &impacted_files {
            println!("  → {}", f);
        }
        println!();
        let sym_label = if symbols_truncated {
            format!(
                "Impacted symbols ({} shown, capped at --max-symbols={}):",
                impacted_symbols.len(),
                max_symbols
            )
        } else {
            format!("Impacted symbols ({}):", impacted_symbols.len())
        };
        println!("{}", sym_label.cyan().bold());
        for s in &impacted_symbols {
            println!("  → {}", s);
        }
    }

    Ok(())
}
