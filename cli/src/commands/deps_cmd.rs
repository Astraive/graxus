use anyhow::Result;
use colored::Colorize;

use crate::context::CliContext;
use graxus_core::dependencies;

/// List detected dependencies in the project.
///
/// # Arguments
/// * `_json` - Output as JSON
pub fn run(ctx: &CliContext, _json: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

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
