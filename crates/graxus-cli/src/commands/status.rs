use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::Path;

use graxus_core::{config::GraxusConfig, workspace};

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let config = GraxusConfig::load(&root)?;

    println!("{}", "Graxus Project Status".green().bold());
    println!("  Name:    {}", config.project.name);
    println!("  Root:    {}", root.display());
    println!("  .graxus: {}", root.join(".graxus").display());

    // Count files in .graxus subdirs
    let graxus_dir = root.join(".graxus");
    for subdir in &["docs", "code", "snapshots", "logs", "reports"] {
        let dir = graxus_dir.join(subdir);
        let count = if dir.is_dir() {
            std::fs::read_dir(&dir)
                .map(|entries| entries.count())
                .unwrap_or(0)
        } else {
            0
        };
        println!("  .graxus/{}: {} files", subdir, count);
    }

    // Config summary
    println!("\n{}", "Config:".green().bold());
    println!("  Docs enabled:     {}", config.docs.enabled);
    println!("  Code enabled:     {}", config.code.enabled);
    println!("  Code parser:      {}", config.code.parser);
    println!("  Code languages:   {}", config.code.languages.join(", "));
    println!("  Index storage:    {}", config.index.storage);
    println!("  Edit snapshots:   {}", config.edit.create_snapshots);
    println!("  Edit max files:   {}", config.edit.max_files_per_operation);

    Ok(())
}
