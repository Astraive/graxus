//! C++ language indexer using tree-sitter.

use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::{
    languages::visibility::cpp_visibility, CallFact, CallKind, ConfidenceScore, ImportFact,
    ImportKind, LanguageIndexer, SymbolFact, SymbolKind,
};

/// C++ language indexer with pre-compiled tree-sitter queries.
pub struct CppIndexer {
    import_system_query: Query,
    import_quote_query: Query,
    import_using_query: Query,
    sym_func_query: Query,
    sym_class_query: Query,
    sym_struct_query: Query,
    sym_enum_query: Query,
    sym_typedef_query: Query,
    sym_namespace_query: Query,
    call_query: Query,
    call_new_query: Query,
}

impl CppIndexer {
    /// Create a new CppIndexer with pre-compiled tree-sitter queries.
    pub fn new() -> Self {
        let lang: tree_sitter::Language = tree_sitter_cpp::LANGUAGE.into();
        Self {
            import_system_query: Query::new(
                &lang,
                r#"(preproc_include path: (system_lib_string) @path) @include"#,
            )
            .expect("valid cpp system include query"),
            import_quote_query: Query::new(
                &lang,
                r#"(preproc_include path: (string_literal) @path) @include"#,
            )
            .expect("valid cpp quote include query"),
            import_using_query: Query::new(&lang, r#"(using_declaration (identifier) @name) @use"#)
                .expect("valid cpp using query"),
            sym_func_query: Query::new(
                &lang,
                r#"(function_definition
                    declarator: (function_declarator
                        declarator: (identifier) @name
                        parameters: (parameter_list) @params)
                ) @def"#,
            )
            .expect("valid cpp func query"),
            sym_class_query: Query::new(
                &lang,
                r#"(class_specifier name: (type_identifier) @name) @def"#,
            )
            .expect("valid cpp class query"),
            sym_struct_query: Query::new(
                &lang,
                r#"(struct_specifier name: (type_identifier) @name) @def"#,
            )
            .expect("valid cpp struct query"),
            sym_enum_query: Query::new(
                &lang,
                r#"(enum_specifier name: (type_identifier) @name) @def"#,
            )
            .expect("valid cpp enum query"),
            sym_typedef_query: Query::new(
                &lang,
                r#"(type_definition type: (type_identifier) @name) @def"#,
            )
            .expect("valid cpp typedef query"),
            sym_namespace_query: Query::new(
                &lang,
                r#"(namespace_definition name: (namespace_identifier) @name) @def"#,
            )
            .expect("valid cpp namespace query"),
            call_query: Query::new(
                &lang,
                r#"(call_expression function: (identifier) @callee) @call"#,
            )
            .expect("valid cpp call query"),
            call_new_query: Query::new(
                &lang,
                r#"(new_expression type: (type_identifier) @name) @call"#,
            )
            .expect("valid cpp new expression query"),
        }
    }
}

