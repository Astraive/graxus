//! Rust language indexer using tree-sitter.

use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::{
    languages::visibility::rust_visibility, CallFact, CallKind, ConfidenceScore, ImportFact,
    ImportKind, LanguageIndexer, SymbolFact, SymbolKind,
};

/// Rust language indexer with pre-compiled tree-sitter queries.
pub struct RustIndexer {
    import_use_query: Query,
    sym_func_query: Query,
    sym_struct_query: Query,
    sym_enum_query: Query,
    sym_trait_query: Query,
    sym_type_query: Query,
    sym_const_query: Query,
    sym_impl_query: Query,
    call_ident_query: Query,
    call_path_query: Query,
}

impl RustIndexer {
    /// Create a new RustIndexer with pre-compiled tree-sitter queries.
    pub fn new() -> Self {
        let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        Self {
            import_use_query: Query::new(
                &lang,
                r#"(use_declaration argument: (scoped_identifier) @path) @use"#,
            )
            .expect("valid rust use query"),
            sym_func_query: Query::new(
                &lang,
                r#"(function_item name: (identifier) @name parameters: (parameters) @params) @def"#,
            )
            .expect("valid rust func query"),
            sym_struct_query: Query::new(
                &lang,
                r#"(struct_item name: (type_identifier) @name) @def"#,
            )
            .expect("valid rust struct query"),
            sym_enum_query: Query::new(&lang, r#"(enum_item name: (type_identifier) @name) @def"#)
                .expect("valid rust enum query"),
            sym_trait_query: Query::new(
                &lang,
                r#"(trait_item name: (type_identifier) @name) @def"#,
            )
            .expect("valid rust trait query"),
            sym_type_query: Query::new(&lang, r#"(type_item name: (type_identifier) @name) @def"#)
                .expect("valid rust type query"),
            sym_const_query: Query::new(&lang, r#"(const_item name: (identifier) @name) @def"#)
                .expect("valid rust const query"),
            sym_impl_query: Query::new(&lang, r#"(impl_item type: (type_identifier) @name) @def"#)
                .expect("valid rust impl query"),
            call_ident_query: Query::new(
                &lang,
                r#"(call_expression function: (identifier) @callee) @call"#,
            )
            .expect("valid rust call ident query"),
            call_path_query: Query::new(
                &lang,
                r#"(call_expression function: (scoped_identifier) @path) @call"#,
            )
            .expect("valid rust call path query"),
        }
    }
}

impl Default for RustIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageIndexer for RustIndexer {
    fn language_id(&self) -> &'static str {
        "rust"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }
    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn extract_imports(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<ImportFact> {
        let mut imports = Vec::new();
        let query = &self.import_use_query;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
        while let Some(m) = matches.next() {
            let mut path = String::new();
            let mut line = 0;
            for cap in m.captures {
                if cap.index == 0 {
                    path = source[cap.node.byte_range()].to_string();
                    line = cap.node.start_position().row + 1;
                }
            }
            if !path.is_empty() {
                let local = path.split("::").last().unwrap_or(&path).to_string();
                imports.push(ImportFact {
                    id: String::new(),
                    file: file_path.to_string(),
                    language: "rust".to_string(),
                    kind: ImportKind::RustUse,
                    source: path,
                    local_name: Some(local),
                    imported_name: None,
                    resolved_file: None,
                    line,
                    confidence: ConfidenceScore::unresolved(),
                });
            }
        }
        imports
    }

    fn extract_symbols(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<SymbolFact> {
        let mut symbols = Vec::new();

        // Functions with parameters for signature
        {
            let query = &self.sym_func_query;
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut params_text = String::new();
                let mut start = 0usize;
                let mut end = 0usize;
                let mut def_node: Option<tree_sitter::Node> = None;
                for cap in m.captures {
                    match cap.index {
                        0 => name = source[cap.node.byte_range()].to_string(),
                        1 => params_text = source[cap.node.byte_range()].to_string(),
                        2 => {
                            def_node = Some(cap.node);
                            start = cap.node.start_position().row + 1;
                            end = cap.node.end_position().row + 1;
                        }
                        _ => {}
                    }
                }
                if !name.is_empty() {
                    let is_test = is_test_function(tree, source, name.as_str(), start);
                    let sig = format!("fn {}{}", name, params_text);
                    let (exported, visibility) = rust_visibility(def_node, source);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "rust".to_string(),
                        kind: SymbolKind::Function,
                        name,
                        exported,
                        line_start: start,
                        line_end: end,
                        visibility,
                        signature: sig,
                        is_test,
                        usage_count: 0,
                        ..Default::default()
                    });
                }
            }
        }

        // Structs, enums, traits, types, constants, impls
        let queries: [(&Query, SymbolKind); 6] = [
            (&self.sym_struct_query, SymbolKind::Struct),
            (&self.sym_enum_query, SymbolKind::Enum),
            (&self.sym_trait_query, SymbolKind::Trait),
            (&self.sym_type_query, SymbolKind::Type),
            (&self.sym_const_query, SymbolKind::Constant),
            (&self.sym_impl_query, SymbolKind::Module),
        ];
        for (query, kind) in &queries {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut start = 0usize;
                let mut end = 0usize;
                let mut def_node: Option<tree_sitter::Node> = None;
                for cap in m.captures {
                    match cap.index {
                        0 => name = source[cap.node.byte_range()].to_string(),
                        1 => {
                            def_node = Some(cap.node);
                            start = cap.node.start_position().row + 1;
                            end = cap.node.end_position().row + 1;
                        }
                        _ => {}
                    }
                }
                if !name.is_empty() {
                    let (exported, visibility) = rust_visibility(def_node, source);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "rust".to_string(),
                        kind: *kind,
                        name,
                        exported,
                        line_start: start,
                        line_end: end,
                        visibility,
                        signature: String::new(),
                        is_test: false,
                        usage_count: 0,
                        ..Default::default()
                    });
                }
            }
        }
        symbols
    }

    fn extract_calls(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<CallFact> {
        let mut calls = Vec::new();
        // Simple: foo()
        {
            let query = &self.call_ident_query;
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut callee = String::new();
                let mut line = 0usize;
                let mut col = 0usize;
                for cap in m.captures {
                    match cap.index {
                        0 => callee = source[cap.node.byte_range()].to_string(),
                        1 => {
                            line = cap.node.start_position().row + 1;
                            col = cap.node.start_position().column;
                        }
                        _ => {}
                    }
                }
                if !callee.is_empty() {
                    calls.push(CallFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "rust".to_string(),
                        kind: CallKind::FunctionCall,
                        caller_symbol: None,
                        callee_text: callee,
                        object: None,
                        resolved_symbol: None,
                        line,
                        column: col,
                        confidence: ConfidenceScore::unresolved(),
                    });
                }
            }
        }
        // Path: foo::bar()
        {
            let query = &self.call_path_query;
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut path = String::new();
                let mut line = 0usize;
                let mut col = 0usize;
                for cap in m.captures {
                    match cap.index {
                        0 => path = source[cap.node.byte_range()].to_string(),
                        1 => {
                            line = cap.node.start_position().row + 1;
                            col = cap.node.start_position().column;
                        }
                        _ => {}
                    }
                }
                if !path.is_empty() {
                    calls.push(CallFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "rust".to_string(),
                        kind: CallKind::PathCall,
                        caller_symbol: None,
                        callee_text: path,
                        object: None,
                        resolved_symbol: None,
                        line,
                        column: col,
                        confidence: ConfidenceScore::unresolved(),
                    });
                }
            }
        }
        calls
    }
}

