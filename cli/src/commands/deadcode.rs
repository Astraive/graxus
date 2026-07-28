use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;

use crate::context::CliContext;

/// Show potentially dead code (uncalled symbols).
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `min_confidence` - Minimum confidence (0-100) that a symbol is truly dead.
///                      Higher = stricter (only private uncalled symbols).
/// * `limit` - Maximum number of results to return
/// * `include_exported` - If true, treat exported symbols as dead-code candidates
///                        (off by default: exported symbols are assumed to be a
///                        public API surface and excluded).
/// * `exclude_tests` - If true (default), exclude test symbols and test files
/// * `json` - Output as JSON
pub fn run(
    ctx: &CliContext,
    min_confidence: f64,
    limit: usize,
    include_exported: bool,
    exclude_tests: bool,
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

    // ── Build the set of "used" symbol names from two signals ─────────────
    // Signal 1: the call graph. A symbol is used if it appears as a callee
    // (raw text) or is the target of a resolved call/path. We record the
    // trailing path segment so `lib::Helper::new` credits both `new` and the
    // path-prefix type usages we can infer.
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(calls) = codemap.get("calls").and_then(|c| c.as_array()) {
        for call in calls {
            if let Some(callee) = call.get("callee_text").and_then(|v| v.as_str()) {
                if !callee.is_empty() {
                    // Credit the full callee text and its trailing segment.
                    used.insert(callee.to_string());
                    if let Some(tail) = callee.rsplit("::").next() {
                        used.insert(tail.to_string());
                    }
                }
            }
            // Resolved target: "file::name" — credit the name segment.
            if let Some(resolved) = call.get("resolved_symbol").and_then(|v| v.as_str()) {
                if let Some(tail) = resolved.rsplit("::").next() {
                    used.insert(tail.to_string());
                }
            }
        }
    }

    // Signal 2: textual references across all indexed files. This catches
    // non-call usages the call graph misses: type annotations, struct
    // construction (`Foo { ... }` / `Foo::new()`), path prefixes, trait impls,
    // macro invocations, etc. We scan each file's source once and record each
    // token together with its (file, line) location, so a symbol's own
    // definition site is NOT counted as a reference to itself.
    let files_json_path = root.join(".graxus").join("files.json");
    // token -> list of (file, line) where it appears
    let mut referenced: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    if files_json_path.exists() {
        if let Ok(files_raw) = std::fs::read_to_string(&files_json_path) {
            if let Ok(files) = serde_json::from_str::<Vec<serde_json::Value>>(&files_raw) {
                for file in &files {
                    let path_str = file.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    let rel = file
                        .get("relative_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(path_str);
                    if path_str.is_empty() {
                        continue;
                    }
                    if let Ok(text) = std::fs::read_to_string(path_str) {
                        for (idx, line) in text.lines().enumerate() {
                            let lineno = idx + 1;
                            for tok in
                                line.split(|c: char| !c.is_alphanumeric() && c != '_')
                            {
                                if !tok.is_empty() {
                                    referenced
                                        .entry(tok.to_string())
                                        .or_default()
                                        .push((rel.to_string(), lineno));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// True if `name` is referenced outside its own definition line.
    fn referenced_elsewhere(
        referenced: &HashMap<String, Vec<(String, usize)>>,
        name: &str,
        sym_file: &str,
        sym_line: usize,
    ) -> bool {
        referenced
            .get(name)
            .is_some_and(|locs| {
                locs.iter()
                    .any(|(f, l)| f != sym_file || *l != sym_line)
            })
    }

    // Find symbols with zero calls. Each candidate carries a confidence score:
    //   - 90 if private + not a test + not main/lib entry
    //   - 50 if exported (lower confidence — could be public API)
    //   - 30 if it looks like an entry point (main/lib) that we keep only as a hint
    // `--min-confidence` filters these; `--include-exported` admits the 50-tier.
    let mut dead: Vec<(String, String, String, usize, f64)> = Vec::new();
    if let Some(symbols) = codemap.get("symbols").and_then(|s| s.as_array()) {
        for sym in symbols {
            let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let sym_file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let line = sym.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0);
            let is_exported = sym
                .get("exported")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let is_test_sym = sym
                .get("is_test")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Test exclusion: explicit flag, or heuristic test names/files.
            if exclude_tests {
                if is_test_sym
                    || name.starts_with("test_")
                    || name.starts_with("Test")
                {
                    continue;
                }
                if sym_file.contains("/test")
                    || sym_file.contains("_test.")
                    || sym_file.contains("test_")
                {
                    continue;
                }
            }

            // Entry points are almost never "dead" even when uncalled.
            if name == "main" || name == "lib" {
                continue;
            }

            // A symbol is "used" if it is reached by the call graph OR appears
            // as a textual reference (outside its own definition line) in the
            // indexed sources. The latter catches type/value usages that don't
            // surface as calls, e.g. a struct used as a type annotation.
            let called = used.contains(name);
            let referenced_anywhere =
                referenced_elsewhere(&referenced, name, sym_file, line as usize);
            if called || referenced_anywhere {
                continue;
            }

            // Confidence tier for this candidate.
            //   - 90 for private, uncalled AND unreferenced symbols (strongest
            //     dead-code signal)
            //   - 75 when the user explicitly opted into --include-exported
            //     (still admits them past the default --min-confidence of 70)
            let confidence = if !is_exported {
                90.0
            } else if include_exported {
                75.0
            } else {
                // Exported and not asked to include — skip (assumed public API).
                continue;
            };

            if confidence + 1e-9 < min_confidence {
                continue;
            }

            dead.push((
                name.to_string(),
                kind.to_string(),
                sym_file.to_string(),
                line as usize,
                confidence,
            ));
        }
    }

    dead.sort_by(|a, b| a.2.cmp(&b.2).then(a.3.cmp(&b.3)));
    let total_before_limit = dead.len();
    if limit > 0 {
        dead.truncate(limit);
    }

    if json {
        let items: Vec<serde_json::Value> = dead
            .iter()
            .map(|(name, kind, file, line, confidence)| {
                serde_json::json!({
                    "name": name,
                    "kind": kind,
                    "file": file,
                    "line": line,
                    "confidence": confidence,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        println!("{}", "=== Potentially Dead Code ===".green().bold());
        if dead.is_empty() {
            println!("  No uncalled symbols found.");
        } else {
            for (name, kind, file, line, confidence) in &dead {
                println!(
                    "  {} [{}%] {} {} {}:{}",
                    "⚠".yellow(),
                    *confidence as u64,
                    kind,
                    name.cyan(),
                    file,
                    line
                );
            }
        }
        println!("\n  Showing {} of {} potentially unused symbols", dead.len(), total_before_limit);
        println!(
            "  {} Note: This is a heuristic. Some symbols may be used via reflection, macros, or dynamic dispatch.",
            "⚠".yellow()
        );
    }

    Ok(())
}
