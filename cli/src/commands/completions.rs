use anyhow::Result;
use clap::CommandFactory;
use colored::Colorize;
use std::path::Path;

use crate::context::CliContext;
use crate::Cli;

/// Generate shell completions for bash, zsh, fish, or powershell.
///
/// # Arguments
/// * `shell` - Shell type: "bash", "zsh", "fish", "powershell"
/// * `output` - Optional output file path (stdout if omitted)
pub fn run(ctx: &CliContext, shell: &str, output: Option<&str>) -> Result<()> {
    let _ = ctx;
    let mut cmd = Cli::command();

    let shell_type = match shell.to_lowercase().as_str() {
        "bash" => clap_complete::Shell::Bash,
        "zsh" => clap_complete::Shell::Zsh,
        "fish" => clap_complete::Shell::Fish,
        "powershell" | "pwsh" => clap_complete::Shell::PowerShell,
        _ => anyhow::bail!(
            "Unknown shell: {}. Use bash, zsh, fish, or powershell",
            shell
        ),
    };

    let mut buf = Vec::new();
    clap_complete::generate(shell_type, &mut cmd, "graxus", &mut buf);
    let content = String::from_utf8(buf)?;

    match output {
        Some(path) => {
            if let Some(parent) = Path::new(path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, &content)?;
            println!("  {} {}", "Saved:".green(), path);
        }
        None => {
            print!("{}", content);
        }
    }

    Ok(())
}
