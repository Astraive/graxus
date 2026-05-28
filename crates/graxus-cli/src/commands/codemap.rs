use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::Path;

use graxus_core::workspace;

pub fn run(json: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let codemap_path = workspace::code_dir(&root).join("codemap.json");

    if !codemap_path.exists() {
        println!("{}", "Codemap not found. Run `graxus index` first.".yellow());
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

pub fn run_symbols(file: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let codemap_path = workspace::code_dir(&root).join("codemap.json");

    if !codemap_path.exists() {
        println!("{}", "Codemap not found. Run `graxus index` first.".yellow());
        return Ok(());
    }

    let content = std::fs::read_to_string(&codemap_path)?;
    let codemap: serde_json::Value = serde_json::from_str(&content)?;

    println!("{}", "=== Symbols ===".green().bold());

    if let Some(symbols) = codemap.get("symbols").and_then(|f| f.as_array()) {
        let filtered: Vec<_> = if let Some(f) = file {
            symbols.iter().filter(|s| {
                let path = s.get("file").and_then(|v| v.as_str()).unwrap_or("");
                let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("");
                path.contains(f) || name.contains(f)
            }).collect()
        } else {
            symbols.iter().collect()
        };

        for sym in &filtered {
            let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
            let path = sym.get("file").and_then(|v| v.as_str()).unwrap_or("?");
            let line = sym.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0);
            let vis = sym.get("visibility").and_then(|v| v.as_str()).unwrap_or("");
            let vis_str = if vis == "public" { " pub" } else { "" };
            println!("  {}:{} — {} {}{} (line {})", path.cyan(), line, kind, name, vis_str, line);
        }
        println!("\n  Total: {} symbols", filtered.len());
    }

    Ok(())
}

pub fn run_imports(file: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let codemap_path = workspace::code_dir(&root).join("codemap.json");

    if !codemap_path.exists() {
        println!("{}", "Codemap not found. Run `graxus index` first.".yellow());
        return Ok(());
    }

    let content = std::fs::read_to_string(&codemap_path)?;
    let codemap: serde_json::Value = serde_json::from_str(&content)?;

    println!("{}", format!("=== Imports for {} ===", file).green().bold());

    if let Some(imports) = codemap.get("imports").and_then(|f| f.as_array()) {
        let filtered: Vec<_> = imports.iter()
            .filter(|i| i.get("file").and_then(|v| v.as_str()).map(|p| p.contains(file)).unwrap_or(false))
            .collect();
        if filtered.is_empty() {
            println!("  No imports found for this file.");
        } else {
            for imp in &filtered {
                let source = imp.get("source").and_then(|v| v.as_str()).unwrap_or("?");
                let local = imp.get("local_name").and_then(|v| v.as_str()).unwrap_or("?");
                let kind = imp.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
                println!("  {} {} from {}", kind, local, source);
            }
            println!("\n  Total: {} imports", filtered.len());
        }
    }

    Ok(())
}

pub fn run_impacted(file: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let codemap_path = workspace::code_dir(&root).join("codemap.json");

    if !codemap_path.exists() {
        println!("{}", "Codemap not found. Run `graxus index` first.".yellow());
        return Ok(());
    }

    let content = std::fs::read_to_string(&codemap_path)?;
    let codemap: serde_json::Value = serde_json::from_str(&content)?;

    println!("{}", format!("=== Files impacted by {} ===", file).green().bold());

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
