//! Java language indexer using tree-sitter.

use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::{
    languages::visibility::java_family_visibility, CallFact, CallKind, ConfidenceScore, ImportFact,
    ImportKind, LanguageIndexer, SymbolFact, SymbolKind,
};

pub struct JavaIndexer {
    import_query: Query,
    sym_class_query: Query,
    sym_method_query: Query,
    call_ident_query: Query,
    call_attr_query: Query,
}

impl JavaIndexer {
    pub fn new() -> Self {
        let lang: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
        Self {
            import_query: Query::new(&lang, r#"(import_declaration (scoped_identifier) @module) @import"#).expect("valid java import query"),
            sym_class_query: Query::new(&lang, r#"(class_declaration name: (identifier) @name) @def"#).expect("valid java class query"),
            sym_method_query: Query::new(&lang, r#"(method_declaration name: (identifier) @name) @def"#).expect("valid java method query"),
            call_ident_query: Query::new(&lang, r#"(method_invocation name: (identifier) @callee) @call"#).expect("valid java call ident query"),
            call_attr_query: Query::new(&lang, r#"(method_invocation object: (identifier) @object name: (identifier) @method) @call"#).expect("valid java call attr query"),
        }
    }
}

impl Default for JavaIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageIndexer for JavaIndexer {
    fn language_id(&self) -> &'static str {
        "java"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }
    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_java::LANGUAGE.into()
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
                let local_name = module.split('.').next_back().unwrap_or(&module).to_string();
                imports.push(ImportFact {
                    id: String::new(),
                    file: file_path.to_string(),
                    language: "java".to_string(),
                    kind: ImportKind::JavaImport,
                    source: module,
                    local_name: Some(local_name.clone()),
                    imported_name: Some(local_name),
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
            (&self.sym_class_query, SymbolKind::Class),
            (&self.sym_method_query, SymbolKind::Function),
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
                        language: "java".to_string(),
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
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.call_attr_query, tree.root_node(), source.as_bytes());
            while let Some(m) = matches.next() {
                let mut object = String::new();
                let mut method = String::new();
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
                        2 => method = text.to_string(),
                        _ => {}
                    }
                }
                if !method.is_empty() {
                    calls.push(CallFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "java".to_string(),
                        kind: CallKind::MethodCall,
                        caller_symbol: None,
                        callee_text: method,
                        object: Some(object),
                        resolved_symbol: None,
                        line,
                        column: col,
                        confidence: ConfidenceScore::unresolved(),
                    });
                }
            }
        }
        {
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
                        language: "java".to_string(),
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
        calls
    }
}
