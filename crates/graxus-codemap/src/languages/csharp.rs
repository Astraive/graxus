//! C# language indexer using tree-sitter.

use tree_sitter::{Query, QueryCursor, StreamingIterator};

use crate::{
    languages::visibility::csharp_visibility, CallFact, CallKind, ConfidenceScore, ImportFact,
    ImportKind, LanguageIndexer, SymbolFact, SymbolKind,
};

/// C# language indexer with pre-compiled tree-sitter queries.
pub struct CSharpIndexer {
    import_identifier_query: Query,
    import_qualified_query: Query,
    sym_class_query: Query,
    sym_interface_query: Query,
    sym_struct_query: Query,
    sym_enum_query: Query,
    sym_method_query: Query,
    sym_namespace_query: Query,
    sym_record_query: Query,
    call_invoke_query: Query,
    call_member_invoke_query: Query,
    call_new_query: Query,
}

impl CSharpIndexer {
    /// Create a new CSharpIndexer with pre-compiled tree-sitter queries.
    pub fn new() -> Self {
        let lang: tree_sitter::Language = tree_sitter_c_sharp::LANGUAGE.into();
        Self {
            import_identifier_query: Query::new(
                &lang,
                r#"(using_directive (identifier) @name) @use"#,
            )
            .expect("valid csharp using identifier query"),
            import_qualified_query: Query::new(
                &lang,
                r#"(using_directive (qualified_name) @name) @use"#,
            )
            .expect("valid csharp using qualified query"),
            sym_class_query: Query::new(
                &lang,
                r#"(class_declaration name: (identifier) @name) @def"#,
            )
            .expect("valid csharp class query"),
            sym_interface_query: Query::new(
                &lang,
                r#"(interface_declaration name: (identifier) @name) @def"#,
            )
            .expect("valid csharp interface query"),
            sym_struct_query: Query::new(
                &lang,
                r#"(struct_declaration name: (identifier) @name) @def"#,
            )
            .expect("valid csharp struct query"),
            sym_enum_query: Query::new(
                &lang,
                r#"(enum_declaration name: (identifier) @name) @def"#,
            )
            .expect("valid csharp enum query"),
            sym_method_query: Query::new(
                &lang,
                r#"(method_declaration name: (identifier) @name parameters: (parameter_list) @params) @def"#,
            )
            .expect("valid csharp method query"),
            sym_namespace_query: Query::new(
                &lang,
                r#"(namespace_declaration name: (identifier) @name) @def"#,
            )
            .expect("valid csharp namespace query"),
            sym_record_query: Query::new(
                &lang,
                r#"(record_declaration name: (identifier) @name) @def"#,
            )
            .expect("valid csharp record query"),
            call_invoke_query: Query::new(
                &lang,
                r#"(invocation_expression function: (identifier) @callee) @call"#,
            )
            .expect("valid csharp invocation query"),
            call_member_invoke_query: Query::new(
                &lang,
                r#"(invocation_expression function: (member_access_expression) @callee) @call"#,
            )
            .expect("valid csharp member invocation query"),
            call_new_query: Query::new(
                &lang,
                r#"(object_creation_expression type: (identifier) @name) @call"#,
            )
            .expect("valid csharp new expression query"),
        }
    }
}

