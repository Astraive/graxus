use anyhow::Result;
use colored::Colorize;
use graxus_agent_api::{AgentExport, BridgeBuilder};
use graxus_codemap::CodeGraph;
use graxus_core::{scanner, workspace};
use graxus_docgraph::graph::DocGraph;
use std::path::Path;

use crate::context::CliContext;

/// Query agent context by text, file, or symbol.
///
/// # Arguments
/// * `query` - Optional text query to search docs and code
/// * `file` - Optional file path for file-based context
/// * `symbol` - Optional symbol name for symbol-based context
/// * `_budget` - Token budget for context assembly
/// * `_max_files` - Maximum files to include in context
/// * `_max_symbols` - Maximum symbols to include in context
/// * `_max_notes` - Maximum doc notes to include in context
/// * `_depth` - Maximum traversal depth
/// * `_min_confidence` - Minimum confidence score (0-100)
#[allow(clippy::too_many_arguments)]
pub fn run(
    ctx: &CliContext,
    query: Option<&str>,
    file: Option<&str>,
    symbol: Option<&str>,
    _budget: usize,
    _max_files: usize,
    _max_symbols: usize,
    _max_notes: usize,
    _depth: usize,
    _min_confidence: f64,
) -> Result<()> {
    let root = ctx.resolve_root()?;

    let config = ctx.load_config(&root)?;

    if let Some(q) = query {
        // Query-based context: search docs and code
        println!(
            "{}",
            format!("=== Context for \"{}\" ===", q).green().bold()
        );

        // Search docs graph
        let docs_dir = workspace::docs_dir(&root);
        let mut found_docs: Vec<(String, String)> = Vec::new();
        if let Ok(graph) = DocGraph::load(&docs_dir) {
            for node in &graph.nodes {
                let matches_title = node.title.to_lowercase().contains(&q.to_lowercase());
                let matches_tags = node
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&q.to_lowercase()));
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

        if let Some(codemap) = load_code_graph(&root) {
            let matching_files = codemap
                .files
                .iter()
                .filter(|file| file.path.contains(f))
                .collect::<Vec<_>>();
            for file_data in matching_files {
                println!("\n  {} {}", "Code:".cyan().bold(), file_data.path);
                let symbols = codemap
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.file == file_data.path);
                for symbol in symbols {
                    println!(
                        "    {:?} {} (line {})",
                        symbol.kind, symbol.name, symbol.line_start
                    );
                }
                let imports = codemap
                    .imports
                    .iter()
                    .filter(|import| import.file == file_data.path);
                for import in imports {
                    println!("    Import: {}", import.source);
                }
                for route in codemap
                    .routes
                    .iter()
                    .filter(|route| route.file == file_data.path)
                {
                    println!(
                        "    Route: {} {} -> {}",
                        route.method, route.path, route.handler
                    );
                }
                for type_impl in codemap
                    .type_impls
                    .iter()
                    .filter(|fact| fact.file == file_data.path)
                {
                    println!(
                        "    Type: {} -> {}",
                        type_impl.implementing_type, type_impl.trait_or_interface
                    );
                }
                for binding in codemap
                    .di_bindings
                    .iter()
                    .filter(|fact| fact.file == file_data.path)
                {
                    println!(
                        "    DI: {} -> {}",
                        binding.abstract_type, binding.concrete_type
                    );
                }
            }
        } else {
            println!("  Codemap not found. Run `graxus index` first.");
        }
    } else if let Some(s) = symbol {
        // Symbol-based context: find symbol across codebase
        println!(
            "{}",
            format!("=== Context for symbol \"{}\" ===", s)
                .green()
                .bold()
        );

        if let Some(codemap) = load_code_graph(&root) {
            let mut found = false;
            for symbol in codemap.symbols.iter().filter(|symbol| symbol.name == s) {
                found = true;
                println!(
                    "  {:?} {} in {} (line {})",
                    symbol.kind, symbol.name, symbol.file, symbol.line_start
                );
                for route in codemap
                    .routes
                    .iter()
                    .filter(|route| route.handler == symbol.name)
                {
                    println!(
                        "    Route: {} {} ({})",
                        route.method, route.path, route.framework
                    );
                }
            }
            for type_impl in codemap
                .type_impls
                .iter()
                .filter(|fact| fact.implementing_type == s || fact.trait_or_interface == s)
            {
                found = true;
                println!(
                    "  Type: {} -> {} in {}",
                    type_impl.implementing_type, type_impl.trait_or_interface, type_impl.file
                );
            }
            for binding in codemap
                .di_bindings
                .iter()
                .filter(|fact| fact.abstract_type == s || fact.concrete_type == s)
            {
                found = true;
                println!(
                    "  DI: {} -> {} in {}",
                    binding.abstract_type, binding.concrete_type, binding.file
                );
            }
            if !found {
                println!("  No symbol or semantic fact found for \"{}\"", s);
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

fn load_code_graph(root: &Path) -> Option<CodeGraph> {
    let codemap_path = workspace::code_dir(root).join("codemap.json");
    std::fs::read_to_string(codemap_path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

/// Export the full agent context, optionally applying a token budget.
pub fn run_export(ctx: &CliContext, budget: Option<usize>, json: bool) -> Result<()> {
    let root = ctx.resolve_root()?;
    let config = ctx.load_config(&root)?;
    let docs_dir = workspace::docs_dir(&root);
    let doc_graph = DocGraph::load(&docs_dir).unwrap_or_default();
    let codemap_path = workspace::code_dir(&root).join("codemap.json");
    let code_graph = std::fs::read_to_string(&codemap_path)
        .ok()
        .and_then(|content| serde_json::from_str::<CodeGraph>(&content).ok())
        .unwrap_or_default();
    let bridge = BridgeBuilder::build(&doc_graph, &code_graph).unwrap_or_default();
    let export = AgentExport::new(&config.project.name, doc_graph, code_graph, bridge);
    let export = budget
        .map(|max_tokens| export.export_bounded(max_tokens))
        .unwrap_or(export);

    if json {
        if budget.is_none() {
            let mut compatibility = serde_json::Map::new();
            compatibility.insert("config".into(), serde_json::to_value(&config)?);
            compatibility.insert(
                "docs_graph".into(),
                serde_json::to_value(&export.doc_graph)?,
            );
            compatibility.insert("codemap".into(), serde_json::to_value(&export.code_graph)?);
            let files_path = root.join(".graxus").join("files.json");
            if let Ok(content) = std::fs::read_to_string(files_path) {
                if let Ok(files) = serde_json::from_str(&content) {
                    compatibility.insert("files".into(), files);
                }
            }
            println!("{}", serde_json::to_string_pretty(&compatibility)?);
        } else {
            println!("{}", serde_json::to_string_pretty(&export)?);
        }
    } else {
        let stats = export.stats();
        println!("{}", "=== Agent Export ===".green().bold());
        println!("  Project: {}", export.project_name);
        println!("  Files: {}", stats.code_files);
        println!("  Symbols: {}", stats.symbols);
        println!("  Imports: {}", stats.imports);
        println!("  Calls: {}", stats.calls);
        println!("  Routes: {}", stats.routes);
        println!("  Type implementations: {}", stats.type_impls);
        println!("  DI bindings: {}", stats.di_bindings);
        println!("  Documentation nodes: {}", stats.doc_nodes);
        println!("  Bridge edges: {}", stats.bridge_edges);
    }
    Ok(())
}
