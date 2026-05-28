use anyhow::{Context, Result};
use colored::Colorize;
use graxus_core::{config::GraxusConfig, scanner, workspace};
use graxus_docgraph::graph::DocGraph;
use std::env;
use std::path::Path;

pub fn run(query: Option<&str>, file: Option<&str>, symbol: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let config = GraxusConfig::load(&root)?;

    if let Some(q) = query {
        // Query-based context: search docs and code
        println!("{}", format!("=== Context for \"{}\" ===", q).green().bold());

        // Search docs graph
        let docs_dir = workspace::docs_dir(&root);
        let mut found_docs: Vec<(String, String)> = Vec::new();
        if let Ok(graph) = DocGraph::load(&docs_dir) {
            for node in &graph.nodes {
                let matches_title = node.title.to_lowercase().contains(&q.to_lowercase());
                let matches_tags = node.tags.iter().any(|t| t.to_lowercase().contains(&q.to_lowercase()));
                let matches_path = node.path.to_lowercase().contains(&q.to_lowercase());
                if matches_title || matches_tags || matches_path {
                    found_docs.push((node.path.clone(), node.title.clone()));
                }
            }
            if !found_docs.is_empty() {
                println!("\n  {} matching docs:", "Docs".cyan().bold());
                for (path, title) in &found_docs {
                    println!("    {} — {}", path, title);
                }
            }
        }

        // Search code files
        let (_docs, code, _) = scanner::scan_categorized(&root, &config)?;
        let code_files: Vec<_> = code.iter().collect();
        let mut found_code = Vec::new();

        for file in &code_files {
            let content = match std::fs::read_to_string(&file.path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let q_lower = q.to_lowercase();
            let mut matches = 0;
            for line in content.lines() {
                if line.to_lowercase().contains(&q_lower) {
                    matches += 1;
                }
            }

            if matches > 0 || file.relative_path.to_lowercase().contains(&q_lower) {
                found_code.push((file.relative_path.clone(), matches));
            }
        }

        if !found_code.is_empty() {
            println!("\n  {} matching code files:", "Code".cyan().bold());
            for (path, count) in &found_code {
                if *count > 0 {
                    println!("    {} ({} matches)", path, count);
                } else {
                    println!("    {} (path match)", path);
                }
            }
        }

        if found_docs.is_empty() && found_code.is_empty() {
            println!("  No context found for \"{}\"", q);
        }

    } else if let Some(f) = file {
        // File-based context: show everything about a file
        println!("{}", format!("=== Context for {} ===", f).green().bold());

        // Check if it's a doc file
        let docs_dir = workspace::docs_dir(&root);
        if let Ok(graph) = DocGraph::load(&docs_dir) {
            if let Some(node) = graph.find_by_path(f) {
                println!("\n  {} {}", "Doc:".cyan().bold(), node.title);
                println!("    Path: {}", node.path);
                if !node.tags.is_empty() {
                    println!("    Tags: {}", node.tags.join(", "));
                }
                if !node.headings.is_empty() {
                    println!("    Headings:");
                    for h in &node.headings {
                        println!("      {} (line {})", h.text, h.line);
                    }
                }
                let backlinks = graph.get_backlinks(&node.id);
                if !backlinks.is_empty() {
                    println!("    Backlinks:");
                    for bl in backlinks {
                        println!("      {}", bl.path);
                    }
                }
            }
        }

        // Check codemap
        let codemap_path = workspace::code_dir(&root).join("codemap.json");
        if let Ok(content) = std::fs::read_to_string(&codemap_path) {
            if let Ok(codemap) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(files) = codemap.get("files").and_then(|v| v.as_array()) {
                    for file_data in files {
                        let path = file_data.get("file").and_then(|v| v.as_str()).unwrap_or("");
                        if !path.contains(f) {
                            continue;
                        }
                        println!("\n  {} {}", "Code:".cyan().bold(), path);
                        if let Some(defs) = file_data.get("definitions").and_then(|v| v.as_array()) {
                            if !defs.is_empty() {
                                println!("    Definitions:");
                                for def in defs {
                                    let name = def.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                                    let kind = def.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
                                    println!("      {} {}", kind, name);
                                }
                            }
                        }
                        if let Some(imports) = file_data.get("imports").and_then(|v| v.as_array()) {
                            if !imports.is_empty() {
                                println!("    Imports:");
                                for imp in imports {
                                    let source = imp.get("fact").and_then(|v| v.get("source")).and_then(|v| v.as_str()).unwrap_or("?");
                                    println!("      {}", source);
                                }
                            }
                        }
                    }
                }
            }
        }

    } else if let Some(s) = symbol {
        // Symbol-based context: find symbol across codebase
        println!("{}", format!("=== Context for symbol \"{}\" ===", s).green().bold());

        let codemap_path = workspace::code_dir(&root).join("codemap.json");
        if let Ok(content) = std::fs::read_to_string(&codemap_path) {
            if let Ok(codemap) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(files) = codemap.get("files").and_then(|v| v.as_array()) {
                    for file_data in files {
                        let path = file_data.get("file").and_then(|v| v.as_str()).unwrap_or("");
                        if let Some(defs) = file_data.get("definitions").and_then(|v| v.as_array()) {
                            for def in defs {
                                let name = def.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                if name == s {
                                    let kind = def.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
                                    let line = def.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0);
                                    println!("  {} {} in {} (line {})", kind, name, path, line);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            println!("  Codemap not found. Run `graxus index` first.");
        }

    } else {
        println!("{}", "Provide --query, --file, or --symbol".yellow());
        println!("  graxus context --query \"auth\"");
        println!("  graxus context --file src/auth/session.ts");
        println!("  graxus context --symbol validateSession");
    }

    Ok(())
}

pub fn run_export() -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let config = GraxusConfig::load(&root)?;

    let mut export = serde_json::Map::new();

    // Export config
    export.insert("config".to_string(), serde_json::to_value(&config)?);

    // Export docs graph
    let docs_dir = workspace::docs_dir(&root);
    if let Ok(graph) = DocGraph::load(&docs_dir) {
        export.insert("docs_graph".to_string(), serde_json::to_value(&graph)?);
    }

    // Export codemap
    let codemap_path = workspace::code_dir(&root).join("codemap.json");
    if let Ok(content) = std::fs::read_to_string(&codemap_path) {
        if let Ok(codemap) = serde_json::from_str::<serde_json::Value>(&content) {
            export.insert("codemap".to_string(), codemap);
        }
    }

    // Export file list
    let files_path = root.join(".graxus").join("files.json");
    if let Ok(content) = std::fs::read_to_string(&files_path) {
        if let Ok(files) = serde_json::from_str::<serde_json::Value>(&content) {
            export.insert("files".to_string(), files);
        }
    }

    println!("{}", serde_json::to_string_pretty(&export)?);
    Ok(())
}
