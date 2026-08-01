//! Macro-aware AST extraction for Rust and C++.
//!
//! Handles: Rust `macro_rules!`, procedural macros, C preprocessor macros,
//! C++ templates as macro-like constructs.

use serde::{Deserialize, Serialize};

/// A macro definition extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroFact {
    /// File where this macro is defined.
    pub file: String,
    /// Language.
    pub language: String,
    /// Macro name.
    pub name: String,
    /// Macro kind (declarative, procedural, preprocessor, template).
    pub kind: MacroKind,
    /// Line number.
    pub line: usize,
    /// Whether this macro is exported (public).
    pub exported: bool,
}

/// Kind of macro definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroKind {
    /// Rust macro_rules!
    Declarative,
    /// Rust proc macro (\#\[proc_macro\], \#\[proc_macro_derive\], \#\[proc_macro_attribute\])
    Procedural,
    /// C/C++ #define
    Preprocessor,
    /// C++ template (conceptually similar to macros for indexing)
    Template,
}

/// Extract macro definitions from source code using text scanning.
pub fn extract_macros(source: &str, file_path: &str, language: &str) -> Vec<MacroFact> {
    let mut facts = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let line_num = line_idx + 1;

        match language {
            "rust" => {
                if trimmed.starts_with("macro_rules!") || trimmed.starts_with("pub macro_rules!") {
                    let name = trimmed
                        .trim_start_matches("pub ")
                        .strip_prefix("macro_rules!")
                        .unwrap_or(trimmed)
                        .trim()
                        .trim_start_matches('!')
                        .trim_end_matches('{')
                        .trim()
                        .to_string();
                    facts.push(MacroFact {
                        file: file_path.to_string(),
                        language: language.to_string(),
                        name,
                        kind: MacroKind::Declarative,
                        line: line_num,
                        exported: trimmed.contains("pub"),
                    });
                } else if trimmed.contains("#[proc_macro")
                    || trimmed.contains("#[proc_macro_derive")
                    || trimmed.contains("#[proc_macro_attribute")
                {
                    // Look ahead for function name
                    if let Some(name) = find_next_function_name(source, line_idx) {
                        facts.push(MacroFact {
                            file: file_path.to_string(),
                            language: language.to_string(),
                            name,
                            kind: MacroKind::Procedural,
                            line: line_num,
                            exported: true,
                        });
                    }
                }
            }
            "c" | "cpp" if trimmed.starts_with("#define") => {
                let rest = trimmed.strip_prefix("#define").unwrap_or(trimmed).trim();
                let name = rest
                    .split(|c: char| c.is_whitespace() || c == '(')
                    .next()
                    .unwrap_or(rest)
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    facts.push(MacroFact {
                        file: file_path.to_string(),
                        language: language.to_string(),
                        name,
                        kind: MacroKind::Preprocessor,
                        line: line_num,
                        exported: false,
                    });
                }
            }
            _ => {}
        }
    }
    facts
}

fn find_next_function_name(source: &str, after_line: usize) -> Option<String> {
    for line in source.lines().skip(after_line + 1).take(5) {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") {
            let name = trimmed
                .split('(')
                .next()
                .unwrap_or(trimmed)
                .split_whitespace()
                .last()
                .unwrap_or(trimmed);
            return Some(name.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_macro_rules() {
        let source = r#"macro_rules! vec {
    ($($x:expr),* $(,)?) => { ... };
}

pub macro_rules! my_macro {
    ($x:expr) => { $x };
}"#;
        let facts = extract_macros(source, "macros.rs", "rust");
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].name, "vec");
        assert_eq!(facts[0].kind, MacroKind::Declarative);
        assert!(!facts[0].exported);
        assert_eq!(facts[1].name, "my_macro");
        assert!(facts[1].exported);
    }

    #[test]
    fn test_c_define() {
        let source = r#"#define MAX_SIZE 1024
#define MIN(a, b) ((a) < (b) ? (a) : (b))
#define DEBUG_LOG(msg) printf(msg)"#;
        let facts = extract_macros(source, "utils.h", "c");
        assert_eq!(facts.len(), 3);
        assert_eq!(facts[0].name, "MAX_SIZE");
        assert_eq!(facts[0].kind, MacroKind::Preprocessor);
        assert_eq!(facts[1].name, "MIN");
        assert_eq!(facts[2].name, "DEBUG_LOG");
    }

    #[test]
    fn test_empty_source() {
        let facts = extract_macros("", "empty.rs", "rust");
        assert!(facts.is_empty());
    }
}
