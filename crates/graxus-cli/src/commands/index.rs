use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::env;
use std::path::Path;

use graxus_core::{config::GraxusConfig, scanner, workspace};
use graxus_docgraph as docgraph;
use graxus_codemap as codemap;

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let config = GraxusConfig::load(&root)?;

    println!("{}", "Indexing project...".green().bold());

    // Step 1: Scan files
    let (docs, code, config_files) = scanner::scan_categorized(&root, &config)?;

    println!("\n{}", "Scan Results:".green().bold());
    println!("  Total files:    {}", docs.len() + code.len() + config_files.len());
    println!("  Docs files:     {}", docs.len());
    println!("  Code files:     {}", code.len());
    println!("  Config files:   {}", config_files.len());

    // Save file list to .graxus/files.json
    let all_files: Vec<_> = docs.iter().chain(code.iter()).chain(config_files.iter()).collect();
    let files_json = serde_json::to_string_pretty(&all_files)?;
    let files_path = root.join(".graxus").join("files.json");
    std::fs::write(&files_path, files_json)?;
    println!("\n  Saved file list to {}", files_path.display());

    // Summary by language
    let mut lang_counts: HashMap<String, usize> = HashMap::new();
    for file in &code {
        *lang_counts.entry(file.language.as_str().to_string()).or_insert(0) += 1;
    }
    if !lang_counts.is_empty() {
        println!("\n{}", "Languages:".green().bold());
        let mut sorted: Vec<_> = lang_counts.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (lang, count) in sorted {
            println!("  {}: {} files", lang, count);
        }
    }

    // Step 2: Build docs graph
    if config.docs.enabled {
        println!("\n{}", "Building docs graph...".green().bold());
        match docgraph::build(&root, &config) {
            Ok(graph) => {
                println!("  Nodes: {}", graph.nodes.len());
                println!("  Edges: {}", graph.edges.len());
                println!("  Tags:  {}", graph.get_all_tags().len());
                println!("  Saved to .graxus/docs/");
            }
            Err(e) => {
                eprintln!("  {} {}", "Warning:".yellow(), e);
            }
        }
    }

    // Step 3: Build codemap
    if config.code.enabled {
        println!("\n{}", "Code codemap:".green().bold());
        let builder = codemap::CodemapBuilder::new(code.clone());
        match builder.build() {
            Ok(graph) => {
                println!("  Files:    {}", graph.files.len());
                println!("  Symbols:  {}", graph.symbols.len());
                println!("  Imports:  {}", graph.imports.len());
                println!("  Calls:    {}", graph.calls.len());
                // Save codemap
                let output_dir = root.join(".graxus").join("code");
                if let Err(e) = codemap::CodemapBuilder::save(&graph, &output_dir) {
                    eprintln!("  {} Failed to save codemap: {}", "Warning:".yellow(), e);
                } else {
                    println!("  Saved to .graxus/code/");
                }
            }
            Err(e) => {
                eprintln!("  {} {}", "Warning:".yellow(), e);
            }
        }
    }

    // Step 4: Save to SQLite
    if config.code.enabled {
        let db_path = root.join(".graxus").join("index.db");
        match graxus_index::sqlite::SqliteStore::new(&db_path) {
            Ok(db) => {
                let code_json = root.join(".graxus").join("code").join("codemap.json");
                if let Ok(content) = std::fs::read_to_string(&code_json) {
                    if let Ok(codemap) = serde_json::from_str::<serde_json::Value>(&content) {
                        // Insert symbols
                        if let Some(symbols) = codemap.get("symbols").and_then(|v| v.as_array()) {
                            for sym in symbols {
                                let _ = db.insert_symbol(
                                    sym.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("file").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("language").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("kind").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("exported").and_then(|v| v.as_bool()).unwrap_or(false),
                                    sym.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                                    sym.get("line_end").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                                    sym.get("visibility").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("signature").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("is_test").and_then(|v| v.as_bool()).unwrap_or(false),
                                    sym.get("usage_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                                );
                            }
                        }
                        // Insert imports
                        if let Some(imports) = codemap.get("imports").and_then(|v| v.as_array()) {
                            for imp in imports {
                                let _ = db.insert_import(
                                    imp.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                    imp.get("file").and_then(|v| v.as_str()).unwrap_or(""),
                                    imp.get("language").and_then(|v| v.as_str()).unwrap_or(""),
                                    imp.get("kind").and_then(|v| v.as_str()).unwrap_or(""),
                                    imp.get("source").and_then(|v| v.as_str()).unwrap_or(""),
                                    imp.get("local_name").and_then(|v| v.as_str()),
                                    imp.get("imported_name").and_then(|v| v.as_str()),
                                    imp.get("resolved_file").and_then(|v| v.as_str()),
                                    imp.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                                    imp.get("confidence").and_then(|v| v.as_str()).unwrap_or(""),
                                );
                            }
                        }
                        // Insert calls
                        if let Some(calls) = codemap.get("calls").and_then(|v| v.as_array()) {
                            for call in calls {
                                let _ = db.insert_call(
                                    call.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                    call.get("file").and_then(|v| v.as_str()).unwrap_or(""),
                                    call.get("language").and_then(|v| v.as_str()).unwrap_or(""),
                                    call.get("kind").and_then(|v| v.as_str()).unwrap_or(""),
                                    call.get("caller_symbol").and_then(|v| v.as_str()),
                                    call.get("callee_text").and_then(|v| v.as_str()).unwrap_or(""),
                                    call.get("object").and_then(|v| v.as_str()),
                                    call.get("resolved_symbol").and_then(|v| v.as_str()),
                                    call.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                                    call.get("column").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                                    call.get("confidence").and_then(|v| v.as_str()).unwrap_or(""),
                                );
                            }
                        }
                        println!("  SQLite: {} symbols, {} imports, {} calls",
                            db.symbol_count().unwrap_or(0),
                            db.import_count().unwrap_or(0),
                            db.call_count().unwrap_or(0));
                    }
                }
            }
            Err(e) => eprintln!("  {} SQLite: {}", "Warning:".yellow(), e),
        }
    }

    println!("\n{}", "Index complete.".green().bold());
    Ok(())
}
