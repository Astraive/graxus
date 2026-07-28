use anyhow::Result;
use colored::Colorize;
use std::env;
use std::path::Path;

use graxus_core::workspace;

use crate::context::CliContext;

/// Initialize a new graxus project at the given path.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `path` - Project root directory (defaults to current directory)
/// * `name` - Optional project name override
/// * `force` - If true, overwrite existing graxus.yaml
/// * `minimal` - If true, create a minimal configuration
pub fn run(
    ctx: &CliContext,
    path: &Path,
    _name: Option<&str>,
    force: bool,
    _minimal: bool,
) -> Result<()> {
    // `init` intentionally ignores `--root`/`--config` since it bootstraps a new
    // project at the given path rather than operating on an existing one.
    let _ = ctx;
    let root = if path.as_os_str() == "." {
        env::current_dir()?
    } else {
        path.to_path_buf()
    };

    if workspace::find_root(&root).is_some() && !force {
        println!(
            "{}",
            "Project already initialized (graxus.yaml or .graxus/ found)".yellow()
        );
        return Ok(());
    }

    let config = workspace::init_project(&root)?;
    println!("{}", "Initialized graxus project".green().bold());
    println!("  Name: {}", config.project.name);
    println!("  Root: {}", root.display());
    println!("  .graxus/ created with subdirs: docs/, code/, snapshots/, logs/, reports/");
    println!("  graxus.yaml created with defaults");
    Ok(())
}
