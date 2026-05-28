use anyhow::{Context, Result};
use colored::Colorize;
use std::env;

use graxus_core::workspace;

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    println!("{}", "=== Graxus JSON-RPC Server ===".green().bold());
    println!("  Root: {}", root.display());
    println!("  Protocol: JSON-RPC 2.0 over stdio");
    println!();
    println!("  Supported methods:");
    println!("    ping              - Returns pong");
    println!("    status            - Project stats");
    println!("    context           - Text search across docs+code");
    println!("    file_context      - All context for a file");
    println!("    symbol_context    - All context for a symbol");
    println!("    update            - Reload graphs from disk");
    println!();
    println!("  {}", "Listening on stdin... (Ctrl+C to stop)".cyan());

    // Create a tokio runtime for the async server
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(graxus_server::run_stdio(root))?;

    Ok(())
}

use std::path::Path;
