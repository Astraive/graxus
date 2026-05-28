use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::{CallFact, CallKind, ConfidenceScore, ImportFact, ImportKind, LanguageIndexer, SymbolFact, SymbolKind, Visibility};

pub struct RustIndexer;

impl LanguageIndexer for RustIndexer {
    fn language_id(&self) -> &'static str { "rust" }
    fn extensions(&self) -> &'static [&'static str] { &["rs"] }
    fn tree_sitter_language(&self) -> tree_sitter::Language { tree_sitter_rust::LANGUAGE.into() }

    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &str, file_path: &str) -> Vec<ImportFact> {
        let lang = self.tree_sitter_language();
        let mut imports = Vec::new();
        let q = r#"(use_declaration argument: (scoped_identifier) @path) @use"#;
        if let Ok(query) = Query::new(&lang, q) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut path = String::new();
                let mut line = 0;
                for cap in m.captures {
                    match cap.index {
                        0 => { path = source[cap.node.byte_range()].to_string(); line = cap.node.start_position().row + 1; }
                        _ => {}
                    }
                }
                if !path.is_empty() {
                    let local = path.split("::").last().unwrap_or(&path).to_string();
                    imports.push(ImportFact {
                        id: String::new(), file: file_path.to_string(), language: "rust".to_string(),
                        kind: ImportKind::RustUse, source: path, local_name: Some(local),
                        imported_name: None, resolved_file: None, line, confidence: ConfidenceScore::unresolved(),
                    });
                }
            }
        }
        imports
    }

    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &str, file_path: &str) -> Vec<SymbolFact> {
        let lang = self.tree_sitter_language();
        let mut symbols = Vec::new();

        // Functions with parameters for signature
        let func_q = r#"(function_item name: (identifier) @name parameters: (parameters) @params) @def"#;
        if let Ok(query) = Query::new(&lang, func_q) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut params_text = String::new();
                let mut start = 0usize;
                let mut end = 0usize;
                for cap in m.captures {
                    match cap.index {
                        0 => name = source[cap.node.byte_range()].to_string(),
                        1 => params_text = source[cap.node.byte_range()].to_string(),
                        2 => { start = cap.node.start_position().row + 1; end = cap.node.end_position().row + 1; }
                        _ => {}
                    }
                }
                if !name.is_empty() {
                    let is_test = is_test_function(tree, source, name.as_str(), start);
                    let sig = format!("fn {}{}", name, params_text);
                    symbols.push(SymbolFact {
                        id: String::new(), file: file_path.to_string(), language: "rust".to_string(),
                        kind: SymbolKind::Function, name, exported: true, line_start: start, line_end: end,
                        visibility: Visibility::Public, signature: sig, is_test, usage_count: 0,
                    });
                }
            }
        }

        // Structs, enums, traits, types, constants, impls
        let queries = [
            (r#"(struct_item name: (type_identifier) @name) @def"#, SymbolKind::Struct),
            (r#"(enum_item name: (type_identifier) @name) @def"#, SymbolKind::Enum),
            (r#"(trait_item name: (type_identifier) @name) @def"#, SymbolKind::Trait),
            (r#"(type_item name: (type_identifier) @name) @def"#, SymbolKind::Type),
            (r#"(const_item name: (identifier) @name) @def"#, SymbolKind::Constant),
            (r#"(impl_item type: (type_identifier) @name) @def"#, SymbolKind::Module),
        ];
        for (q, kind) in queries {
            if let Ok(query) = Query::new(&lang, q) {
                let mut cursor = QueryCursor::new();
                let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
                while let Some(m) = matches.next() {
                    let mut name = String::new();
                    let mut start = 0usize;
                    let mut end = 0usize;
                    for cap in m.captures {
                        match cap.index {
                            0 => name = source[cap.node.byte_range()].to_string(),
                            1 => { start = cap.node.start_position().row + 1; end = cap.node.end_position().row + 1; }
                            _ => {}
                        }
                    }
                    if !name.is_empty() {
                        symbols.push(SymbolFact {
                            id: String::new(), file: file_path.to_string(), language: "rust".to_string(),
                            kind, name, exported: true, line_start: start, line_end: end,
                            visibility: Visibility::Public, signature: String::new(), is_test: false, usage_count: 0,
                        });
                    }
                }
            }
        }
        symbols
    }

    fn extract_calls(&self, tree: &tree_sitter::Tree, source: &str, file_path: &str) -> Vec<CallFact> {
        let lang = self.tree_sitter_language();
        let mut calls = Vec::new();
        // Simple: foo()
        let q1 = r#"(call_expression function: (identifier) @callee) @call"#;
        if let Ok(query) = Query::new(&lang, q1) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut callee = String::new();
                let mut line = 0usize;
                let mut col = 0usize;
                for cap in m.captures {
                    match cap.index {
                        0 => callee = source[cap.node.byte_range()].to_string(),
                        1 => { line = cap.node.start_position().row + 1; col = cap.node.start_position().column; }
                        _ => {}
                    }
                }
                if !callee.is_empty() {
                    calls.push(CallFact { id: String::new(), file: file_path.to_string(), language: "rust".to_string(),
                        kind: CallKind::FunctionCall, caller_symbol: None, callee_text: callee, object: None,
                        resolved_symbol: None, line, column: col, confidence: ConfidenceScore::unresolved() });
                }
            }
        }
        // Path: foo::bar()
        let q2 = r#"(call_expression function: (scoped_identifier) @path) @call"#;
        if let Ok(query) = Query::new(&lang, q2) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut path = String::new();
                let mut line = 0usize;
                let mut col = 0usize;
                for cap in m.captures {
                    match cap.index {
                        0 => path = source[cap.node.byte_range()].to_string(),
                        1 => { line = cap.node.start_position().row + 1; col = cap.node.start_position().column; }
                        _ => {}
                    }
                }
                if !path.is_empty() {
                    calls.push(CallFact { id: String::new(), file: file_path.to_string(), language: "rust".to_string(),
                        kind: CallKind::PathCall, caller_symbol: None, callee_text: path, object: None,
                        resolved_symbol: None, line, column: col, confidence: ConfidenceScore::unresolved() });
                }
            }
        }
        calls
    }
}

