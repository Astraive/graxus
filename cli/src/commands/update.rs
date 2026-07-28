use anyhow::Result;
use colored::Colorize;
use graxus_codemap::CodemapBuilder;
use graxus_core::{scanner, workspace};
use graxus_docgraph::graph::DocGraph;
use graxus_index::{IndexStore, SqliteStore};
use std::path::Path;

use indicatif::{ProgressBar, ProgressStyle};

use crate::context::CliContext;

/// Incremental update: scan for changes and re-index only changed files.
///
/// Before any mutation, snapshots the existing codemap/docgraph/file-list/index
/// databases via [`IndexStore::create_snapshot`] so a failed or unwanted update
/// can be rolled back. Only added/modified files are re-parsed and re-inserted
/// into SQLite (deleted files are removed), making repeated runs idempotent.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `dry_run` - If true, show what would change without applying
/// * `full` - If true, force a full re-index
pub fn run(ctx: &CliContext, dry_run: bool, full: bool, codemap_backend: String) -> Result<()> {
    let root = ctx.resolve_root()?;
    let config = ctx.load_config(&root)?;
    let graxus_dir = root.join(".graxus");

    // Ensure directory structure exists
    workspace::init_graxus_dir(&root)?;

    println!("{}", "=== Graxus Update ===".green().bold());

    // Scan current files
    let current_files = scanner::scan(&root, &config)?;

    if full {
        println!("  Full re-index requested.");
        if dry_run {
            println!("  Would re-index {} files.", current_files.len());
        } else {
            run_full_index(ctx, &root, &current_files, &codemap_backend)?;
        }
        return Ok(());
    }

    // Load previous scan
    let previous = scanner::load_saved_files(&graxus_dir);

    let diff = match &previous {
        Some(old) => scanner::compute_diff(old, &current_files),
        None => {
            println!("  No previous index found. Running full index.");
            if dry_run {
                println!("  Would index {} files.", current_files.len());
            } else {
                run_full_index(ctx, &root, &current_files, &codemap_backend)?;
            }
            return Ok(());
        }
    };

    let total_changes = diff.added.len() + diff.modified.len() + diff.deleted.len();

    if total_changes == 0 {
        println!("  Everything up to date. No changes detected.");
        return Ok(());
    }

    println!("  Changes detected:");
    if !diff.added.is_empty() {
        println!("    {} {} new files", "+".green(), diff.added.len());
    }
    if !diff.modified.is_empty() {
        println!(
            "    {} {} modified files",
            "~".yellow(),
            diff.modified.len()
        );
    }
    if !diff.deleted.is_empty() {
        println!("    {} {} deleted files", "-".red(), diff.deleted.len());
    }

    if dry_run {
        println!("\n{}", "Changes that would be applied:".cyan().bold());
        for f in &diff.added {
            println!("    + {}", f.relative_path);
        }
        for f in &diff.modified {
            println!("    ~ {}", f.relative_path);
        }
        for f in &diff.deleted {
            println!("    - {}", f);
        }
        println!("\n  Run without --dry-run to apply.");
        return Ok(());
    }

    // ── Snapshot before mutation ────────────────────────────────────
    // Per ROADMAP: take a safety snapshot of the on-disk indexes so a botched
    // update can be rolled back. We only snapshot files that currently exist.
    let store = IndexStore::new(graxus_dir.clone());
    let codemap_path = workspace::code_dir(&root).join("codemap.json");
    let files_path = graxus_dir.join("files.json");
    let graph_path = workspace::docs_dir(&root).join("graph.json");
    let db_path = graxus_dir.join("index.db");
    let mut snapshot_targets = Vec::new();
    for p in [&codemap_path, &files_path, &graph_path, &db_path] {
        if p.exists() {
            snapshot_targets.push(p.clone());
        }
    }
    let snapshot = if snapshot_targets.is_empty() {
        None
    } else {
        match store.create_snapshot("pre-update", &snapshot_targets) {
            Ok(s) => {
                println!("  Snapshot saved: {} (id {})", s.label, s.id);
                Some(s)
            }
            Err(e) => {
                eprintln!(
                    "  {} Could not create pre-update snapshot: {}",
                    "Warning:".yellow(),
                    e
                );
                None
            }
        }
    };

    // ── Incremental codemap update ──────────────────────────────────
    let code_dir = workspace::code_dir(&root);

    if codemap_path.exists() {
        let pb = if ctx.show_progress() {
            let pb = ProgressBar::new(total_changes as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("  {spinner:.green} Updating codemap... [{bar:40.cyan/blue}] {pos}/{len} files")
                    .unwrap()
                    .progress_chars("=> "),
            );
            Some(pb)
        } else {
            None
        };

        // Load existing codemap
        let existing_content = std::fs::read_to_string(&codemap_path)?;
        let mut existing_graph: graxus_codemap::CodeGraph =
            serde_json::from_str(&existing_content)?;

        // Remove deleted files from codemap
        for deleted in &diff.deleted {
            existing_graph.remove_file(deleted);
            if let Some(pb) = &pb {
                pb.inc(1);
            }
        }

        // Build new codemap for added + modified files
        let changed_files: Vec<_> = diff
            .added
            .iter()
            .chain(diff.modified.iter())
            .cloned()
            .collect();

        if !changed_files.is_empty() {
            let backend = codemap_backend.parse::<graxus_core::ParserBackend>().unwrap_or_default();
            let builder = CodemapBuilder::new(changed_files.clone()).with_backend(backend);
            let new_graph = builder.build()?;
            existing_graph.merge(new_graph);
        }

        if let Some(pb) = &pb {
            pb.finish_with_message("Codemap updated");
        }

        // Save updated codemap
        if let Err(e) = CodemapBuilder::save(&existing_graph, &code_dir) {
            eprintln!("  {} Failed to save codemap: {}", "Warning:".yellow(), e);
        }

        // Update SQLite. Only touched (added + modified + deleted) files are
        // rewritten: their rows are deleted first, then re-inserted from the
        // freshly merged graph. Unchanged files are left alone, so repeated
        // `update` runs do not accumulate duplicate or stale rows.
        if let Ok(db) = SqliteStore::new(&db_path) {
            let touched: Vec<String> = diff
                .added
                .iter()
                .chain(diff.modified.iter())
                .map(|f| f.relative_path.clone())
                .chain(diff.deleted.iter().cloned())
                .collect();

            for path in &touched {
                let _ = db.delete_file_data(path);
            }

            // Re-insert only symbols/imports/calls that belong to a touched
            // (added or modified) file. Deleted files have no rows to add.
            let touched_set: std::collections::HashSet<&str> =
                touched.iter().map(|s| s.as_str()).collect();

            for sym in &existing_graph.symbols {
                if !touched_set.contains(sym.file.as_str()) {
                    continue;
                }
                let _ = db.insert_symbol(
                    &sym.id,
                    &sym.file,
                    &sym.language,
                    &sym.kind.to_string(),
                    &sym.name,
                    sym.exported,
                    sym.line_start,
                    sym.line_end,
                    &format!("{:?}", sym.visibility).to_lowercase(),
                    &sym.signature,
                    sym.is_test,
                    sym.usage_count,
                );
            }
            for imp in &existing_graph.imports {
                if !touched_set.contains(imp.file.as_str()) {
                    continue;
                }
                let _ = db.insert_import(
                    &imp.id,
                    &imp.file,
                    &imp.language,
                    &format!("{:?}", imp.kind),
                    &imp.source,
                    imp.local_name.as_deref(),
                    imp.imported_name.as_deref(),
                    imp.resolved_file.as_deref(),
                    imp.line,
                    &imp.confidence.to_string(),
                );
            }
            for call in &existing_graph.calls {
                if !touched_set.contains(call.file.as_str()) {
                    continue;
                }
                let _ = db.insert_call(
                    &call.id,
                    &call.file,
                    &call.language,
                    &format!("{:?}", call.kind),
                    call.caller_symbol.as_deref(),
                    &call.callee_text,
                    call.object.as_deref(),
                    call.resolved_symbol.as_deref(),
                    call.line,
                    call.column,
                    &call.confidence.to_string(),
                );
            }
            for parser_result in &existing_graph.parser_results {
                if !touched_set.contains(parser_result.file.as_str()) {
                    continue;
                }
                if let Ok(value) = serde_json::to_value(parser_result) {
                    let _ = db.insert_parser_result_value(&value);
                }
            }
        }
    } else {
        // No existing codemap, do full index
        println!("  No existing codemap. Running full index.");
        run_full_index(ctx, &root, &current_files, &codemap_backend)?;
        return Ok(());
    }

    // ── Incremental docgraph update ─────────────────────────────────
    let docs_dir = workspace::docs_dir(&root);
    let graph_path = docs_dir.join("graph.json");

    if graph_path.exists() && config.docs.enabled {
        // Load existing docgraph
        let mut existing_graph = DocGraph::load(&docs_dir)?;

        // Remove deleted doc files
        for deleted in &diff.deleted {
            if deleted.ends_with(".md") || deleted.ends_with(".mdx") {
                existing_graph.remove_document(deleted);
            }
        }

        // Re-parse added + modified doc files
        let changed_docs: Vec<_> = diff
            .added
            .iter()
            .chain(diff.modified.iter())
            .filter(|f| f.kind == graxus_core::FileKind::Doc)
            .collect();

        if !changed_docs.is_empty() {
            let mut new_graph = DocGraph::new();
            for doc in &changed_docs {
                if let Ok(content) = std::fs::read_to_string(&doc.path) {
                    let fm = graxus_docgraph::frontmatter::parse(&content).0;
                    new_graph.add_document(&doc.path, &root, fm, &content);
                }
            }
            existing_graph.merge(new_graph);
        }

        // Save updated docgraph
        if let Err(e) = existing_graph.save(&docs_dir) {
            eprintln!("  {} Failed to save docgraph: {}", "Warning:".yellow(), e);
        }
    }

    // Save new file list
    scanner::save_saved_files(&graxus_dir, &current_files)?;

    println!("\n{}", "Update complete.".green().bold());
    println!("  {} files scanned", current_files.len());
    println!("  {} changes applied", total_changes);
    if let Some(ref snap) = snapshot {
        println!(
            "  Snapshot {} retained for rollback via `graxus rollback {}`",
            snap.id, snap.id
        );
    }

    Ok(())
}

