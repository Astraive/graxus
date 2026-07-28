//! Swift language indexer using tree-sitter.

use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::{
    languages::visibility::java_family_visibility, CallFact, CallKind, ConfidenceScore, ImportFact,
    ImportKind, LanguageIndexer, SymbolFact, SymbolKind,
};

pub struct SwiftIndexer {
    import_query: Query,
    sym_func_query: Query,
    sym_class_query: Query,
    call_ident_query: Query,
}

impl SwiftIndexer {
    pub fn new() -> Self {
        let lang: tree_sitter::Language = tree_sitter_swift::LANGUAGE.into();
        Self {
            import_query: Query::new(&lang, r#"(import_declaration (identifier) @module) @import"#)
                .expect("valid swift import query"),
            sym_func_query: Query::new(&lang, r#"(function_declaration (identifier) @name) @def"#)
                .expect("valid swift func query"),
            sym_class_query: Query::new(&lang, r#"(class_declaration (identifier) @name) @def"#)
                .expect("valid swift class query"),
            call_ident_query: Query::new(&lang, r#"(call_expression (identifier) @callee) @call"#)
                .expect("valid swift call ident query"),
        }
    }
}

impl Default for SwiftIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageIndexer for SwiftIndexer {
    fn language_id(&self) -> &'static str {
        "swift"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["swift"]
    }
    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_swift::LANGUAGE.into()
    }

    fn extract_imports(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<ImportFact> {
        let mut imports = Vec::new();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.import_query, tree.root_node(), source.as_bytes());
        while let Some(m) = matches.next() {
            let mut module = String::new();
            let mut line = 0;
            for cap in m.captures {
                let text = &source[cap.node.byte_range()];
                match cap.index {
                    0 => {
                        module = text.to_string();
                        line = cap.node.start_position().row + 1;
                    }
                    1 => {}
                    _ => {}
                }
            }
            if !module.is_empty() {
                imports.push(ImportFact {
                    id: String::new(),
                    file: file_path.to_string(),
                    language: "swift".to_string(),
                    kind: ImportKind::SwiftImport,
                    source: module.clone(),
                    local_name: Some(module),
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
                    let (exported, visibility) = java_family_visibility(def_node, source);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "swift".to_string(),
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
        let mut cursor = QueryCursor::new();
        let mut matches =
            cursor.matches(&self.call_ident_query, tree.root_node(), source.as_bytes());
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
                    language: "swift".to_string(),
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
        calls
    }
}
