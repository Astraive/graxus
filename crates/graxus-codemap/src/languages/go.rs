use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::{CallFact, CallKind, ConfidenceScore, ImportFact, ImportKind, LanguageIndexer, SymbolFact, SymbolKind, Visibility};

pub struct GoIndexer;

impl LanguageIndexer for GoIndexer {
    fn language_id(&self) -> &'static str { "go" }
    fn extensions(&self) -> &'static [&'static str] { &["go"] }
    fn tree_sitter_language(&self) -> tree_sitter::Language { tree_sitter_go::LANGUAGE.into() }

    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &str, file_path: &str) -> Vec<ImportFact> {
        let lang = self.tree_sitter_language();
        let mut imports = Vec::new();

        let import_query = r#"(import_declaration (import_spec_list (import_spec path: (interpreted_string_literal) @source))) @import"#;
        if let Ok(query) = Query::new(&lang, import_query) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut source_str = String::new();
                let mut line = 0;
                for capture in m.captures {
                    if capture.index == 0 {
                        source_str = source[capture.node.byte_range()].trim_matches('"').to_string();
                        line = capture.node.start_position().row + 1;
                    }
                }
                if !source_str.is_empty() {
                    let local_name = source_str.split('/').last().unwrap_or(&source_str).to_string();
                    imports.push(ImportFact {
                        id: String::new(), file: file_path.to_string(), language: "go".to_string(),
                        kind: ImportKind::GoImport, source: source_str, local_name: Some(local_name),
                        imported_name: None, resolved_file: None, line, confidence: ConfidenceScore::unresolved(),
                    });
                }
            }
        }

        // Single import
        let single_query = r#"(import_declaration (import_spec path: (interpreted_string_literal) @source)) @import"#;
        if let Ok(query) = Query::new(&lang, single_query) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut source_str = String::new();
                let mut line = 0;
                for capture in m.captures {
                    if capture.index == 0 {
                        source_str = source[capture.node.byte_range()].trim_matches('"').to_string();
                        line = capture.node.start_position().row + 1;
                    }
                }
                if !source_str.is_empty() {
                    let local_name = source_str.split('/').last().unwrap_or(&source_str).to_string();
                    imports.push(ImportFact {
                        id: String::new(), file: file_path.to_string(), language: "go".to_string(),
                        kind: ImportKind::GoImport, source: source_str, local_name: Some(local_name),
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
        let func_q = r#"(function_declaration name: (identifier) @name parameters: (parameter_list) @params) @func"#;
        if let Ok(query) = Query::new(&lang, func_q) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut params_text = String::new();
                let mut start = 0; let mut end = 0;
                for capture in m.captures {
                    match capture.index {
                        0 => name = source[capture.node.byte_range()].to_string(),
                        1 => params_text = source[capture.node.byte_range()].to_string(),
                        2 => { start = capture.node.start_position().row + 1; end = capture.node.end_position().row + 1; }
                        _ => {}
                    }
                }
                if !name.is_empty() {
                    let exported = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
                    let is_test = is_go_test(&name, &params_text);
                    let sig = format!("func {}{}", name, params_text);
                    symbols.push(SymbolFact {
                        id: String::new(), file: file_path.to_string(), language: "go".to_string(),
                        kind: SymbolKind::Function, name, exported, line_start: start, line_end: end,
                        visibility: if exported { Visibility::Public } else { Visibility::Private },
                        signature: sig, is_test, usage_count: 0,
                    });
                }
            }
        }

        // Types (struct, interface)
        let type_queries = [
            (r#"(type_declaration (type_spec name: (type_identifier) @name)) @type"#, SymbolKind::Struct),
        ];
        for (q, kind) in type_queries {
            if let Ok(query) = Query::new(&lang, q) {
                let mut cursor = QueryCursor::new();
                let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
                while let Some(m) = matches.next() {
                    let mut name = String::new();
                    let mut start = 0; let mut end = 0;
                    for capture in m.captures {
                        match capture.index {
                            0 => name = source[capture.node.byte_range()].to_string(),
                            1 => { start = capture.node.start_position().row + 1; end = capture.node.end_position().row + 1; }
                            _ => {}
                        }
                    }
                    if !name.is_empty() {
                        let exported = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
                        symbols.push(SymbolFact {
                            id: String::new(), file: file_path.to_string(), language: "go".to_string(),
                            kind, name, exported, line_start: start, line_end: end,
                            visibility: if exported { Visibility::Public } else { Visibility::Private },
                            signature: String::new(), is_test: false, usage_count: 0,
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

        // Function calls: foo()
        let call_q = r#"(call_expression function: (identifier) @callee) @call"#;
        if let Ok(query) = Query::new(&lang, call_q) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut callee = String::new(); let mut line = 0; let mut col = 0;
                for capture in m.captures {
                    match capture.index {
                        0 => callee = source[capture.node.byte_range()].to_string(),
                        1 => { line = capture.node.start_position().row + 1; col = capture.node.start_position().column; }
                        _ => {}
                    }
                }
                if !callee.is_empty() {
                    calls.push(CallFact { id: String::new(), file: file_path.to_string(), language: "go".to_string(),
                        kind: CallKind::FunctionCall, caller_symbol: None, callee_text: callee, object: None,
                        resolved_symbol: None, line, column: col, confidence: ConfidenceScore::unresolved() });
                }
            }
        }

        // Selector calls: obj.method()
        let selector_q = r#"(call_expression function: (selector_expression operand: (identifier) @object field: (field_identifier) @property)) @call"#;
        if let Ok(query) = Query::new(&lang, selector_q) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut object = String::new(); let mut property = String::new();
                let mut line = 0; let mut col = 0;
                for capture in m.captures {
                    let text = &source[capture.node.byte_range()];
                    match capture.index {
                        0 => { line = capture.node.start_position().row + 1; col = capture.node.start_position().column; }
                        1 => object = text.to_string(),
                        2 => property = text.to_string(),
                        _ => {}
                    }
                }
                if !property.is_empty() {
                    calls.push(CallFact { id: String::new(), file: file_path.to_string(), language: "go".to_string(),
                        kind: CallKind::SelectorCall, caller_symbol: None, callee_text: property, object: Some(object),
                        resolved_symbol: None, line, column: col, confidence: ConfidenceScore::unresolved() });
                }
            }
        }
        calls
    }
}

/// Go test functions start with "Test" and take *testing.T
fn is_go_test(name: &str, params: &str) -> bool {
    name.starts_with("Test") && name.len() > 4 && params.contains("testing.T")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_go(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_go::LANGUAGE.into()).unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_extract_functions() {
        let source = r#"package main
func hello() string { return "hi" }"#;
        let tree = parse_go(source);
        let indexer = GoIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.go");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "hello");
    }

    #[test]
    fn test_extract_function_signature() {
        let source = r#"package main
func add(a int, b int) int { return a + b }"#;
        let tree = parse_go(source);
        let indexer = GoIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.go");
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].signature.contains("add"));
        assert!(symbols[0].signature.contains("a int"));
    }

    #[test]
    fn test_extract_struct() {
        let source = r#"package main
type MyStruct struct { ID int }"#;
        let tree = parse_go(source);
        let indexer = GoIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.go");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyStruct");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"package main
import "fmt""#;
        let tree = parse_go(source);
        let indexer = GoIndexer;
        let imports = indexer.extract_imports(&tree, source, "test.go");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "fmt");
    }

    #[test]
    fn test_is_go_test() {
        assert!(is_go_test("TestHello", "(t *testing.T)"));
        assert!(!is_go_test("hello", "()"));
        assert!(!is_go_test("Test", "(t *testing.T)"));
    }

    #[test]
    fn test_detect_test_function() {
        let source = r#"package main
func TestAdd(t *testing.T) {
    if add(1, 2) != 3 { t.Fatal("fail") }
}"#;
        let tree = parse_go(source);
        let indexer = GoIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.go");
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].is_test);
    }
}
