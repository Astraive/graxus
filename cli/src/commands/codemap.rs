use anyhow::Result;
use colored::Colorize;

use graxus_core::workspace;

use crate::context::CliContext;

/// Show the code codemap overview (files, symbols, imports, calls).
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `json` - Output as JSON
pub fn run(ctx: &CliContext, json: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

    let codemap_path = workspace::code_dir(&root).join("codemap.json");

    if !codemap_path.exists() {
        println!(
            "{}",
            "Codemap not found. Run `graxus index` first.".yellow()
        );
        return Ok(());
    }

    let content = std::fs::read_to_string(&codemap_path)?;
    let codemap: serde_json::Value = serde_json::from_str(&content)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&codemap)?);
        return Ok(());
    }

    println!("{}", "=== Code Codemap ===".green().bold());
    if let Some(files) = codemap.get("files").and_then(|f| f.as_array()) {
        println!("  Files analyzed: {}", files.len());
    }
    if let Some(symbols) = codemap.get("symbols").and_then(|f| f.as_array()) {
        println!("  Symbols: {}", symbols.len());
    }
    if let Some(imports) = codemap.get("imports").and_then(|f| f.as_array()) {
        println!("  Imports: {}", imports.len());
    }
    if let Some(calls) = codemap.get("calls").and_then(|f| f.as_array()) {
        println!("  Calls: {}", calls.len());
    }
    if let Some(results) = codemap.get("parser_results").and_then(|v| v.as_array()) {
        let ripex = results
            .iter()
            .filter(|result| result.get("used_backend").and_then(|v| v.as_str()) == Some("ripex"))
            .count();
        let tree_sitter = results.len().saturating_sub(ripex);
        let fallbacks = results
            .iter()
            .filter(|result| {
                result
                    .get("fallback_reason")
                    .is_some_and(|reason| !reason.is_null())
            })
            .count();
        println!("  Parser backends: ripex={ripex}, tree-sitter={tree_sitter}");
        if fallbacks > 0 {
            println!("  Ripex fallbacks: {fallbacks}");
        }
    }

    if let Some(files) = codemap.get("files").and_then(|f| f.as_array()) {
        println!("\n{}", "Files:".cyan().bold());
        for file in files {
            let path = file.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let lang = file.get("language").and_then(|v| v.as_str()).unwrap_or("?");
            println!("  {} ({})", path, lang);
        }
    }

    Ok(())
}

/// Filter options for symbol queries.
pub struct SymbolFilter {
    pub file: Option<String>,
    pub kind: Option<String>,
    pub lang: Option<String>,
    pub exported: bool,
    pub include_tests: bool,
    pub _min_confidence: f64,
    pub limit: usize,
    pub json: bool,
}

/// Show symbols from the codemap, optionally filtered by file and other criteria.
pub fn run_symbols(ctx: &CliContext, filter: &SymbolFilter) -> Result<()> {
    let root = ctx.resolve_root()?;

    let codemap_path = workspace::code_dir(&root).join("codemap.json");

    if !codemap_path.exists() {
        println!(
            "{}",
            "Codemap not found. Run `graxus index` first.".yellow()
        );
        return Ok(());
    }

    let content = std::fs::read_to_string(&codemap_path)?;
    let codemap: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(symbols) = codemap.get("symbols").and_then(|f| f.as_array()) {
        let filtered: Vec<_> = symbols
            .iter()
            .filter(|s| {
                // File filter
                if let Some(ref f) = filter.file {
                    let path = s.get("file").and_then(|v| v.as_str()).unwrap_or("");
                    let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    if !path.contains(f.as_str()) && !name.contains(f.as_str()) {
                        return false;
                    }
                }
                // Kind filter
                if let Some(ref k) = filter.kind {
                    let sym_kind = s.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                    if !sym_kind.eq_ignore_ascii_case(k) {
                        return false;
                    }
                }
                // Language filter
                if let Some(ref l) = filter.lang {
                    let sym_lang = s.get("language").and_then(|v| v.as_str()).unwrap_or("");
                    if !sym_lang.eq_ignore_ascii_case(l) {
                        return false;
                    }
                }
                // Exported filter
                if filter.exported {
                    let is_exported = s
                        .get("exported")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if !is_exported {
                        return false;
                    }
                }
                // Test filter
                if !filter.include_tests {
                    let is_test = s
                        .get("is_test")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if is_test {
                        return false;
                    }
                }
                true
            })
            .take(filter.limit)
            .collect();

        if filter.json {
            println!("{}", serde_json::to_string_pretty(&filtered)?);
        } else {
            println!("{}", "=== Symbols ===".green().bold());
            for sym in &filtered {
                let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
                let path = sym.get("file").and_then(|v| v.as_str()).unwrap_or("?");
                let line = sym.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0);
                let vis = sym.get("visibility").and_then(|v| v.as_str()).unwrap_or("");
                let vis_str = if vis == "public" { " pub" } else { "" };
                let signature = sym.get("signature").and_then(|v| v.as_str()).unwrap_or("");
                let is_test = sym
                    .get("is_test")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let test_str = if is_test { " [test]" } else { "" };
                let usage = sym.get("usage_count").and_then(|v| v.as_u64()).unwrap_or(0);
                let usage_str = if usage > 0 {
                    format!(" ({} calls)", usage)
                } else {
                    String::new()
                };
                if signature.is_empty() {
                    println!(
                        "  {}:{} — {} {}{}{} (line {}){}",
                        path.cyan(),
                        line,
                        kind,
                        name,
                        vis_str,
                        test_str,
                        line,
                        usage_str
                    );
                } else {
                    println!(
                        "  {}:{} — {} {}{}{} (line {}){}\n    signature: {}",
                        path.cyan(),
                        line,
                        kind,
                        name,
                        vis_str,
                        test_str,
                        line,
                        usage_str,
                        signature.dimmed()
                    );
                }
            }
            println!("\n  Total: {} symbols", filtered.len());
        }
    }

    Ok(())
}

