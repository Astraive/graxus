/// count-symbols: A reference graxus plugin.
///
/// Reads a PluginContext from stdin (JSON), counts symbols in the codemap file,
/// and outputs a PluginResult to stdout (JSON).
///
/// Build: rustc main.rs -o count-symbols
/// Or use: cargo build --release (requires Cargo.toml)

use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).expect("Failed to read stdin");

    let ctx: serde_json::Value = serde_json::from_str(&input).expect("Invalid JSON on stdin");

    let codemap_path = ctx.get("codemap_path").and_then(|v| v.as_str());

    let (symbol_count, message) = match codemap_path {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(codemap) => {
                    let count = codemap
                        .get("symbols")
                        .and_then(|s| s.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    (count, format!("Found {} symbols in {}", count, path))
                }
                Err(e) => (0, format!("Failed to parse codemap JSON: {}", e)),
            },
            Err(e) => (0, format!("Failed to read codemap file: {}", e)),
        },
        None => (0, "No codemap path provided".to_string()),
    };

    let result = serde_json::json!({
        "success": true,
        "output": {
            "symbol_count": symbol_count
        },
        "files_modified": [],
        "message": message
    });

    println!("{}", serde_json::to_string(&result).unwrap());
}
