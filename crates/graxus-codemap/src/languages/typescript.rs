use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::{CallFact, CallKind, ConfidenceScore, ImportFact, ImportKind, LanguageIndexer, SymbolFact, SymbolKind, Visibility};

pub struct TypeScriptIndexer;

impl LanguageIndexer for TypeScriptIndexer {
    fn language_id(&self) -> &'static str { "typescript" }
    fn extensions(&self) -> &'static [&'static str] { &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"] }
    fn tree_sitter_language(&self) -> tree_sitter::Language { tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into() }

    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &str, file_path: &str) -> Vec<ImportFact> {
        let lang = self.tree_sitter_language();
        let mut imports = Vec::new();

        // Named: import { a } from "source"
        let q1 = r#"(import_statement (import_clause (named_imports (import_specifier name: (identifier) @name))) source: (string) @source) @import"#;
        if let Ok(query) = Query::new(&lang, q1) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut src = String::new();
                let mut line = 0;
                for cap in m.captures {
                    let text = &source[cap.node.byte_range()];
                    match cap.index {
                        0 => name = text.to_string(),
                        1 => src = text.trim_matches('"').trim_matches('\'').to_string(),
                        2 => line = cap.node.start_position().row + 1,
                        _ => {}
                    }
                }
                if !src.is_empty() && !name.is_empty() {
                    imports.push(ImportFact { id: String::new(), file: file_path.to_string(), language: "typescript".to_string(),
                        kind: ImportKind::NamedImport, source: src, local_name: Some(name.clone()), imported_name: Some(name),
                        resolved_file: None, line, confidence: ConfidenceScore::unresolved() });
                }
            }
        }

        // Default: import X from "source"
        let q2 = r#"(import_statement (import_clause (identifier) @name) source: (string) @source) @import"#;
        if let Ok(query) = Query::new(&lang, q2) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut src = String::new();
                let mut line = 0;
                for cap in m.captures {
                    let text = &source[cap.node.byte_range()];
                    match cap.index {
                        0 => name = text.to_string(),
                        1 => src = text.trim_matches('"').trim_matches('\'').to_string(),
                        2 => line = cap.node.start_position().row + 1,
                        _ => {}
                    }
                }
                if !src.is_empty() {
                    imports.push(ImportFact { id: String::new(), file: file_path.to_string(), language: "typescript".to_string(),
                        kind: ImportKind::DefaultImport, source: src, local_name: Some(name), imported_name: None,
                        resolved_file: None, line, confidence: ConfidenceScore::unresolved() });
                }
            }
        }

        // Namespace: import * as X from "source"
        let q3 = r#"(import_statement (import_clause (namespace_import (identifier) @name)) source: (string) @source) @import"#;
        if let Ok(query) = Query::new(&lang, q3) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut src = String::new();
                let mut line = 0;
                for cap in m.captures {
                    let text = &source[cap.node.byte_range()];
                    match cap.index {
                        0 => name = text.to_string(),
                        1 => src = text.trim_matches('"').trim_matches('\'').to_string(),
                        2 => line = cap.node.start_position().row + 1,
                        _ => {}
                    }
                }
                if !src.is_empty() {
                    imports.push(ImportFact { id: String::new(), file: file_path.to_string(), language: "typescript".to_string(),
                        kind: ImportKind::NamespaceImport, source: src, local_name: Some(name), imported_name: None,
                        resolved_file: None, line, confidence: ConfidenceScore::unresolved() });
                }
            }
        }

        imports
    }

    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &str, file_path: &str) -> Vec<SymbolFact> {
        let lang = self.tree_sitter_language();
        let mut symbols = Vec::new();

        // Functions with parameters (for signature)
        let func_q = r#"(function_declaration name: (identifier) @name parameters: (formal_parameters) @params) @def"#;
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
                    let is_test = is_test_name(&name);
                    let sig = format!("function {}{}", name, params_text);
                    symbols.push(SymbolFact {
                        id: String::new(), file: file_path.to_string(), language: "typescript".to_string(),
                        kind: SymbolKind::Function, name, exported: true, line_start: start, line_end: end,
                        visibility: Visibility::Public, signature: sig, is_test, usage_count: 0,
                    });
                }
            }
        }

        // Classes, interfaces, types, enums
        let queries = [
            (r#"(class_declaration name: (type_identifier) @name) @def"#, SymbolKind::Class),
            (r#"(interface_declaration name: (type_identifier) @name) @def"#, SymbolKind::Interface),
            (r#"(type_alias_declaration name: (type_identifier) @name) @def"#, SymbolKind::Type),
            (r#"(enum_declaration name: (identifier) @name) @def"#, SymbolKind::Enum),
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
                            id: String::new(), file: file_path.to_string(), language: "typescript".to_string(),
                            kind, name, exported: true, line_start: start, line_end: end,
                            visibility: Visibility::Public, signature: String::new(), is_test: false, usage_count: 0,
                        });
                    }
                }
            }
        }

        // Constants: const/let at module level
        let q_const = r#"(lexical_declaration (variable_declarator name: (identifier) @name)) @def"#;
        if let Ok(query) = Query::new(&lang, q_const) {
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
                        id: String::new(), file: file_path.to_string(), language: "typescript".to_string(),
                        kind: SymbolKind::Constant, name, exported: true, line_start: start, line_end: end,
                        visibility: Visibility::Public, signature: String::new(), is_test: false, usage_count: 0,
                    });
                }
            }
        }

        // Detect test functions via it() / test() / describe() calls with arrow functions
        let test_q = r#"(call_expression function: (identifier) @test_name arguments: (arguments (string) @desc (arrow_function) @fn)) @test"#;
        if let Ok(query) = Query::new(&lang, test_q) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut test_name = String::new();
                let mut desc = String::new();
                let mut start = 0usize;
                for cap in m.captures {
                    match cap.index {
                        0 => test_name = source[cap.node.byte_range()].to_string(),
                        1 => desc = source[cap.node.byte_range()].trim_matches('"').trim_matches('\'').to_string(),
                        2 => { start = cap.node.start_position().row + 1; }
                        _ => {}
                    }
                }
                if test_name == "it" || test_name == "test" {
                    let sym_name = if desc.is_empty() { test_name.clone() } else { desc.clone() };
                    symbols.push(SymbolFact {
                        id: String::new(), file: file_path.to_string(), language: "typescript".to_string(),
                        kind: SymbolKind::Function, name: sym_name, exported: false,
                        line_start: start, line_end: start, visibility: Visibility::Private,
                        signature: format!("{}({})", test_name, desc), is_test: true, usage_count: 0,
                    });
                }
            }
        }

        symbols
    }

    fn extract_calls(&self, tree: &tree_sitter::Tree, source: &str, file_path: &str) -> Vec<CallFact> {
        let lang = self.tree_sitter_language();
        let mut calls = Vec::new();
        let q = r#"(call_expression function: (identifier) @callee) @call"#;
        if let Ok(query) = Query::new(&lang, q) {
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
                    calls.push(CallFact { id: String::new(), file: file_path.to_string(), language: "typescript".to_string(),
                        kind: CallKind::FunctionCall, caller_symbol: None, callee_text: callee, object: None,
                        resolved_symbol: None, line, column: col, confidence: ConfidenceScore::unresolved() });
                }
            }
        }
        // Member calls: obj.method()
        let q2 = r#"(call_expression function: (member_expression object: (identifier) @object property: (property_identifier) @property)) @call"#;
        if let Ok(query) = Query::new(&lang, q2) {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut object = String::new();
                let mut property = String::new();
                let mut line = 0usize;
                let mut col = 0usize;
                for cap in m.captures {
                    let text = &source[cap.node.byte_range()];
                    match cap.index {
                        0 => { line = cap.node.start_position().row + 1; col = cap.node.start_position().column; }
                        1 => object = text.to_string(),
                        2 => property = text.to_string(),
                        _ => {}
                    }
                }
                if !property.is_empty() {
                    calls.push(CallFact { id: String::new(), file: file_path.to_string(), language: "typescript".to_string(),
                        kind: CallKind::MethodCall, caller_symbol: None, callee_text: property,
                        object: Some(object), resolved_symbol: None, line, column: col, confidence: ConfidenceScore::unresolved() });
                }
            }
        }
        // Constructor calls: new Foo()
        let q3 = r#"(new_expression constructor: (identifier) @callee) @call"#;
        if let Ok(query) = Query::new(&lang, q3) {
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
                    calls.push(CallFact { id: String::new(), file: file_path.to_string(), language: "typescript".to_string(),
                        kind: CallKind::ConstructorCall, caller_symbol: None, callee_text: callee, object: None,
                        resolved_symbol: None, line, column: col, confidence: ConfidenceScore::unresolved() });
                }
            }
        }
        calls
    }
}

