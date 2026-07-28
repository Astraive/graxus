//! CLI stdout rendering and JSON output helper utilities.

use colored::Colorize;
use serde::Serialize;

/// Prints a standard success message in green.
pub fn success(msg: &str) {
    println!("{}", msg.green());
}

/// Prints a bold header in green.
pub fn header(msg: &str) {
    println!("{}", msg.green().bold());
}

/// Prints an info message in cyan.
pub fn info(msg: &str) {
    println!("{}", msg.cyan());
}

/// Prints a warning message in yellow.
pub fn warn(msg: &str) {
    println!("{}", msg.yellow());
}

/// Prints a failure message in red.
pub fn error(msg: &str) {
    eprintln!("{}", msg.red());
}

/// Formats and pretty prints Serialize objects as JSON.
pub fn print_json<T: Serialize>(val: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(val)?);
    Ok(())
}