/// Check if a function has a #[test] attribute by looking at the source text above the function.
fn is_test_function(_tree: &tree_sitter::Tree, source: &str, _name: &str, line_start: usize) -> bool {
    // Simple heuristic: check if line_start-1 or line_start-2 has #[test]
    let lines: Vec<&str> = source.lines().collect();
    if line_start >= 2 {
        let prev = lines.get(line_start - 2).unwrap_or(&"");
        if prev.trim().starts_with("#[test") {
            return true;
        }
    }
    if line_start >= 3 {
        let prev = lines.get(line_start - 3).unwrap_or(&"");
        if prev.trim().starts_with("#[test") {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_rust(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into()).unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_extract_functions() {
        let source = r#"fn hello() { println!("hi"); }"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "hello");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
    }

    #[test]
    fn test_extract_function_signature() {
        let source = r#"fn add(a: i32, b: i32) -> i32 { a + b }"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].signature.contains("add"));
        assert!(symbols[0].signature.contains("a: i32"));
    }

    #[test]
    fn test_extract_struct() {
        let source = r#"struct MyStruct { id: u32 }"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyStruct");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"use std::collections::HashMap;"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer;
        let imports = indexer.extract_imports(&tree, source, "test.rs");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "std::collections::HashMap");
        assert_eq!(imports[0].local_name, Some("HashMap".to_string()));
    }

    #[test]
    fn test_extract_calls() {
        let source = r#"fn main() { foo(); bar::baz(); }"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer;
        let calls = indexer.extract_calls(&tree, source, "test.rs");
        assert!(calls.len() >= 2);
    }

    #[test]
    fn test_is_test_function() {
        let source = r#"#[test]
fn my_test() {
    assert_eq!(1, 1);
}"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].is_test);
    }
}