/// Detect if a function name looks like a test name.
fn is_test_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with("test_") || lower.starts_with("test ") || lower == "test"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ts(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()).unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_extract_functions() {
        let source = r#"function hello() { return 1; }"#;
        let tree = parse_ts(source);
        let indexer = TypeScriptIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "hello");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
    }

    #[test]
    fn test_extract_function_signature() {
        let source = r#"function greet(name: string, age: number): string { return ""; }"#;
        let tree = parse_ts(source);
        let indexer = TypeScriptIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].signature.contains("greet"));
        assert!(symbols[0].signature.contains("name: string"));
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"import { foo } from "./bar";"#;
        let tree = parse_ts(source);
        let indexer = TypeScriptIndexer;
        let imports = indexer.extract_imports(&tree, source, "test.ts");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "./bar");
        assert_eq!(imports[0].local_name, Some("foo".to_string()));
    }

    #[test]
    fn test_extract_class() {
        let source = r#"class MyClass {}"#;
        let tree = parse_ts(source);
        let indexer = TypeScriptIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyClass");
        assert_eq!(symbols[0].kind, SymbolKind::Class);
    }

    #[test]
    fn test_extract_interface() {
        let source = r#"interface MyInterface { id: number; }"#;
        let tree = parse_ts(source);
        let indexer = TypeScriptIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyInterface");
        assert_eq!(symbols[0].kind, SymbolKind::Interface);
    }

    #[test]
    fn test_extract_calls() {
        let source = r#"foo(); bar.baz();"#;
        let tree = parse_ts(source);
        let indexer = TypeScriptIndexer;
        let calls = indexer.extract_calls(&tree, source, "test.ts");
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn test_is_test_name() {
        assert!(is_test_name("test_something"));
        assert!(is_test_name("test it works"));
        assert!(!is_test_name("handleClick"));
        assert!(!is_test_name("getData"));
    }

    #[test]
    fn test_detect_test_calls() {
        let source = r#"it("should work", () => { expect(true).toBe(true); });"#;
        let tree = parse_ts(source);
        let indexer = TypeScriptIndexer;
        let symbols = indexer.extract_symbols(&tree, source, "test.ts");
        let test_syms: Vec<_> = symbols.iter().filter(|s| s.is_test).collect();
        assert_eq!(test_syms.len(), 1);
        assert_eq!(test_syms[0].name, "should work");
    }
}
