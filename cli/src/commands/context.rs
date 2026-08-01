use anyhow::Result;
use colored::Colorize;
use graxus_agent_api::{AgentContext, AgentExport, BridgeBuilder, ContextBudget, ContextEngine};
use graxus_codemap::CodeGraph;
use graxus_core::workspace;
use graxus_docgraph::graph::DocGraph;
use std::collections::HashSet;
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
        // Query through the agent context engine so semantic facts and all
        // limits use the same path as structured consumers.
        let docs_dir = workspace::docs_dir(&root);
        let doc_graph = DocGraph::load(&docs_dir).unwrap_or_default();
        let code_graph = load_code_graph(&root).unwrap_or_default();
        let bridge = BridgeBuilder::build(&doc_graph, &code_graph).unwrap_or_default();
        let engine = ContextEngine::new(doc_graph, code_graph, bridge);
        let mut context = engine.query_bounded(q, ContextBudget::new(_budget));
        let max_edges = config
            .context
            .max_edges
            .unwrap_or(config.defaults.max_edges);
        limit_query_context(
            &mut context,
            _max_files,
            _max_symbols.min(config.defaults.max_nodes),
            _max_notes,
            max_edges,
            _depth,
            _min_confidence,
        );
        print_query_context(&context);
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
                for route in codemap.routes.iter().filter(|route| {
                    route.file == file_data.path
                        || route.handler_file.as_deref() == Some(file_data.path.as_str())
                }) {
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
/// Apply the same structural limits used by the context command's other
/// branches to a query result. Semantic facts do not carry confidence scores,
/// so the confidence filter applies to confidence-bearing graph facts only.
fn limit_query_context(
    context: &mut AgentContext,
    max_files: usize,
    max_nodes: usize,
    max_notes: usize,
    max_edges: usize,
    depth: usize,
    min_confidence: f64,
) {
    for symbol in &context.code {
        context.related_files.push(symbol.file.clone());
    }
    for import in &context.imports {
        context.related_files.push(import.file.clone());
    }
    for call in &context.calls {
        context.related_files.push(call.file.clone());
    }
    for route in &context.routes {
        context.related_files.push(route.file.clone());
        if let Some(handler_file) = &route.handler_file {
            context.related_files.push(handler_file.clone());
        }
    }
    for type_impl in &context.type_impls {
        context.related_files.push(type_impl.file.clone());
    }
    for binding in &context.di_bindings {
        context.related_files.push(binding.file.clone());
    }

    if max_notes > 0 {
        context.docs.truncate(max_notes);
    }
    if max_nodes > 0 {
        context.code.truncate(max_nodes);
    }

    if min_confidence > 0.0 {
        context
            .imports
            .retain(|import| import.confidence.score >= min_confidence);
        context
            .calls
            .retain(|call| call.confidence.score >= min_confidence);
    }

    // A depth of zero keeps direct query matches but excludes traversed
    // relationships. ContextEngine currently exposes one-hop relationships;
    // positive depths therefore retain those relationships.
    if depth == 0 {
        context.imports.clear();
        context.calls.clear();
        context.bridge_edges.clear();
    }

    let mut files = context.related_files.clone();
    files.sort();
    files.dedup();
    let limited_files = max_files > 0 && files.len() > max_files;
    let allowed_files: HashSet<String> = if limited_files {
        files.into_iter().take(max_files).collect()
    } else {
        files.into_iter().collect()
    };
    let file_is_allowed = |file: &str| !limited_files || allowed_files.contains(file);

    context.code.retain(|symbol| file_is_allowed(&symbol.file));
    context
        .imports
        .retain(|import| file_is_allowed(&import.file));
    context.calls.retain(|call| file_is_allowed(&call.file));
    context.routes.retain(|route| {
        file_is_allowed(&route.file) || route.handler_file.as_deref().is_some_and(file_is_allowed)
    });
    context
        .type_impls
        .retain(|type_impl| file_is_allowed(&type_impl.file));
    context
        .di_bindings
        .retain(|binding| file_is_allowed(&binding.file));
    context.related_files.retain(|file| file_is_allowed(file));
    context.related_files.sort();
    context.related_files.dedup();

    // Keep a single deterministic edge budget across graph and semantic
    // relationships. A zero cap follows the CLI convention for unlimited.
    if max_edges > 0 {
        let mut remaining = max_edges;
        take_edge_budget(&mut context.bridge_edges, &mut remaining);
        take_edge_budget(&mut context.imports, &mut remaining);
        take_edge_budget(&mut context.calls, &mut remaining);
        take_edge_budget(&mut context.routes, &mut remaining);
        take_edge_budget(&mut context.type_impls, &mut remaining);
        take_edge_budget(&mut context.di_bindings, &mut remaining);
    }
}

fn take_edge_budget<T>(items: &mut Vec<T>, remaining: &mut usize) {
    if items.len() > *remaining {
        items.truncate(*remaining);
    }
    *remaining = (*remaining).saturating_sub(items.len());
}

fn print_query_context(context: &AgentContext) {
    println!(
        "{}",
        format!("=== Context for \"{}\" ===", context.query)
            .green()
            .bold()
    );

    if !context.docs.is_empty() {
        println!("\n  {} matching docs:", "Docs".cyan().bold());
        for doc in &context.docs {
            println!("    {} — {}", doc.path, doc.title);
        }
    }
    if !context.code.is_empty() {
        println!("\n  {} matching symbols:", "Code".cyan().bold());
        for symbol in &context.code {
            println!(
                "    {:?} {} in {} (line {})",
                symbol.kind, symbol.name, symbol.file, symbol.line_start
            );
        }
    }
    if !context.imports.is_empty() {
        println!("\n  {} matching imports:", "Imports".cyan().bold());
        for import in &context.imports {
            println!("    {} <- {}", import.file, import.source);
        }
    }
    if !context.calls.is_empty() {
        println!("\n  {} matching calls:", "Calls".cyan().bold());
        for call in &context.calls {
            println!("    {} -> {}", call.file, call.callee_text);
        }
    }
    if !context.routes.is_empty() {
        println!("\n  {} matching routes:", "Routes".cyan().bold());
        for route in &context.routes {
            println!(
                "    Route: {} {} -> {} [{}] ({})",
                route.method, route.path, route.handler, route.framework, route.file
            );
        }
    }
    if !context.type_impls.is_empty() {
        println!(
            "\n  {} matching type implementations:",
            "Types".cyan().bold()
        );
        for type_impl in &context.type_impls {
            println!(
                "    Type: {} -> {} ({})",
                type_impl.implementing_type, type_impl.trait_or_interface, type_impl.file
            );
        }
    }
    if !context.di_bindings.is_empty() {
        println!("\n  {} matching dependency bindings:", "DI".cyan().bold());
        for binding in &context.di_bindings {
            println!(
                "    DI: {} -> {} [{}] ({})",
                binding.abstract_type, binding.concrete_type, binding.framework, binding.file
            );
        }
    }

    if context.docs.is_empty()
        && context.code.is_empty()
        && context.imports.is_empty()
        && context.calls.is_empty()
        && context.routes.is_empty()
        && context.type_impls.is_empty()
        && context.di_bindings.is_empty()
    {
        println!("  No context found for \"{}\"", context.query);
    }
    for warning in &context.warnings {
        println!("  Warning: {}", warning);
    }
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
        // JSON consumers receive the same rich AgentExport shape regardless
        // of whether a budget was requested. In particular, never bypass the
        // bounded export with a raw codemap/files compatibility map.
        println!("{}", serde_json::to_string_pretty(&export)?);
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
