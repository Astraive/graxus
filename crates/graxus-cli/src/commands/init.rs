use anyhow::Result;
use colored::Colorize;
use std::env;
use std::path::Path;

use graxus_core::workspace;

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let root = Path::new(&cwd);

    if workspace::find_root(root).is_some() {
        println!("{}", "Project already initialized (graxus.yaml or .graxus/ found)".yellow());
        return Ok(());
    }

    let config = workspace::init_project(root)?;
    println!("{}", "Initialized graxus project".green().bold());
    println!("  Name: {}", config.project.name);
    println!("  Root: {}", root.display());
    println!("  .graxus/ created with subdirs: docs/, code/, snapshots/, logs/, reports/");
    println!("  graxus.yaml created with defaults");
    Ok(())
}
