//! Extractor for decorators, attributes, and annotations across languages.
//!
//! Handles: Python decorators (@app.get), TypeScript decorators (@Component),
//! C# attributes (\[HttpGet\]), Rust attributes (\#\[route\]).

use serde::{Deserialize, Serialize};

/// A decorator/attribute/annotation extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecoratorFact {
    /// File where the decorator appears.
    pub file: String,
    /// Language.
    pub language: String,
    /// Decorator name (e.g. "get", "Component", "HttpGet", "route").
    pub name: String,
    /// Full decorator text including arguments.
    pub full_text: String,
    /// Line number.
    pub line: usize,
    /// The symbol this decorator modifies (function/class name).
    pub target_symbol: Option<String>,
}

/// Extract decorators/annotations from a source file using simple text scanning.
///
/// This is a heuristic approach that works across languages:
/// - Python: lines starting with `@` before function/class definitions
/// - TypeScript: lines starting with `@` before function/class definitions
/// - C#: lines starting with `[` containing attributes
/// - Rust: lines starting with `#[` before items
pub fn extract_decorators(source: &str, file_path: &str, language: &str) -> Vec<DecoratorFact> {
    let mut facts = Vec::new();
    let mut pending_decorator_lines: Vec<(usize, String)> = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let line_num = line_idx + 1;

        match language {
            "python" | "typescript" | "javascript" => {
                if trimmed.starts_with('@') {
                    pending_decorator_lines.push((line_num, trimmed.to_string()));
                } else if !trimmed.is_empty()
                    && !trimmed.starts_with('#')
                    && !trimmed.starts_with("//")
                    && !trimmed.starts_with("/*")
                    && !trimmed.starts_with('*')
                {
                    // This is a definition line — attach pending decorators
                    if !pending_decorator_lines.is_empty() {
                        let target = extract_name_from_definition(trimmed);
                        for (dec_line, dec_text) in &pending_decorator_lines {
                            let name = dec_text
                                .split('(')
                                .next()
                                .unwrap_or(dec_text)
                                .trim_start_matches('@')
                                .to_string();
                            facts.push(DecoratorFact {
                                file: file_path.to_string(),
                                language: language.to_string(),
                                name,
                                full_text: dec_text.clone(),
                                line: *dec_line,
                                target_symbol: target.clone(),
                            });
                        }
                        pending_decorator_lines.clear();
                    }
                }
            }
            "csharp"
                if trimmed.starts_with('[')
                    && trimmed.contains(']')
                    && !trimmed.starts_with("[!")
                    && !trimmed.starts_with("[/") =>
            {
                // Extract attribute name
                let inner = &trimmed[1..trimmed.find(']').unwrap_or(trimmed.len())];
                let name = inner.split('(').next().unwrap_or(inner).trim().to_string();
                if !name.is_empty()
                    && name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                {
                    // Look ahead for the definition line
                    let target = find_next_definition(source, line_idx);
                    facts.push(DecoratorFact {
                        file: file_path.to_string(),
                        language: language.to_string(),
                        name,
                        full_text: trimmed.to_string(),
                        line: line_num,
                        target_symbol: target,
                    });
                }
            }
            "rust" if trimmed.starts_with("#[") && !trimmed.starts_with("#![") => {
                let inner = &trimmed[2..trimmed.find(']').unwrap_or(trimmed.len())];
                let name = inner.split('(').next().unwrap_or(inner).trim().to_string();
                if !name.is_empty() {
                    let target = find_next_definition(source, line_idx);
                    facts.push(DecoratorFact {
                        file: file_path.to_string(),
                        language: language.to_string(),
                        name,
                        full_text: trimmed.to_string(),
                        line: line_num,
                        target_symbol: target,
                    });
                }
            }
            _ => {}
        }
    }
    facts
}

fn extract_name_from_definition(line: &str) -> Option<String> {
    // "fn goodbye" / "def goodbye" / "function goodbye" / "class Foo" / "async def goodbye"
    let keywords = [
        "fn ",
        "def ",
        "function ",
        "class ",
        "async def ",
        "export function ",
        "export default function ",
        "export class ",
        "const ",
        "let ",
        "var ",
    ];
    for kw in &keywords {
        if let Some(rest) = line.strip_prefix(kw) {
            let name = rest
                .split(|c: char| c.is_whitespace() || c == '(' || c == '{' || c == ':')
                .next()
                .unwrap_or(rest);
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn find_next_definition(source: &str, after_line: usize) -> Option<String> {
    for line in source.lines().skip(after_line + 1).take(5) {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("//")
            || trimmed.starts_with('[')
        {
            continue;
        }
        return extract_name_from_definition(trimmed);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_decorators() {
        let source = r#"@app.get("/users")
fn get_users():
    pass

@app.post("/users")
fn create_user():
    pass"#;
        let facts = extract_decorators(source, "test.py", "python");
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].name, "app.get");
        assert_eq!(facts[0].target_symbol, Some("get_users".to_string()));
        assert_eq!(facts[1].name, "app.post");
        assert_eq!(facts[1].target_symbol, Some("create_user".to_string()));
    }

    #[test]
    fn test_csharp_attributes() {
        let source = r#"[HttpGet("users")]
public IActionResult GetUsers() { }

[HttpPost]
[Authorize]
public IActionResult CreateUser() { }"#;
        let facts = extract_decorators(source, "Controller.cs", "csharp");
        assert!(facts.len() >= 3);
        assert_eq!(facts[0].name, "HttpGet");
    }

    #[test]
    fn test_rust_attributes() {
        let source = r#"#[route("/api/users", method = "GET")]
fn get_users() {}

#[test]
fn my_test() {}"#;
        let facts = extract_decorators(source, "main.rs", "rust");
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].name, "route");
        assert_eq!(facts[1].name, "test");
    }
}
