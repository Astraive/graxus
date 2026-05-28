use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::Path;

use graxus_core::{workspace, workspaces};

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

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

    Ok(())
}