impl Default for CppIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageIndexer for CppIndexer {
    fn language_id(&self) -> &'static str {
        "cpp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cpp", "cc", "cxx", "hpp", "hxx", "hh"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_cpp::LANGUAGE.into()
    }

    fn extract_imports(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<ImportFact> {
        let mut imports = Vec::new();

        // #include directives
        {
            let include_queries = [&self.import_system_query, &self.import_quote_query];
            for query in &include_queries {
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
                        imports.push(ImportFact {
                            id: String::new(),
                            file: file_path.to_string(),
                            language: "cpp".to_string(),
                            kind: ImportKind::GoImport,
                            source: path.clone(),
                            local_name: Some(path),
                            imported_name: None,
                            resolved_file: None,
                            line,
                            confidence: ConfidenceScore::unresolved(),
                        });
                    }
                }
            }
        }

        // using declarations
        {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(
                &self.import_using_query,
                tree.root_node(),
                source.as_bytes(),
            );
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut line = 0;
                for cap in m.captures {
                    if cap.index == 0 {
                        name = source[cap.node.byte_range()].to_string();
                        line = cap.node.start_position().row + 1;
                    }
                }
                if !name.is_empty() {
                    imports.push(ImportFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "cpp".to_string(),
                        kind: ImportKind::GoImport,
                        source: name.clone(),
                        local_name: Some(name),
                        imported_name: None,
                        resolved_file: None,
                        line,
                        confidence: ConfidenceScore::unresolved(),
                    });
                }
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

        // Functions with parameter lists for signature
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.sym_func_query, tree.root_node(), source.as_bytes());
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
                    let sig = format!("{}{}", name, params_text);
                    let (exported, visibility) = cpp_visibility(def_node, source, false);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "cpp".to_string(),
                        kind: SymbolKind::Function,
                        name,
                        exported,
                        line_start: start,
                        line_end: end,
                        visibility,
                        signature: sig,
                        is_test: false,
                        usage_count: 0,
                        ..Default::default()
                    });
                }
            }
        }

        // Classes
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.sym_class_query, tree.root_node(), source.as_bytes());
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
                    let (exported, visibility) = cpp_visibility(def_node, source, false);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "cpp".to_string(),
                        kind: SymbolKind::Class,
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

        // Structs
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.sym_struct_query, tree.root_node(), source.as_bytes());
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
                    let (exported, visibility) = cpp_visibility(def_node, source, false);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "cpp".to_string(),
                        kind: SymbolKind::Struct,
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

        // Enums
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.sym_enum_query, tree.root_node(), source.as_bytes());
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
                    let (exported, visibility) = cpp_visibility(def_node, source, false);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "cpp".to_string(),
                        kind: SymbolKind::Enum,
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

        // Typedefs
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.sym_typedef_query, tree.root_node(), source.as_bytes());
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
                    let (exported, visibility) = cpp_visibility(def_node, source, false);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "cpp".to_string(),
                        kind: SymbolKind::Type,
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

        // Namespaces
        {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(
                &self.sym_namespace_query,
                tree.root_node(),
                source.as_bytes(),
            );
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
                    let (exported, visibility) = cpp_visibility(def_node, source, false);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "cpp".to_string(),
                        kind: SymbolKind::Module,
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

        // Function calls: foo()
        {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&self.call_query, tree.root_node(), source.as_bytes());
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
                        language: "cpp".to_string(),
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

        // New expressions: new Foo()
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.call_new_query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut line = 0usize;
                let mut col = 0usize;
                for cap in m.captures {
                    match cap.index {
                        0 => name = source[cap.node.byte_range()].to_string(),
                        1 => {
                            line = cap.node.start_position().row + 1;
                            col = cap.node.start_position().column;
                        }
                        _ => {}
                    }
                }
                if !name.is_empty() {
                    calls.push(CallFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "cpp".to_string(),
                        kind: CallKind::ConstructorCall,
                        caller_symbol: None,
                        callee_text: name,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_cpp(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_extract_class() {
        let source = r#"class MyClass {
    int x;
    void do_something();
};"#;
        let tree = parse_cpp(source);
        let indexer = CppIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.cpp");
        let classes: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name, "MyClass");
    }

    #[test]
    fn test_extract_template_class() {
        let source = r#"template <typename T>
class Container {
    T value;
};"#;
        let tree = parse_cpp(source);
        let indexer = CppIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.hpp");
        let classes: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name, "Container");
    }

    #[test]
    fn test_extract_namespace() {
        let source = r#"namespace mylib {
    void func() {}
}"#;
        let tree = parse_cpp(source);
        let indexer = CppIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.cpp");
        let namespaces: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Module)
            .collect();
        assert_eq!(namespaces.len(), 1);
        assert_eq!(namespaces[0].name, "mylib");
    }

    #[test]
    fn test_extract_includes() {
        let source = r#"#include <vector>
#include "myheader.h""#;
        let tree = parse_cpp(source);
        let indexer = CppIndexer::new();
        let imports = indexer.extract_imports(&tree, source, "test.cpp");
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].source, "<vector>");
        assert_eq!(imports[1].source, "\"myheader.h\"");
    }

    #[test]
    fn test_extract_function_signature() {
        let source = r#"int add(int a, int b) { return a + b; }"#;
        let tree = parse_cpp(source);
        let indexer = CppIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.cpp");
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].signature.contains("add"));
        assert!(symbols[0].signature.contains("int a"));
    }
}
