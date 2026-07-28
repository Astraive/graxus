use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;

use graxus_codemap as codemap;
use graxus_core::{scanner, workspace};
use graxus_docgraph as docgraph;

use crate::context::CliContext;
use crate::filters::{apply_filters, build_glob_set};

/// Index the project by scanning files, building the docs graph and codemap.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `docs_only` - If true, only scan documentation files
/// * `code_only` - If true, only scan code files
/// * `include` - Include glob patterns for file filtering
/// * `exclude` - Exclude glob patterns for file filtering
/// * `lang` - Filter by programming language
/// * `max_files` - Maximum number of files to process
// CLI dispatch keeps these independently parsed flags explicit.
#[allow(clippy::too_many_arguments)]
pub fn run(
    ctx: &CliContext,
    docs_only: bool,
    code_only: bool,
    include: Vec<String>,
    exclude: Vec<String>,
    lang: Vec<String>,
    max_files: Option<usize>,
    codemap_backend: String,
) -> Result<()> {
    let root = ctx.resolve_root()?;
    let config = ctx.load_config(&root)?;

    // Ensure .graxus directory structure exists
    workspace::init_graxus_dir(&root)?;

    // Compile user-supplied filters once.
    let include_set = build_glob_set(&include)?;
    let exclude_set = build_glob_set(&exclude)?;
    let deadline = ctx.deadline();

    if ctx.show_progress() {
        println!("{}", "Indexing project...".green().bold());
    }

    // Step 1: Scan files
    let spinner = if ctx.show_progress() {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"])
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        spinner.set_message("Scanning files...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));
        Some(spinner)
    } else {
        None
    };

    let (mut docs, mut code, config_files) = scanner::scan_categorized(&root, &config)?;

    if let Some(s) = &spinner {
        s.finish_and_clear();
    }

    // Apply --include / --exclude / --lang filters.
    apply_filters(&mut docs, &include_set, &exclude_set, &lang);
    apply_filters(&mut code, &include_set, &exclude_set, &lang);

    // Apply --docs-only / --code-only filters
    if docs_only {
        code.clear();
    } else if code_only {
        docs.clear();
    }

    // Apply --max-files limit
    if let Some(max) = max_files {
        docs.truncate(max);
        code.truncate(max);
    }

    if ctx.show_progress() {
        println!("\n{}", "Scan Results:".green().bold());
        println!(
            "  Total files:    {}",
            docs.len() + code.len() + config_files.len()
        );
        println!("  Docs files:     {}", docs.len());
        println!("  Code files:     {}", code.len());
        println!("  Config files:   {}", config_files.len());
    }

    // Save file list to .graxus/files.json
    let all_files: Vec<_> = docs
        .iter()
        .chain(code.iter())
        .chain(config_files.iter())
        .collect();
    let files_json = serde_json::to_string_pretty(&all_files)?;
    let files_path = root.join(".graxus").join("files.json");
    std::fs::write(&files_path, files_json)?;
    if ctx.show_progress() {
        println!("\n  Saved file list to {}", files_path.display());
    }

    // Summary by language
    if ctx.show_progress() {
        let mut lang_counts: HashMap<String, usize> = HashMap::new();
        for file in &code {
            *lang_counts
                .entry(file.language.as_str().to_string())
                .or_insert(0) += 1;
        }
        if !lang_counts.is_empty() {
            println!("\n{}", "Languages:".green().bold());
            let mut sorted: Vec<_> = lang_counts.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));
            for (lang, count) in sorted {
                println!("  {}: {} files", lang, count);
            }
        }
    }

    // Step 2: Build docs graph
    if config.docs.enabled {
        if ctx.show_progress() {
            println!("\n{}", "Building docs graph...".green().bold());
        }
        match docgraph::build(&root, &config) {
            Ok(graph) => {
                if ctx.show_progress() {
                    println!("  Nodes: {}", graph.nodes.len());
                    println!("  Edges: {}", graph.edges.len());
                    println!("  Tags:  {}", graph.get_all_tags().len());
                    println!("  Saved to .graxus/docs/");
                }
            }
            Err(e) => {
                eprintln!("  {} {}", "Warning:".yellow(), e);
            }
        }
    }

    // Step 3: Build codemap
    if config.code.enabled {
        let pb = if ctx.show_progress() {
            let pb = ProgressBar::new(code.len() as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("  {spinner:.green} Building codemap... [{bar:40.cyan/blue}] {pos}/{len} files")
                    .unwrap()
                    .progress_chars("=> "),
            );
            pb.enable_steady_tick(std::time::Duration::from_millis(50));
            println!("\n{}", "Code codemap:".green().bold());
            Some(pb)
        } else {
            None
        };
        let backend = match codemap_backend.parse::<graxus_core::ParserBackend>() {
            Ok(b) => b,
            Err(_) => {
                anyhow::bail!(
                    "invalid --codemap-backend {codemap_backend:?}; expected ripex|tree-sitter|auto"
                )
            }
        };
        let builder = codemap::CodemapBuilder::new(code.clone()).with_backend(backend);
        match builder.build() {
            Ok(graph) => {
                if let Some(pb) = &pb {
                    pb.finish_and_clear();
                    println!("  Files:    {}", graph.files.len());
                    println!("  Symbols:  {}", graph.symbols.len());
                    println!("  Imports:  {}", graph.imports.len());
                    println!("  Calls:    {}", graph.calls.len());
                    println!("  Routes:   {}", graph.routes.len());
                    println!("  Type impls: {}", graph.type_impls.len());
                    println!("  DI bindings: {}", graph.di_bindings.len());
                }
                // Save codemap
                let output_dir = root.join(".graxus").join("code");
                if let Err(e) = codemap::CodemapBuilder::save(&graph, &output_dir) {
                    eprintln!("  {} Failed to save codemap: {}", "Warning:".yellow(), e);
                } else if ctx.show_progress() {
                    println!("  Saved to .graxus/code/");
                }
            }
            Err(e) => {
                if let Some(pb) = &pb {
                    pb.finish_and_clear();
                }
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
                        // Count total items for progress bar
                        let sym_count = codemap
                            .get("symbols")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        let imp_count = codemap
                            .get("imports")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        let call_count = codemap
                            .get("calls")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        let route_count = codemap
                            .get("routes")
                            .and_then(|v| v.as_array())
                            .map_or(0, Vec::len);
                        let type_impl_count = codemap
                            .get("type_impls")
                            .and_then(|v| v.as_array())
                            .map_or(0, Vec::len);
                        let di_binding_count = codemap
                            .get("di_bindings")
                            .and_then(|v| v.as_array())
                            .map_or(0, Vec::len);
                        let parser_count = codemap
                            .get("parser_results")
                            .and_then(|v| v.as_array())
                            .map(|results| {
                                results
                                    .iter()
                                    .map(|result| {
                                        1 + result
                                            .get("facts")
                                            .and_then(|v| v.as_array())
                                            .map_or(0, |facts| facts.len())
                                    })
                                    .sum::<usize>()
                            })
                            .unwrap_or(0);
                        let total = sym_count
                            + imp_count
                            + call_count
                            + route_count
                            + type_impl_count
                            + di_binding_count
                            + parser_count;

                        let pb = if ctx.show_progress() {
                            let pb = ProgressBar::new(total as u64);
                            pb.set_style(
                                ProgressStyle::default_bar()
                                    .template("  {spinner:.green} Indexing to SQLite... [{bar:40.cyan/blue}] {pos}/{len}")
                                    .unwrap()
                                    .progress_chars("=> "),
                            );
                            Some(pb)
                        } else {
                            None
                        };

                        // Insert symbols
                        if let Some(symbols) = codemap.get("symbols").and_then(|v| v.as_array()) {
                            for sym in symbols {
                                let _ = db.insert_symbol(
                                    sym.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("file").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("language").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("kind").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("exported")
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(false),
                                    sym.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as usize,
                                    sym.get("line_end").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as usize,
                                    sym.get("visibility").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("signature").and_then(|v| v.as_str()).unwrap_or(""),
                                    sym.get("is_test")
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(false),
                                    sym.get("usage_count").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as usize,
                                );
                                if let Some(pb) = &pb {
                                    pb.inc(1);
                                }
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
                                if let Some(pb) = &pb {
                                    pb.inc(1);
                                }
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
                                    call.get("callee_text")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                    call.get("object").and_then(|v| v.as_str()),
                                    call.get("resolved_symbol").and_then(|v| v.as_str()),
                                    call.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                                    call.get("column").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as usize,
                                    call.get("confidence")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                );
                                if let Some(pb) = &pb {
                                    pb.inc(1);
                                }
                                if let Some(d) = &deadline {
                                    d.check()?;
                                }
                            }
                        }
                        // Insert framework route facts.
                        if let Some(routes) = codemap.get("routes").and_then(|v| v.as_array()) {
                            for route in routes {
                                let middleware: Vec<String> = route
                                    .get("middleware")
                                    .cloned()
                                    .and_then(|value| serde_json::from_value(value).ok())
                                    .unwrap_or_default();
                                let _ = db.insert_route(
                                    route.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                    route.get("file").and_then(|v| v.as_str()).unwrap_or(""),
                                    route.get("language").and_then(|v| v.as_str()).unwrap_or(""),
                                    route.get("method").and_then(|v| v.as_str()).unwrap_or("*"),
                                    route.get("path").and_then(|v| v.as_str()).unwrap_or(""),
                                    route.get("handler").and_then(|v| v.as_str()).unwrap_or(""),
                                    route.get("handler_file").and_then(|v| v.as_str()),
                                    route.get("line").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as usize,
                                    route
                                        .get("framework")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                    &middleware,
                                );
                                if let Some(pb) = &pb {
                                    pb.inc(1);
                                }
                            }
                        }
                        // Insert type relationship facts.
                        if let Some(type_impls) =
                            codemap.get("type_impls").and_then(|v| v.as_array())
                        {
                            for type_impl in type_impls {
                                let _ = db.insert_type_impl(
                                    type_impl.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                    type_impl.get("file").and_then(|v| v.as_str()).unwrap_or(""),
                                    type_impl
                                        .get("language")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                    type_impl
                                        .get("implementing_type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                    type_impl
                                        .get("trait_or_interface")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                    type_impl.get("line").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as usize,
                                    type_impl.get("kind").and_then(|v| v.as_str()).unwrap_or(""),
                                );
                                if let Some(pb) = &pb {
                                    pb.inc(1);
                                }
                            }
                        }
                        // Insert dependency-injection binding facts.
                        if let Some(bindings) =
                            codemap.get("di_bindings").and_then(|v| v.as_array())
                        {
                            for binding in bindings {
                                let _ = db.insert_di_binding(
                                    binding.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                    binding.get("file").and_then(|v| v.as_str()).unwrap_or(""),
                                    binding
                                        .get("language")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                    binding
                                        .get("abstract_type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                    binding
                                        .get("concrete_type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                    binding.get("lifetime").and_then(|v| v.as_str()),
                                    binding.get("line").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as usize,
                                    binding
                                        .get("framework")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(""),
                                );
                                if let Some(pb) = &pb {
                                    pb.inc(1);
                                }
                            }
                        }
                        if let Some(results) =
                            codemap.get("parser_results").and_then(|v| v.as_array())
                        {
                            for result in results {
                                if db.insert_parser_result_value(result).is_ok() {
                                    if let Some(pb) = &pb {
                                        let facts = result
                                            .get("facts")
                                            .and_then(|v| v.as_array())
                                            .map_or(0, |items| items.len());
                                        pb.inc((facts + 1) as u64);
                                    }
                                }
                            }
                        }
                        if let Some(pb) = &pb {
                            pb.finish_with_message("SQLite index complete");
                            println!(
                                "  SQLite: {} symbols, {} imports, {} calls",
                                db.symbol_count().unwrap_or(0),
                                db.import_count().unwrap_or(0),
                                db.call_count().unwrap_or(0)
                            );
                        }
                    }
                }
            }
            Err(e) => eprintln!("  {} SQLite: {}", "Warning:".yellow(), e),
        }
    }

    if ctx.show_progress() {
        println!("\n{}", "Index complete.".green().bold());
    }
    Ok(())
}