/// Check if a function has a #[test] attribute by looking at the source text above the function.
fn is_test_function(
    _tree: &tree_sitter::Tree,
    source: &str,
    _name: &str,
    line_start: usize,
) -> bool {
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
    use crate::Visibility;

    fn parse_rust(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_extract_functions() {
        let source = r#"fn goodbye() { println!("hi"); }"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "goodbye");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
    }

    #[test]
    fn test_extract_function_signature() {
        let source = r#"fn add(a: i32, b: i32) -> i32 { a + b }"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].signature.contains("add"));
        assert!(symbols[0].signature.contains("a: i32"));
    }

    #[test]
    fn test_extract_struct() {
        let source = r#"struct MyStruct { id: u32 }"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyStruct");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"use std::collections::HashMap;"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer::new();
        let imports = indexer.extract_imports(&tree, source, "test.rs");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "std::collections::HashMap");
        assert_eq!(imports[0].local_name, Some("HashMap".to_string()));
    }

    #[test]
    fn test_extract_calls() {
        let source = r#"fn main() { foo(); bar::baz(); }"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer::new();
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
        let indexer = RustIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].is_test);
    }

    fn symbol_named<'a>(symbols: &'a [SymbolFact], name: &str) -> &'a SymbolFact {
        symbols
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("symbol '{}' not found in {:?}", name, symbols))
    }

    #[test]
    fn test_visibility_private_function() {
        let source = "fn private_fn() {}";
        let tree = parse_rust(source);
        let indexer = RustIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        let s = symbol_named(&symbols, "private_fn");
        assert!(!s.exported, "plain fn should NOT be exported");
        assert_eq!(s.visibility, Visibility::Private);
    }

    #[test]
    fn test_visibility_pub_function() {
        let source = "pub fn public_fn() {}";
        let tree = parse_rust(source);
        let indexer = RustIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        let s = symbol_named(&symbols, "public_fn");
        assert!(s.exported, "pub fn should be exported");
        assert_eq!(s.visibility, Visibility::Public);
    }

    #[test]
    fn test_visibility_pub_crate_is_not_exported() {
        let source = "pub(crate) fn crate_fn() {}";
        let tree = parse_rust(source);
        let indexer = RustIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        let s = symbol_named(&symbols, "crate_fn");
        assert!(
            !s.exported,
            "pub(crate) is crate-internal, must not be flagged exported (else dead-code skips it)"
        );
        assert_eq!(s.visibility, Visibility::Internal);
    }

    #[test]
    fn test_visibility_on_struct_and_enum() {
        let source = r#"
struct PrivateStruct {}
pub struct PublicStruct {}
enum PrivateEnum {}
pub enum PublicEnum {}
"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        assert!(!symbol_named(&symbols, "PrivateStruct").exported);
        assert!(symbol_named(&symbols, "PublicStruct").exported);
        assert!(!symbol_named(&symbols, "PrivateEnum").exported);
        assert!(symbol_named(&symbols, "PublicEnum").exported);
    }

    #[test]
    fn test_mixed_visibility_in_one_file() {
        // This is the dead-code scan scenario: pub used + private unused.
        let source = r#"
fn main() {
    used();
}

fn used() {}

fn unused() {}

pub fn api() {}
"#;
        let tree = parse_rust(source);
        let indexer = RustIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.rs");
        assert!(!symbol_named(&symbols, "main").exported);
        assert!(!symbol_named(&symbols, "used").exported);
        assert!(!symbol_named(&symbols, "unused").exported);
        assert!(symbol_named(&symbols, "api").exported);
    }
}