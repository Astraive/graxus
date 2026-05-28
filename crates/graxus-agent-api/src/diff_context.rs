//! Differential context — change-aware queries for AI agents.

use graxus_codemap::CodeGraph;
use graxus_docgraph::graph::DocGraph;
use serde::{Deserialize, Serialize};

use crate::bridge::BridgeEdge;

/// Differential context: what an agent needs to know about uncommitted changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffContext {
    pub changed_files: Vec<String>,
    pub affected_symbols: Vec<String>,
    pub affected_imports: Vec<String>,
    pub related_docs: Vec<String>,
    pub callers: Vec<String>,
    pub warnings: Vec<String>,
}

/// Parse a unified diff and extract changed file paths.
pub fn parse_diff_paths(diff: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in diff.lines() {
        // Unified diff format: +++ b/path/to/file or --- a/path/to/file
        if let Some(rest) = line.strip_prefix("+++ b/") {
            let path = rest.trim();
            if !path.is_empty() && path != "/dev/null" {
                paths.push(path.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("--- a/") {
            let path = rest.trim();
            if !path.is_empty() && path != "/dev/null" && !paths.contains(&path.to_string()) {
                paths.push(path.to_string());
            }
        }
    }
    paths
}

/// Given changed files and a CodeGraph, find all symbols defined in those files.
pub fn affected_symbols(files: &[String], code_graph: &CodeGraph) -> Vec<String> {
    let mut symbols = Vec::new();
    for file in files {
        for sym in code_graph.symbols_in_file(file) {
            if !symbols.contains(&sym.name) {
                symbols.push(sym.name.clone());
            }
        }
    }
    symbols
}

/// Given affected symbols, find all callers — who calls these symbols?
pub fn affected_callers(symbols: &[String], code_graph: &CodeGraph) -> Vec<String> {
    let mut callers = Vec::new();
    for call in &code_graph.calls {
        // Check if this call resolves to one of our affected symbols
        if let Some(ref resolved) = call.resolved_symbol {
            // resolved is "symbol:file:name" format, extract the name
            if let Some(name) = resolved.split("::").last().or_else(|| resolved.split(':').last()) {
                if symbols.iter().any(|s| s == name) {
                    let caller = call
                        .caller_symbol
                        .clone()
                        .unwrap_or_else(|| format!("{}:{}", call.file, call.line));
                    if !callers.contains(&caller) {
                        callers.push(caller);
                    }
                }
            }
        }
        // Also check by callee_text
        if symbols.contains(&call.callee_text) {
            let caller = call
                .caller_symbol
                .clone()
                .unwrap_or_else(|| format!("{}:{}", call.file, call.line));
            if !callers.contains(&caller) {
                callers.push(caller);
            }
        }
    }
    callers
}

/// Find docs that reference any of the changed files or affected symbols.
pub fn affected_docs(
    files: &[String],
    symbols: &[String],
    doc_graph: &DocGraph,
    bridge: &[BridgeEdge],
) -> Vec<String> {
    let mut docs = Vec::new();

    // Check bridge edges
    for edge in bridge {
        let matches_file = files.iter().any(|f| edge.to == *f || edge.from == *f);
        let matches_symbol = symbols.iter().any(|s| edge.to == *s);
        if matches_file || matches_symbol {
            // Add the doc side of the edge
            if !docs.contains(&edge.from) {
                docs.push(edge.from.clone());
            }
        }
    }

    // Check doc paths that contain changed file names
    for file in files {
        let file_stem = std::path::Path::new(file)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if !file_stem.is_empty() {
            for node in &doc_graph.nodes {
                if node.path.contains(&file_stem) || node.title.to_lowercase().contains(&file_stem.to_lowercase()) {
                    if !docs.contains(&node.id) {
                        docs.push(node.id.clone());
                    }
                }
            }
        }
    }

    docs
}

/// Find imports that reference changed files.
pub fn affected_imports(files: &[String], code_graph: &CodeGraph) -> Vec<String> {
    let mut imports = Vec::new();
    for import in &code_graph.imports {
        if let Some(ref resolved) = import.resolved_file {
            if files.contains(resolved) {
                let entry = format!("{}:{}", import.file, import.local_name.as_deref().unwrap_or(&import.source));
                if !imports.contains(&entry) {
                    imports.push(entry);
                }
            }
        }
    }
    imports
}

/// Build full differential context from changed files.
pub fn build_diff_context(
    changed_files: &[String],
    code_graph: &CodeGraph,
    doc_graph: &DocGraph,
    bridge: &[BridgeEdge],
) -> DiffContext {
    let affected_syms = affected_symbols(changed_files, code_graph);
    let callers = affected_callers(&affected_syms, code_graph);
    let related_docs = affected_docs(changed_files, &affected_syms, doc_graph, bridge);
    let affected_imps = affected_imports(changed_files, code_graph);

    let mut warnings = Vec::new();
    if affected_syms.is_empty() {
        warnings.push("No symbols found in changed files".to_string());
    }
    if related_docs.is_empty() {
        warnings.push("No related documentation found for changes".to_string());
    }

    DiffContext {
        changed_files: changed_files.to_vec(),
        affected_symbols: affected_syms,
        affected_imports: affected_imps,
        related_docs,
        callers,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graxus_codemap::{ConfidenceScore, ResolutionMethod};

    #[test]
    fn test_parse_diff_paths() {
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("hello");
 }
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
"#;
        let paths = parse_diff_paths(diff);
        assert!(paths.len() >= 2, "Expected at least 2 paths, got {}", paths.len());
        assert!(paths.contains(&"src/main.rs".to_string()));
        assert!(paths.contains(&"src/lib.rs".to_string()));
    }

    #[test]
    fn test_parse_diff_paths_empty() {
        let paths = parse_diff_paths("");
        assert!(paths.is_empty());
    }

    #[test]
    fn test_affected_symbols() {
        let code_graph = graxus_codemap::CodeGraph {
            files: vec![graxus_codemap::FileNode {
                path: "src/main.rs".into(),
                language: "rust".into(),
                hash: "abc".into(),
                size: 100,
            }],
            symbols: vec![
                graxus_codemap::SymbolFact {
                    id: "sym1".into(),
                    file: "src/main.rs".into(),
                    language: "rust".into(),
                    kind: graxus_codemap::SymbolKind::Function,
                    name: "main".into(),
                    exported: true,
                    line_start: 1,
                    line_end: 5,
                    visibility: graxus_codemap::Visibility::Public,
                    signature: "fn main()".into(),
                    is_test: false,
                    usage_count: 0,
                },
                graxus_codemap::SymbolFact {
                    id: "sym2".into(),
                    file: "src/lib.rs".into(),
                    language: "rust".into(),
                    kind: graxus_codemap::SymbolKind::Function,
                    name: "helper".into(),
                    exported: false,
                    line_start: 1,
                    line_end: 3,
                    visibility: graxus_codemap::Visibility::Private,
                    signature: "fn helper()".into(),
                    is_test: false,
                    usage_count: 0,
                },
            ],
            imports: vec![],
            calls: vec![],
            edges: vec![],
            type_hints: vec![],
        };

        let files = vec!["src/main.rs".to_string()];
        let syms = affected_symbols(&files, &code_graph);
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0], "main");
    }

    #[test]
    fn test_affected_callers() {
        let code_graph = graxus_codemap::CodeGraph {
            files: vec![],
            symbols: vec![],
            imports: vec![],
            calls: vec![graxus_codemap::CallFact {
                id: "call1".into(),
                file: "src/app.rs".into(),
                language: "rust".into(),
                kind: graxus_codemap::CallKind::FunctionCall,
                caller_symbol: Some("app::run".into()),
                callee_text: "main".into(),
                object: None,
                resolved_symbol: Some("symbol:src/main.rs:main".into()),
                line: 42,
                column: 8,
                confidence: graxus_codemap::ConfidenceScore::new(85.0, ResolutionMethod::PathMatchOnly),
            }],
            edges: vec![],
            type_hints: vec![],
        };

        let symbols = vec!["main".to_string()];
        let callers = affected_callers(&symbols, &code_graph);
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0], "app::run");
    }

    #[test]
    fn test_affected_callers_by_callee_text() {
        let code_graph = graxus_codemap::CodeGraph {
            files: vec![],
            symbols: vec![],
            imports: vec![],
            calls: vec![graxus_codemap::CallFact {
                id: "call1".into(),
                file: "src/app.rs".into(),
                language: "rust".into(),
                kind: graxus_codemap::CallKind::FunctionCall,
                caller_symbol: None,
                callee_text: "process_data".into(),
                object: None,
                resolved_symbol: None,
                line: 10,
                column: 5,
                confidence: graxus_codemap::ConfidenceScore::new(40.0, ResolutionMethod::FuzzySymbolMatch),
            }],
            edges: vec![],
            type_hints: vec![],
        };

        let symbols = vec!["process_data".to_string()];
        let callers = affected_callers(&symbols, &code_graph);
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0], "src/app.rs:10");
    }

    #[test]
    fn test_affected_imports() {
        let code_graph = graxus_codemap::CodeGraph {
            files: vec![],
            symbols: vec![],
            imports: vec![graxus_codemap::ImportFact {
                id: "imp1".into(),
                file: "src/main.rs".into(),
                language: "rust".into(),
                kind: graxus_codemap::ImportKind::RustUse,
                source: "crate::lib".into(),
                local_name: Some("lib".into()),
                imported_name: None,
                resolved_file: Some("src/lib.rs".into()),
                line: 1,
                confidence: graxus_codemap::ConfidenceScore::new(85.0, ResolutionMethod::PathMatchOnly),
            }],
            calls: vec![],
            edges: vec![],
            type_hints: vec![],
        };

        let files = vec!["src/lib.rs".to_string()];
        let imps = affected_imports(&files, &code_graph);
        assert_eq!(imps.len(), 1);
        assert!(imps[0].contains("lib"));
    }

    #[test]
    fn test_build_diff_context_empty() {
        let code_graph = CodeGraph::new();
        let doc_graph = DocGraph::new();
        let ctx = build_diff_context(&[], &code_graph, &doc_graph, &[]);
        assert!(ctx.changed_files.is_empty());
        assert!(ctx.affected_symbols.is_empty());
        assert!(ctx.warnings.iter().any(|w| w.contains("No symbols")));
    }
}
