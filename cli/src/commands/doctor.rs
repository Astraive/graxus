use anyhow::Result;
use colored::Colorize;
use std::path::Path;

use crate::context::CliContext;
use graxus_core::config::GraxusConfig;

/// Run health diagnostics on the graxus project.
///
/// # Arguments
/// * `_json` - Output as JSON
/// * `_strict` - Fail on warnings instead of only errors
pub fn run(ctx: &CliContext, _json: bool, _strict: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

    println!("{}", "=== Graxus Health Diagnostics ===".green().bold());
    println!();

    // 1. Check .graxus/ exists
    let graxus_dir = root.join(".graxus");
    if graxus_dir.exists() {
        println!("  {} .graxus directory exists", "✓".green());
    } else {
        println!(
            "  {} .graxus directory missing — run `graxus init`",
            "✗".red()
        );
        return Ok(());
    }

    // 2. Load config
    match GraxusConfig::load(&root) {
        Ok(config) => {
            println!("  {} Config loaded successfully", "✓".green());
            println!("    Project: {}", config.project.name);
            println!("    Storage: {}", config.index.storage);
        }
        Err(e) => {
            println!("  {} Config error: {}", "✗".red(), e);
            return Ok(());
        }
    }

    // 3. Check index freshness
    let files_json = graxus_dir.join("files.json");
    if files_json.exists() {
        let metadata = std::fs::metadata(&files_json)?;
        let modified = metadata.modified()?;
        let age = std::time::SystemTime::now().duration_since(modified)?;
        let age_hours = age.as_secs() / 3600;

        if age_hours > 24 {
            println!(
                "  {} Index is {} hours old — consider `graxus index`",
                "⚠".yellow(),
                age_hours
            );
        } else {
            println!("  {} Index is {} hours old", "✓".green(), age_hours);
        }
    } else {
        println!("  {} No index found — run `graxus index`", "⚠".yellow());
    }

    // 4. Count files by language
    let code_dir = graxus_dir.join("code");
    let codemap_path = code_dir.join("codemap.json");
    if codemap_path.exists() {
        let content = std::fs::read_to_string(&codemap_path)?;
        let codemap: serde_json::Value = serde_json::from_str(&content)?;

        if let Some(files) = codemap.get("files").and_then(|f| f.as_array()) {
            let mut lang_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for f in files {
                let lang = f
                    .get("language")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                *lang_counts.entry(lang.to_string()).or_insert(0) += 1;
            }
            println!("  {} {} files indexed", "✓".green(), files.len());
            for (lang, count) in &lang_counts {
                println!("    {}: {}", lang, count);
            }
        }

        if let Some(symbols) = codemap.get("symbols").and_then(|f| f.as_array()) {
            println!("  {} {} symbols extracted", "✓".green(), symbols.len());
        }

        if let Some(imports) = codemap.get("imports").and_then(|f| f.as_array()) {
            let total = imports.len();
            let resolved = imports
                .iter()
                .filter(|i| {
                    i.get("resolved_file")
                        .and_then(|v| v.as_str())
                        .map(|s| !s.is_empty())
                        .unwrap_or(false)
                })
                .count();
            let pct = if total > 0 {
                (resolved as f64 / total as f64) * 100.0
            } else {
                100.0
            };

            if pct < 80.0 {
                println!(
                    "  {} Import resolution: {:.0}% ({}/{} resolved)",
                    "⚠".yellow(),
                    pct,
                    resolved,
                    total
                );
            } else {
                println!(
                    "  {} Import resolution: {:.0}% ({}/{} resolved)",
                    "✓".green(),
                    pct,
                    resolved,
                    total
                );
            }
        }
    } else {
        println!("  {} No codemap found — run `graxus index`", "⚠".yellow());
    }

    // 5. Docs graph
    let docs_dir = graxus_dir.join("docs");
    let graph_path = docs_dir.join("graph.json");
    if graph_path.exists() {
        let content = std::fs::read_to_string(&graph_path)?;
        let graph: serde_json::Value = serde_json::from_str(&content)?;
        let nodes = graph
            .get("nodes")
            .and_then(|n| n.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let edges = graph
            .get("edges")
            .and_then(|e| e.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        println!(
            "  {} Docs graph: {} nodes, {} edges",
            "✓".green(),
            nodes,
            edges
        );
    } else {
        println!("  {} No docs graph found", "⚠".yellow());
    }

    // 6. Storage stats
    let total_size = dir_size(&graxus_dir).unwrap_or(0);
    let size_str = if total_size > 1_000_000 {
        format!("{:.1} MB", total_size as f64 / 1_000_000.0)
    } else {
        format!("{:.1} KB", total_size as f64 / 1_000.0)
    };
    println!("  {} Storage: {}", "✓".green(), size_str);

    // 7. Snapshots
    let snapshots_dir = graxus_dir.join("snapshots");
    if snapshots_dir.exists() {
        let count = std::fs::read_dir(&snapshots_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .count();
        if count > 0 {
            println!("  {} {} snapshots available", "✓".green(), count);
        }
    }

    println!();
    println!("{}", "Diagnostics complete.".green().bold());
    Ok(())
}

fn dir_size(path: &Path) -> std::io::Result<u64> {
    let mut size = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.path().is_dir() {
                size += dir_size(&entry.path())?;
            } else {
                size += entry.metadata()?.len();
            }
        }
    }
    Ok(size)
}
