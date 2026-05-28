use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use graxus_core::workspace;

pub fn run(debounce: u64) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    println!("{}", "=== Watch Mode ===".green().bold());
    println!("  Watching for changes in: {}", root.display());
    println!("  Debounce: {}s", debounce);
    println!("  Press Ctrl+C to stop");
    println!();

    // Use notify crate for filesystem events
    use notify::{Event, RecursiveMode, Watcher};

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;

    watcher.watch(&root, RecursiveMode::Recursive)?;

    let debounce_duration = Duration::from_secs(debounce);
    let mut last_event = std::time::Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(event)) => {
                // Filter for relevant file changes
                let relevant = event.paths.iter().any(|p| {
                    let path_str = p.to_string_lossy();
                    !path_str.contains(".graxus/")
                        && !path_str.contains("/target/")
                        && !path_str.contains("/node_modules/")
                        && !path_str.contains("/.git/")
                        && (path_str.ends_with(".rs")
                            || path_str.ends_with(".ts")
                            || path_str.ends_with(".tsx")
                            || path_str.ends_with(".js")
                            || path_str.ends_with(".jsx")
                            || path_str.ends_with(".go")
                            || path_str.ends_with(".py")
                            || path_str.ends_with(".md")
                            || path_str.ends_with(".mdx"))
                });

                if relevant && last_event.elapsed() >= debounce_duration {
                    last_event = std::time::Instant::now();
                    println!(
                        "\n{} Change detected, re-indexing...",
                        "⟳".cyan()
                    );

                    match super::index::run() {
                        Ok(_) => {
                            println!("{} Re-index complete.", "✓".green());
                        }
                        Err(e) => {
                            eprintln!("{} Re-index failed: {}", "✗".red(), e);
                        }
                    }

                    println!(
                        "\n  {} Watching for changes... (Ctrl+C to stop)",
                        "👁".cyan()
                    );
                }
            }
            Ok(Err(e)) => {
                eprintln!("Watch error: {}", e);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No events, continue
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}