/// Show all imports for a given file.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `file` - File path to show imports for
/// * `resolved` - If true, only show resolved imports
/// * `min_confidence` - Minimum confidence threshold
/// * `json` - Output as JSON
pub fn run_imports(
    ctx: &CliContext,
    file: &str,
    resolved: bool,
    min_confidence: f64,
    json: bool,
) -> Result<()> {
    let root = ctx.resolve_root()?;

    let codemap_path = workspace::code_dir(&root).join("codemap.json");

    if !codemap_path.exists() {
        println!(
            "{}",
            "Codemap not found. Run `graxus index` first.".yellow()
        );
        return Ok(());
    }

    let content = std::fs::read_to_string(&codemap_path)?;
    let codemap: serde_json::Value = serde_json::from_str(&content)?;

    println!("{}", format!("=== Imports for {} ===", file).green().bold());

    if let Some(imports) = codemap.get("imports").and_then(|f| f.as_array()) {
        let filtered: Vec<_> = imports
            .iter()
            .filter(|i| {
                // File filter
                if !i.get("file")
                    .and_then(|v| v.as_str())
                    .map(|p| p.contains(file))
                    .unwrap_or(false)
                {
                    return false;
                }
                // Resolved filter
                if resolved {
                    let has_resolved = i.get("resolved_file").and_then(|v| v.as_str()).is_some();
                    if !has_resolved {
                        return false;
                    }
                }
                // Min confidence filter
                if min_confidence > 0.0 {
                    let conf = i
                        .get("confidence")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    if conf < min_confidence {
                        return false;
                    }
                }
                true
            })
            .collect();

        if json {
            println!("{}", serde_json::to_string_pretty(&filtered)?);
        } else if filtered.is_empty() {
            println!("  No imports found for this file.");
        } else {
            for imp in &filtered {
                let source = imp.get("source").and_then(|v| v.as_str()).unwrap_or("?");
                let local = imp
                    .get("local_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let kind = imp.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
                let conf = imp
                    .get("confidence")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let conf_str = if conf.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", conf)
                };
                println!("  {} {} from {}{}", kind, local, source, conf_str);
            }
            println!("\n  Total: {} imports", filtered.len());
        }
    }

    Ok(())
}

