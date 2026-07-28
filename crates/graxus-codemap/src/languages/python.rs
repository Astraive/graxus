//! Python language indexer using tree-sitter.

use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::{
    CallFact, CallKind, ConfidenceScore, ImportFact, ImportKind, LanguageIndexer, SymbolFact,
    SymbolKind, Visibility,
};

/// Python language indexer with pre-compiled tree-sitter queries.
pub struct PythonIndexer {
    import_from_query: Query,
    import_stmt_query: Query,
    sym_func_query: Query,
    sym_class_query: Query,
    call_ident_query: Query,
    call_attr_query: Query,
}

impl PythonIndexer {
    /// Create a new PythonIndexer with pre-compiled tree-sitter queries.
    pub fn new() -> Self {
        let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
        Self {
            import_from_query: Query::new(&lang, r#"(import_from_statement module_name: (dotted_name) @module name: (dotted_name) @name) @import"#).expect("valid python from import query"),
            import_stmt_query: Query::new(&lang, r#"(import_statement name: (dotted_name) @module) @import"#).expect("valid python import query"),
            sym_func_query: Query::new(&lang, r#"(function_definition name: (identifier) @name) @def"#).expect("valid python func query"),
            sym_class_query: Query::new(&lang, r#"(class_definition name: (identifier) @name) @def"#).expect("valid python class query"),
            call_ident_query: Query::new(&lang, r#"(call function: (identifier) @callee) @call"#).expect("valid python call ident query"),
            call_attr_query: Query::new(&lang, r#"(call function: (attribute object: (identifier) @object attribute: (identifier) @property)) @call"#).expect("valid python call attr query"),
        }
    }
}

impl Default for PythonIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageIndexer for PythonIndexer {
    fn language_id(&self) -> &'static str {
        "python"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["py", "pyi"]
    }
    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn extract_imports(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<ImportFact> {
        let mut imports = Vec::new();

        let queries: [(&Query, ImportKind); 2] = [
            (&self.import_from_query, ImportKind::FromImport),
            (&self.import_stmt_query, ImportKind::PythonImport),
        ];

        for (query, kind) in &queries {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut module = String::new();
                let mut name = String::new();
                let mut line = 0;
                for capture in m.captures {
                    let text = &source[capture.node.byte_range()];
                    match capture.index {
                        0 => {
                            module = text.to_string();
                            line = capture.node.start_position().row + 1;
                        }
                        1 => name = text.to_string(),
                        _ => {}
                    }
                }
                if !module.is_empty() {
                    let local = if name.is_empty() {
                        module.split('.').next_back().unwrap_or(&module).to_string()
                    } else {
                        name.clone()
                    };
                    imports.push(ImportFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "python".to_string(),
                        kind: *kind,
                        source: module,
                        local_name: Some(local.clone()),
                        imported_name: if name.is_empty() { None } else { Some(local) },
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
        let queries: [(&Query, SymbolKind); 2] = [
            (&self.sym_func_query, SymbolKind::Function),
            (&self.sym_class_query, SymbolKind::Class),
        ];
        for (query, kind) in &queries {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut name = String::new();
                let mut start = 0usize;
                let mut end = 0usize;
                for cap in m.captures {
                    match cap.index {
                        0 => name = source[cap.node.byte_range()].to_string(),
                        1 => {
                            start = cap.node.start_position().row + 1;
                            end = cap.node.end_position().row + 1;
                        }
                        _ => {}
                    }
                }
                if !name.is_empty() {
                    let exported = !name.starts_with('_');
                    let is_test = name.starts_with("test_") || name.starts_with("Test");
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "python".to_string(),
                        kind: *kind,
                        name,
                        exported,
                        line_start: start,
                        line_end: end,
                        visibility: if exported {
                            Visibility::Public
                        } else {
                            Visibility::Private
                        },
                        signature: String::new(),
                        is_test,
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
                        language: "python".to_string(),
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
        // Attribute: obj.method()
        {
            let query = &self.call_attr_query;
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut object = String::new();
                let mut property = String::new();
                let mut line = 0usize;
                let mut col = 0usize;
                for cap in m.captures {
                    let text = &source[cap.node.byte_range()];
                    match cap.index {
                        0 => {
                            line = cap.node.start_position().row + 1;
                            col = cap.node.start_position().column;
                        }
                        1 => object = text.to_string(),
                        2 => property = text.to_string(),
                        _ => {}
                    }
                }
                if !property.is_empty() {
                    calls.push(CallFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "python".to_string(),
                        kind: CallKind::MethodCall,
                        caller_symbol: None,
                        callee_text: property,
                        object: Some(object),
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
