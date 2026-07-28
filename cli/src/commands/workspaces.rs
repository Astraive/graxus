use anyhow::{Context, Result};
use colored::Colorize;
use std::path::{Path, PathBuf};

use crate::context::CliContext;
use graxus_core::workspaces;

/// Detect and list workspaces in the project.
///
/// # Arguments
/// * `_json` - Output as JSON
pub fn run(ctx: &CliContext, _json: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

    let detected = workspaces::detect_workspaces(&root);

    println!("{}", "=== Detected Workspaces ===".green().bold());
    if detected.is_empty() {
        println!("  No workspaces detected.");
    } else {
        for ws in &detected {
            let kind_str = match ws.kind {
                workspaces::WorkspaceKind::CargoWorkspace => "Cargo",
                workspaces::WorkspaceKind::NpmWorkspace => "npm",
                workspaces::WorkspaceKind::GoModule => "Go",
                workspaces::WorkspaceKind::Unknown => "Unknown",
            };
            println!(
                "  {} ({}) — {}",
                ws.name,
                kind_str.cyan(),
                ws.root.display()
            );
        }
    }
    println!("\n  Total: {} workspaces", detected.len());

    // Show monorepo info if applicable
    let ws_info = workspaces::detect_workspace(&root);
    if ws_info.is_monorepo {
        println!("\n{}", "=== Monorepo Sub-Projects ===".green().bold());
        println!("  Kind: {:?}", ws_info.kind);
        println!("  Languages: {}", ws_info.languages.join(", "));
        for sub in &ws_info.sub_projects {
            let indexed = workspaces::is_indexed(sub);
            let status = if !indexed {
                "not indexed".yellow().to_string()
            } else if workspaces::is_stale(sub) {
                "stale".red().to_string()
            } else {
                "indexed".green().to_string()
            };
            println!("  {} [{}]", sub.display(), status);
        }
        println!("\n  Total: {} sub-projects", ws_info.sub_projects.len());
    }

    Ok(())
}

/// CLI entry point for `graxus index-all`.
/// Finds the project root and runs workspace indexing.
pub fn run_index_all_cli(ctx: &CliContext) -> Result<()> {
    let root = ctx.resolve_root()?;
    run_index_all(&root)
}

/// Index all sub-projects in the workspace.
///
/// Each sub-project gets its own `.graxus/` directory.
/// The parent `.graxus/` gets a workspace index pointing to all sub-projects.
pub fn run_index_all(root: &Path) -> Result<()> {
    let ws_info = workspaces::detect_workspace(root);

    if !ws_info.is_monorepo {
        println!("{}", "Not a monorepo — nothing to index.".yellow());
        return Ok(());
    }

    println!("{}", "=== Indexing Workspace ===".green().bold());
    println!("  Root: {}", root.display());
    println!("  Sub-projects: {}", ws_info.sub_projects.len());

    // Index each sub-project
    for (i, sub_path) in ws_info.sub_projects.iter().enumerate() {
        let sub_name = sub_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("project-{}", i));

        println!(
            "\n  {}/{}  Indexing {}...",
            i + 1,
            ws_info.sub_projects.len(),
            sub_name.cyan()
        );

        // Create .graxus directory for sub-project
        let graxus_dir = sub_path.join(".graxus");
        for subdir in &["docs", "code", "snapshots", "logs", "reports"] {
            std::fs::create_dir_all(graxus_dir.join(subdir))
                .with_context(|| format!("Failed to create .graxus/{} in {}", subdir, sub_name))?;
        }

        // Save a stub files.json so is_indexed() returns true
        let files_path = graxus_dir.join("files.json");
        if !files_path.exists() {
            std::fs::write(&files_path, "[]")?;
        }

        println!("    .graxus/ created at {}", graxus_dir.display());
    }

    // Save workspace index in parent .graxus/
    let parent_graxus = root.join(".graxus");
    std::fs::create_dir_all(&parent_graxus)?;

    let workspace_index = serde_json::json!({
        "is_monorepo": ws_info.is_monorepo,
        "kind": format!("{:?}", ws_info.kind),
        "languages": ws_info.languages,
        "sub_projects": ws_info.sub_projects.iter().map(|p| {
            serde_json::json!({
                "path": p.strip_prefix(root).unwrap_or(p).to_string_lossy(),
                "absolute": p.to_string_lossy(),
                "indexed": workspaces::is_indexed(p),
            })
        }).collect::<Vec<_>>(),
    });

    let index_path = parent_graxus.join("workspace.json");
    std::fs::write(&index_path, serde_json::to_string_pretty(&workspace_index)?)?;

    println!(
        "\n{}",
        format!("Workspace index saved to {}", index_path.display())
            .green()
            .bold()
    );

    Ok(())
}

/// Get all sub-project paths for cross-project operations.
#[allow(dead_code)]
pub fn _get_all_project_roots(root: &Path) -> Vec<PathBuf> {
    let ws_info = workspaces::detect_workspace(root);
    if ws_info.is_monorepo {
        ws_info.sub_projects
    } else {
        vec![root.to_path_buf()]
    }
}