impl Default for CSharpIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageIndexer for CSharpIndexer {
    fn language_id(&self) -> &'static str {
        "csharp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cs"]
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_c_sharp::LANGUAGE.into()
    }

    fn extract_imports(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<ImportFact> {
        let mut imports = Vec::new();
        let queries = [&self.import_identifier_query, &self.import_qualified_query];
        for query in &queries {
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
                    let local = path.split('.').next_back().unwrap_or(&path).to_string();
                    imports.push(ImportFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "csharp".to_string(),
                        kind: ImportKind::GoImport,
                        source: path,
                        local_name: Some(local),
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
                    let (exported, visibility) = csharp_visibility(def_node, source);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "csharp".to_string(),
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

        // Interfaces
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.sym_interface_query, tree.root_node(), source.as_bytes());
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
                    let (exported, visibility) = csharp_visibility(def_node, source);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "csharp".to_string(),
                        kind: SymbolKind::Interface,
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
                    let (exported, visibility) = csharp_visibility(def_node, source);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "csharp".to_string(),
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
                    let (exported, visibility) = csharp_visibility(def_node, source);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "csharp".to_string(),
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

        // Methods with parameter lists for signature
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.sym_method_query, tree.root_node(), source.as_bytes());
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
                    let (exported, visibility) = csharp_visibility(def_node, source);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "csharp".to_string(),
                        kind: SymbolKind::Method,
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

        // Namespaces
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.sym_namespace_query, tree.root_node(), source.as_bytes());
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
                    let (exported, visibility) = csharp_visibility(def_node, source);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "csharp".to_string(),
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

        // Records
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.sym_record_query, tree.root_node(), source.as_bytes());
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
                    let (exported, visibility) = csharp_visibility(def_node, source);
                    symbols.push(SymbolFact {
                        id: String::new(),
                        file: file_path.to_string(),
                        language: "csharp".to_string(),
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

        symbols
    }

    fn extract_calls(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<CallFact> {
        let mut calls = Vec::new();

        // Simple invocation: Foo()
        {
            let mut cursor = QueryCursor::new();
            let mut matches =
                cursor.matches(&self.call_invoke_query, tree.root_node(), source.as_bytes());
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
                        language: "csharp".to_string(),
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

        // Member invocation: Console.WriteLine()
        {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(
                &self.call_member_invoke_query,
                tree.root_node(),
                source.as_bytes(),
            );
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
                        language: "csharp".to_string(),
                        kind: CallKind::MethodCall,
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

        // Object creation: new Foo()
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
                        language: "csharp".to_string(),
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

    fn parse_csharp(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_c_sharp::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_extract_class() {
        let source = r#"public class MyClass {
    private int x;
    public void DoSomething() {}
}"#;
        let tree = parse_csharp(source);
        let indexer = CSharpIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.cs");
        let classes: Vec<_> = symbols.iter().filter(|s| s.kind == SymbolKind::Class).collect();
        assert!(!classes.is_empty());
        assert_eq!(classes[0].name, "MyClass");
    }

    #[test]
    fn test_extract_interface() {
        let source = r#"public interface ILogger {
    void Log(string message);
}"#;
        let tree = parse_csharp(source);
        let indexer = CSharpIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.cs");
        let interfaces: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Interface)
            .collect();
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].name, "ILogger");
    }

    #[test]
    fn test_extract_using() {
        let source = r#"using System;
using System.Collections.Generic;"#;
        let tree = parse_csharp(source);
        let indexer = CSharpIndexer::new();
        let imports = indexer.extract_imports(&tree, source, "test.cs");
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].source, "System");
        assert_eq!(imports[1].source, "System.Collections.Generic");
    }

    #[test]
    fn test_extract_method_calls() {
        let source = r#"class Program {
    static void Main() {
        Console.WriteLine("goodbye");
        var obj = new MyClass();
    }
}"#;
        let tree = parse_csharp(source);
        let indexer = CSharpIndexer::new();
        let calls = indexer.extract_calls(&tree, source, "test.cs");
        assert!(calls.len() >= 2);
    }

    #[test]
    fn test_extract_method_signature() {
        let source = r#"public class Calculator {
    public int Add(int a, int b) { return a + b; }
}"#;
        let tree = parse_csharp(source);
        let indexer = CSharpIndexer::new();
        let symbols = indexer.extract_symbols(&tree, source, "test.cs");
        let methods: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Method)
            .collect();
        assert_eq!(methods.len(), 1);
        assert!(methods[0].signature.contains("Add"));
        assert!(methods[0].signature.contains("int a"));
    }
}