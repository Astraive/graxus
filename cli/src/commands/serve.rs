use anyhow::Result;
use colored::Colorize;

use crate::context::CliContext;

/// Start the JSON-RPC server for AI tool integration over stdio.
pub fn run(ctx: &CliContext) -> Result<()> {
    let root = ctx.resolve_root()?;

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

/// Start the LSP server for editor integration over stdio.
pub fn run_lsp(ctx: &CliContext) -> Result<()> {
    let root = ctx.resolve_root()?;

    tracing::info!("Starting LSP server, root: {}", root.display());

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(graxus_server::run_lsp(root))?;

    Ok(())
}
