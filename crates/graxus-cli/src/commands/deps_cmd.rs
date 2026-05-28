use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::Path;

use graxus_core::{workspace, dependencies};

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let deps = dependencies::detect_dependencies(&root);

    println!("{}", "=== Dependencies ===".green().bold());
    if deps.is_empty() {
        println!("  No dependencies detected.");
    } else {
        for dep in &deps {
            let version = dep.version.as_deref().unwrap_or("*");
            let kind_str = match dep.kind {
                dependencies::DependencyKind::Runtime => "",
                dependencies::DependencyKind::Dev => " (dev)",
                dependencies::DependencyKind::Build => " (build)",
            };
            println!("  {} v{} ({:?}{})", dep.name, version, dep.source, kind_str);
        }
    }
    println!("\n  Total: {} dependencies", deps.len());

    Ok(())
}
