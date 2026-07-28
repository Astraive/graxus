use anyhow::Result;
use colored::Colorize;
use std::path::Path;

use crate::context::CliContext;
use graxus_core::plugins::PluginRegistry;

/// List all installed plugins.
pub fn run_list(ctx: &CliContext) -> Result<()> {
    let root = ctx.resolve_root()?;

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

/// Install a plugin from a local path.
///
/// # Arguments
/// * `path` - Path to the plugin directory
pub fn run_install(ctx: &CliContext, path: &str) -> Result<()> {
    let root = ctx.resolve_root()?;

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

/// Uninstall a plugin by name.
///
/// # Arguments
/// * `name` - Name of the plugin to uninstall
pub fn run_uninstall(ctx: &CliContext, name: &str) -> Result<()> {
    let root = ctx.resolve_root()?;

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
