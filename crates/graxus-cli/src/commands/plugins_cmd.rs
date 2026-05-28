use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::Path;

use graxus_core::plugins::PluginRegistry;
use graxus_core::workspace;

pub fn run_list() -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let plugin_dir = root.join(".graxus").join("plugins");
    let mut registry = PluginRegistry::new(plugin_dir);
    registry.discover()?;

    println!("{}", "=== Installed Plugins ===".green().bold());
    if registry.list().is_empty() {
        println!("  No plugins installed.");
        println!("  Install: graxus plugins install <path>");
    } else {
        for plugin in registry.list() {
            println!(
                "  {} v{} — {}",
                plugin.name.cyan(),
                plugin.version,
                plugin.description
            );
            println!("    Type: {:?}", plugin.plugin_type);
            println!("    Entry: {}", plugin.entry_point);
        }
    }
    println!("\n  Total: {} plugins", registry.list().len());

    Ok(())
}

pub fn run_install(path: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let plugin_dir = root.join(".graxus").join("plugins");
    std::fs::create_dir_all(&plugin_dir)?;
    let mut registry = PluginRegistry::new(plugin_dir);
    registry.discover()?;

    let source = Path::new(path);
    registry.install(source)?;

    println!("{}", "Plugin installed!".green().bold());
    println!("  Source: {}", path);

    Ok(())
}

pub fn run_uninstall(name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let plugin_dir = root.join(".graxus").join("plugins");
    let mut registry = PluginRegistry::new(plugin_dir);
    registry.discover()?;

    if registry.get(name).is_none() {
        println!("{} Plugin '{}' not found.", "Error:".red(), name);
        return Ok(());
    }

    registry.uninstall(name)?;

    println!("{}", "Plugin uninstalled!".green().bold());
    println!("  Name: {}", name);

    Ok(())
}