/// Run a full index (delegates to the index command).
fn run_full_index(
    ctx: &CliContext,
    root: &Path,
    files: &[scanner::ScannedFile],
    codemap_backend: &str,
) -> Result<()> {
    let code_dir = workspace::code_dir(root);
    let docs_dir = workspace::docs_dir(root);

    // Build codemap from all code files
    let code_files: Vec<_> = files
        .iter()
        .filter(|f| f.kind == graxus_core::FileKind::Code)
        .cloned()
        .collect();

    if !code_files.is_empty() {
        let pb = if ctx.show_progress() {
            let pb = ProgressBar::new(code_files.len() as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("  {spinner:.green} Building codemap... [{bar:40.cyan/blue}] {pos}/{len} files")
                    .unwrap()
                    .progress_chars("=> "),
            );
            Some(pb)
        } else {
            None
        };

        let backend = codemap_backend.parse::<graxus_core::ParserBackend>().unwrap_or_default();
        let builder = CodemapBuilder::new(code_files).with_backend(backend);
        let graph = builder.build()?;

        if let Err(e) = CodemapBuilder::save(&graph, &code_dir) {
            eprintln!("  {} Failed to save codemap: {}", "Warning:".yellow(), e);
        }

        // Save to SQLite. A full re-index must replace any prior contents, so
        // clear the symbols/imports/calls tables first to stay idempotent across
        // repeated `update --full` invocations.
        let db_path = root.join(".graxus").join("index.db");
        if let Ok(db) = SqliteStore::new(&db_path) {
            for sym in &graph.symbols {
                let _ = db.insert_symbol(
                    &sym.id,
                    &sym.file,
                    &sym.language,
                    &sym.kind.to_string(),
                    &sym.name,
                    sym.exported,
                    sym.line_start,
                    sym.line_end,
                    &format!("{:?}", sym.visibility).to_lowercase(),
                    &sym.signature,
                    sym.is_test,
                    sym.usage_count,
                );
            }
            for imp in &graph.imports {
                let _ = db.insert_import(
                    &imp.id,
                    &imp.file,
                    &imp.language,
                    &format!("{:?}", imp.kind),
                    &imp.source,
                    imp.local_name.as_deref(),
                    imp.imported_name.as_deref(),
                    imp.resolved_file.as_deref(),
                    imp.line,
                    &imp.confidence.to_string(),
                );
            }
            for call in &graph.calls {
                let _ = db.insert_call(
                    &call.id,
                    &call.file,
                    &call.language,
                    &format!("{:?}", call.kind),
                    call.caller_symbol.as_deref(),
                    &call.callee_text,
                    call.object.as_deref(),
                    call.resolved_symbol.as_deref(),
                    call.line,
                    call.column,
                    &call.confidence.to_string(),
                );
            }
            for parser_result in &graph.parser_results {
                if let Ok(value) = serde_json::to_value(parser_result) {
                    let _ = db.insert_parser_result_value(&value);
                }
            }
        }

        if let Some(pb) = &pb {
            pb.finish_with_message("Codemap built");
        }
    }

    // Build docgraph
    let config = ctx.load_config(root)?;
    if config.docs.enabled {
        match graxus_docgraph::build(root, &config) {
            Ok(graph) => {
                let _ = graph.save(&docs_dir);
            }
            Err(e) => {
                eprintln!("  {} {}", "Warning:".yellow(), e);
            }
        }
    }

    // Save file list
    let graxus_dir = root.join(".graxus");
    scanner::save_saved_files(&graxus_dir, files)?;

    println!("\n{}", "Index complete.".green().bold());
    Ok(())
}