/// Show callers and callees for a given symbol.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `symbol` - Symbol name to look up
/// * `depth` - Traversal depth for call chains
/// * `min_confidence` - Minimum confidence threshold
/// * `json` - Output as JSON
pub fn run_calls(
    ctx: &CliContext,
    symbol: &str,
    depth: usize,
    _min_confidence: f64,
    json: bool,
) -> Result<()> {
    let root = ctx.resolve_root()?;

    let codemap_path = workspace::code_dir(&root).join("codemap.json");
    if !codemap_path.exists() {
        println!(
            "{}",
            "Codemap not found. Run `graxus index` first.".yellow()
        );
        return Ok(());
    }

    let content = std::fs::read_to_string(&codemap_path)?;
    let codemap: serde_json::Value = serde_json::from_str(&content)?;

    // Find calls where symbol is the caller
    if let Some(calls) = codemap.get("calls").and_then(|f| f.as_array()) {
        let callers: Vec<_> = calls
            .iter()
            .filter(|c| {
                c.get("caller_symbol")
                    .and_then(|v| v.as_str())
                    .map(|s| s.contains(symbol))
                    .unwrap_or(false)
            })
            .take(depth * 10)
            .collect();

        let callees: Vec<_> = calls
            .iter()
            .filter(|c| {
                c.get("callee_text")
                    .and_then(|v| v.as_str())
                    .map(|s| s == symbol)
                    .unwrap_or(false)
            })
            .take(depth * 10)
            .collect();

        if json {
            let result = serde_json::json!({
                "symbol": symbol,
                "outgoing": callers,
                "incoming": callees,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!(
                "{}",
                format!("=== Calls from {} ===", symbol).green().bold()
            );
            if callers.is_empty() {
                println!("  No outgoing calls found.");
            } else {
                for call in &callers {
                    let callee = call
                        .get("callee_text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let file = call.get("file").and_then(|v| v.as_str()).unwrap_or("?");
                    let line = call.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                    let kind = call.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
                    println!(
                        "  {}:{} — {} {} (line {})",
                        file.cyan(),
                        line,
                        kind,
                        callee,
                        line
                    );
                }
                println!("\n  Total: {} outgoing calls", callers.len());
            }

            println!(
                "\n{}",
                format!("=== Calls to {} ===", symbol).green().bold()
            );
            if callees.is_empty() {
                println!("  No incoming calls found.");
            } else {
                for call in &callees {
                    let caller = call
                        .get("caller_symbol")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(unknown)");
                    let file = call.get("file").and_then(|v| v.as_str()).unwrap_or("?");
                    let line = call.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
                    println!(
                        "  {}:{} — called by {} (line {})",
                        file.cyan(),
                        line,
                        caller,
                        line
                    );
                }
                println!("\n  Total: {} incoming calls", callees.len());
            }
        }
    }

    Ok(())
}

/// Show all files transitively impacted by changes to a given file.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `file` - File path to analyze impact for
/// * `depth` - Traversal depth
pub fn run_impacted(ctx: &CliContext, file: &str, _depth: usize) -> Result<()> {
    let root = ctx.resolve_root()?;

    let codemap_path = workspace::code_dir(&root).join("codemap.json");

    if !codemap_path.exists() {
        println!(
            "{}",
            "Codemap not found. Run `graxus index` first.".yellow()
        );
        return Ok(());
    }

    let content = std::fs::read_to_string(&codemap_path)?;
    let codemap: serde_json::Value = serde_json::from_str(&content)?;

    println!(
        "{}",
        format!("=== Files impacted by {} ===", file).green().bold()
    );

    // Find files that import the given file
    let mut impacted = Vec::new();
    if let Some(imports) = codemap.get("imports").and_then(|f| f.as_array()) {
        for imp in imports {
            let source = imp.get("source").and_then(|v| v.as_str()).unwrap_or("");
            let imp_file = imp.get("file").and_then(|v| v.as_str()).unwrap_or("");
            if source.contains(file) {
                impacted.push(imp_file.to_string());
            }
        }
    }
    impacted.sort();
    impacted.dedup();

    if impacted.is_empty() {
        println!("  No impacted files found.");
    } else {
        for path in &impacted {
            println!("  {}", path);
        }
        println!("\n  Total: {} files", impacted.len());
    }

    Ok(())
}

/// Export codemap data in various formats.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `format` - Output format: "json", "csv", "markdown"
/// * `output` - Optional output file path (stdout if omitted)
/// * `save` - If true, save to .graxus/exports/ with auto-generated filename
pub fn run_export(ctx: &CliContext, format: &str, output: Option<&str>, save: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

    let codemap_path = workspace::code_dir(&root).join("codemap.json");
    if !codemap_path.exists() {
        println!(
            "{}",
            "Codemap not found. Run `graxus index` first.".yellow()
        );
        return Ok(());
    }

    let content = std::fs::read_to_string(&codemap_path)?;
    let codemap: serde_json::Value = serde_json::from_str(&content)?;

    let ext = match format {
        "json" => "json",
        "csv" => "csv",
        "markdown" | "md" => "md",
        _ => anyhow::bail!("Unknown format: {}. Use json, csv, or markdown", format),
    };

    let output_content = match format {
        "json" => serde_json::to_string_pretty(&codemap)?,
        "csv" => codemap_to_csv(&codemap)?,
        "markdown" | "md" => codemap_to_markdown(&codemap)?,
        _ => unreachable!(),
    };

    // Determine save path: --output > --save > stdout
    let save_path = if let Some(path) = output {
        Some(path.to_string())
    } else if save {
        let exports_dir = root.join(".graxus").join("exports");
        std::fs::create_dir_all(&exports_dir)?;
        let filename = format!("codemap.{}", ext);
        Some(exports_dir.join(&filename).to_string_lossy().to_string())
    } else {
        None
    };

    match save_path {
        Some(path) => {
            std::fs::write(&path, &output_content)?;
            println!("  {} {}", "Saved:".green(), path);
        }
        None => {
            println!("{}", output_content);
        }
    }

    Ok(())
}

fn codemap_to_csv(codemap: &serde_json::Value) -> Result<String> {
    let mut csv = String::new();
    // Symbols
    csv.push_str("type,name,kind,file,line_start,line_end,language,exported\n");
    if let Some(symbols) = codemap.get("symbols").and_then(|v| v.as_array()) {
        for sym in symbols {
            let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let line_start = sym.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0);
            let line_end = sym.get("line_end").and_then(|v| v.as_u64()).unwrap_or(0);
            let language = sym.get("language").and_then(|v| v.as_str()).unwrap_or("");
            let exported = sym.get("exported").and_then(|v| v.as_bool()).unwrap_or(false);
            csv.push_str(&format!(
                "symbol,{},{},{},{},{},{},{}\n",
                name, kind, file, line_start, line_end, language, exported
            ));
        }
    }
    // Imports
    csv.push_str("\ntype,source,local_name,file,line,kind\n");
    if let Some(imports) = codemap.get("imports").and_then(|v| v.as_array()) {
        for imp in imports {
            let source = imp.get("source").and_then(|v| v.as_str()).unwrap_or("");
            let local = imp.get("local_name").and_then(|v| v.as_str()).unwrap_or("");
            let file = imp.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let line = imp.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
            let kind = imp.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            csv.push_str(&format!("import,{},{},{},{},{}\n", source, local, file, line, kind));
        }
    }
    // Calls
    csv.push_str("\ntype,caller,callee,file,line,kind\n");
    if let Some(calls) = codemap.get("calls").and_then(|v| v.as_array()) {
        for call in calls {
            let caller = call.get("caller_symbol").and_then(|v| v.as_str()).unwrap_or("");
            let callee = call.get("callee_text").and_then(|v| v.as_str()).unwrap_or("");
            let file = call.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let line = call.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
            let kind = call.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            csv.push_str(&format!("call,{},{},{},{},{}\n", caller, callee, file, line, kind));
        }
    }
    Ok(csv)
}

fn codemap_to_markdown(codemap: &serde_json::Value) -> Result<String> {
    let mut md = String::new();
    md.push_str("# Code Codemap\n\n");

    if let Some(files) = codemap.get("files").and_then(|v| v.as_array()) {
        md.push_str(&format!("## Files ({} total)\n\n", files.len()));
        for file in files {
            let path = file.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let lang = file.get("language").and_then(|v| v.as_str()).unwrap_or("?");
            md.push_str(&format!("- `{}` ({})\n", path, lang));
        }
        md.push('\n');
    }

    if let Some(symbols) = codemap.get("symbols").and_then(|v| v.as_array()) {
        md.push_str(&format!("## Symbols ({} total)\n\n", symbols.len()));
        md.push_str("| Name | Kind | File | Line |\n");
        md.push_str("|------|------|------|------|\n");
        for sym in symbols {
            let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
            let file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("?");
            let line = sym.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0);
            md.push_str(&format!("| {} | {} | {} | {} |\n", name, kind, file, line));
        }
        md.push('\n');
    }

    if let Some(imports) = codemap.get("imports").and_then(|v| v.as_array()) {
        md.push_str(&format!("## Imports ({} total)\n\n", imports.len()));
        for imp in imports {
            let source = imp.get("source").and_then(|v| v.as_str()).unwrap_or("?");
            let local = imp.get("local_name").and_then(|v| v.as_str()).unwrap_or("?");
            let file = imp.get("file").and_then(|v| v.as_str()).unwrap_or("?");
            md.push_str(&format!("- `{}` as `{}` in `{}`\n", source, local, file));
        }
        md.push('\n');
    }

    if let Some(calls) = codemap.get("calls").and_then(|v| v.as_array()) {
        md.push_str(&format!("## Calls ({} total)\n\n", calls.len()));
        for call in calls {
            let caller = call.get("caller_symbol").and_then(|v| v.as_str()).unwrap_or("?");
            let callee = call.get("callee_text").and_then(|v| v.as_str()).unwrap_or("?");
            let file = call.get("file").and_then(|v| v.as_str()).unwrap_or("?");
            md.push_str(&format!("- `{}` → `{}` in `{}`\n", caller, callee, file));
        }
    }

    Ok(md)
}
