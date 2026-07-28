//! Serialize Graxus graphs to D3-compatible JSON format.
//!
//! Each function converts a specific graph type (docs, code, impact, bridge,
//! deps) into a [`D3Graph`] that can be rendered with [`crate::template::render_html`].

use crate::{D3Graph, D3Link, D3Node};
use graxus_docgraph::graph::DocGraph;

/// Serialize a [`DocGraph`] into a D3 graph.
///
/// Each document becomes a node of type `"doc"`. Wiki links, tags, and
/// other edge types are preserved as directed links.
pub fn doc_graph_to_d3(graph: &DocGraph) -> D3Graph {
    let mut d3 = D3Graph::new(
        "Documentation Graph",
        "Wiki links, tags, and backlinks between docs",
    );

    for node in &graph.nodes {
        d3.nodes.push(D3Node {
            id: node.id.clone(),
            label: node.title.clone(),
            node_type: "doc".to_string(),
            file: Some(node.path.clone()),
            line: None,
            details: if node.tags.is_empty() {
                None
            } else {
                Some(format!("Tags: {}", node.tags.join(", ")))
            },
        });
    }

    for edge in &graph.edges {
        d3.links.push(D3Link {
            source: edge.from.clone(),
            target: edge.to.clone(),
            edge_type: format!("{:?}", edge.edge_type),
            label: None,
        });
    }

    d3
}

/// Serialize a code graph (as a JSON [`serde_json::Value`]) into a D3 graph.
///
/// Expects the JSON to contain `"files"`, `"symbols"`, and optionally `"calls"`
/// arrays. Files become nodes of type `"file"`, symbols become typed nodes
/// (function, struct, enum, etc.), and calls become directed links.
pub fn code_graph_to_d3(graph: &serde_json::Value) -> D3Graph {
    let mut d3 = D3Graph::new(
        "Code Codemap",
        "Symbols, imports, and calls in the codebase",
    );

    if let Some(files) = graph.get("files").and_then(|v| v.as_array()) {
        for file in files {
            let path = file.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let lang = file.get("language").and_then(|v| v.as_str()).unwrap_or("?");
            d3.nodes.push(D3Node {
                id: path.to_string(),
                label: path.split('/').next_back().unwrap_or(path).to_string(),
                node_type: "file".to_string(),
                file: Some(path.to_string()),
                line: None,
                details: Some(format!("Language: {}", lang)),
            });
        }
    }

    if let Some(symbols) = graph.get("symbols").and_then(|v| v.as_array()) {
        for sym in symbols {
            let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
            let file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("?");
            let line = sym.get("line_start").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let sig = sym.get("signature").and_then(|v| v.as_str()).unwrap_or("");

            let node_type = match kind {
                "function" | "Function" => "function",
                "class" | "Class" => "class",
                "struct" | "Struct" => "struct",
                "enum" | "Enum" => "enum",
                "trait" | "Trait" => "trait",
                "interface" | "Interface" => "interface",
                "type" | "Type" => "type",
                "constant" | "Constant" => "constant",
                "module" | "Module" => "module",
                "method" | "Method" => "method",
                _ => "other",
            };

            let sig_str = if sig.is_empty() {
                String::new()
            } else {
                format!("\n{}", sig)
            };
            d3.nodes.push(D3Node {
                id: format!("symbol:{}:{}", file, name),
                label: name.to_string(),
                node_type: node_type.to_string(),
                file: Some(file.to_string()),
                line: Some(line),
                details: Some(format!("{} in {}{}", kind, file, sig_str)),
            });

            d3.links.push(D3Link {
                source: file.to_string(),
                target: format!("symbol:{}:{}", file, name),
                edge_type: "defines".to_string(),
                label: None,
            });
        }
    }

    if let Some(calls) = graph.get("calls").and_then(|v| v.as_array()) {
        for call in calls {
            let file = call.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let callee = call
                .get("callee_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let caller = call
                .get("caller_symbol")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let resolved = call
                .get("resolved_symbol")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let from_id = if caller.is_empty() {
                file.to_string()
            } else {
                format!("symbol:{}:{}", file, caller)
            };
            let to_id = if resolved.is_empty() {
                format!("call:{}", callee)
            } else {
                resolved.to_string()
            };

            d3.links.push(D3Link {
                source: from_id,
                target: to_id,
                edge_type: "calls".to_string(),
                label: Some(callee.to_string()),
            });
        }
    }

    d3
}

/// Serialize blast radius (impact analysis) for a specific symbol from JSON.
///
/// Shows the target symbol and all direct callers that reference it,
/// forming a star topology centred on the target.
pub fn blast_radius_to_d3(
    graph: &serde_json::Value,
    target_symbol: &str,
    _max_depth: usize,
) -> D3Graph {
    let mut d3 = D3Graph::new(
        &format!("Impact: {}", target_symbol),
        "Blast radius — transitive callers of this symbol",
    );

    let mut target_file = String::new();
    if let Some(symbols) = graph.get("symbols").and_then(|v| v.as_array()) {
        for sym in symbols {
            if sym.get("name").and_then(|v| v.as_str()) == Some(target_symbol) {
                target_file = sym
                    .get("file")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                break;
            }
        }
    }

    let target_id = format!("symbol:{}:{}", target_file, target_symbol);
    d3.nodes.push(D3Node {
        id: target_id.clone(),
        label: target_symbol.to_string(),
        node_type: "target".to_string(),
        file: Some(target_file),
        line: None,
        details: Some("Target symbol".to_string()),
    });

    if let Some(calls) = graph.get("calls").and_then(|v| v.as_array()) {
        for call in calls {
            if call.get("callee_text").and_then(|v| v.as_str()) == Some(target_symbol) {
                let file = call.get("file").and_then(|v| v.as_str()).unwrap_or("");
                let caller = call
                    .get("caller_symbol")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(unknown)");
                let line = call.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let caller_id = format!("symbol:{}:{}", file, caller);

                d3.nodes.push(D3Node {
                    id: caller_id.clone(),
                    label: caller.to_string(),
                    node_type: "caller".to_string(),
                    file: Some(file.to_string()),
                    line: Some(line),
                    details: Some(format!("Calls {} at line {}", target_symbol, line)),
                });
                d3.links.push(D3Link {
                    source: caller_id,
                    target: target_id.clone(),
                    edge_type: "calls".to_string(),
                    label: None,
                });
            }
        }
    }

    d3
}

/// Serialize a docs-code bridge as a D3 graph.
///
/// Combines doc nodes and code symbols into a single graph with bridge
/// edges that link documentation to source code. Confidence scores are
/// shown as edge labels.
pub fn bridge_to_d3(
    doc_graph: &DocGraph,
    code_graph: &serde_json::Value,
    bridge: &[graxus_agent_api::BridgeEdge],
) -> D3Graph {
    let mut d3 = D3Graph::new(
        "Docs-Code Bridge",
        "Connections between documentation and source code",
    );

    for node in &doc_graph.nodes {
        d3.nodes.push(D3Node {
            id: node.id.clone(),
            label: node.title.clone(),
            node_type: "doc".to_string(),
            file: Some(node.path.clone()),
            line: None,
            details: None,
        });
    }

    if let Some(symbols) = code_graph.get("symbols").and_then(|v| v.as_array()) {
        for sym in symbols {
            let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("?");
            let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
            d3.nodes.push(D3Node {
                id: format!("symbol:{}:{}", file, name),
                label: name.to_string(),
                node_type: "symbol".to_string(),
                file: Some(file.to_string()),
                line: None,
                details: Some(kind.to_string()),
            });
        }
    }

    for edge in bridge {
        d3.links.push(D3Link {
            source: edge.from.clone(),
            target: edge.to.clone(),
            edge_type: format!("{:?}", edge.edge_type),
            label: Some(format!("{:.0}%", edge.confidence.score)),
        });
    }

    d3
}

/// Serialize a dependency graph as a D3 graph.
///
/// Files become nodes and imports become directed links. Unresolved imports
/// (empty `resolved_file`) are skipped.
pub fn deps_to_d3(graph: &serde_json::Value) -> D3Graph {
    let mut d3 = D3Graph::new("Dependency Graph", "File-to-file import relationships");

    if let Some(files) = graph.get("files").and_then(|v| v.as_array()) {
        for file in files {
            let path = file.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let lang = file.get("language").and_then(|v| v.as_str()).unwrap_or("?");
            d3.nodes.push(D3Node {
                id: path.to_string(),
                label: path.split('/').next_back().unwrap_or(path).to_string(),
                node_type: "file".to_string(),
                file: Some(path.to_string()),
                line: None,
                details: Some(format!("Language: {}", lang)),
            });
        }
    }

    if let Some(imports) = graph.get("imports").and_then(|v| v.as_array()) {
        for imp in imports {
            let file = imp.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = imp
                .get("resolved_file")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let local = imp.get("local_name").and_then(|v| v.as_str()).unwrap_or("");
            if !resolved.is_empty() {
                d3.links.push(D3Link {
                    source: file.to_string(),
                    target: resolved.to_string(),
                    edge_type: "imports".to_string(),
                    label: Some(local.to_string()),
                });
            }
        }
    }

    d3
}

#[cfg(test)]
mod tests {
    use super::*;
    use graxus_agent_api::{BridgeEdge, BridgeEdgeType};
    use graxus_codemap::{ConfidenceScore, ResolutionMethod};
    use graxus_docgraph::graph::{DocEdge, DocEdgeType, DocNode, DocNodeType};

    fn make_doc_node(id: &str, title: &str, path: &str, tags: Vec<String>) -> DocNode {
        DocNode {
            id: id.into(),
            node_type: DocNodeType::Document,
            path: path.into(),
            title: title.into(),
            tags,
            frontmatter: None,
            headings: Vec::new(),
            wiki_links: Vec::new(),
        }
    }

    // ── doc_graph_to_d3 ─────────────────────────────────────────────

    #[test]
    fn test_doc_graph_to_d3_empty() {
        let doc = DocGraph::new();
        let d3 = doc_graph_to_d3(&doc);
        assert_eq!(d3.title, "Documentation Graph");
        assert!(d3.nodes.is_empty());
        assert!(d3.links.is_empty());
    }

    #[test]
    fn test_doc_graph_to_d3_with_nodes() {
        let mut doc = DocGraph::new();
        doc.nodes.push(make_doc_node(
            "doc:a",
            "Getting Started",
            "docs/a.md",
            vec!["rust".into()],
        ));
        doc.nodes
            .push(make_doc_node("doc:b", "API Reference", "docs/b.md", vec![]));
        doc.edges.push(DocEdge {
            from: "doc:a".into(),
            to: "doc:b".into(),
            edge_type: DocEdgeType::LinksTo,
        });

        let d3 = doc_graph_to_d3(&doc);
        assert_eq!(d3.nodes.len(), 2);
        assert_eq!(d3.links.len(), 1);

        assert_eq!(d3.nodes[0].id, "doc:a");
        assert_eq!(d3.nodes[0].label, "Getting Started");
        assert_eq!(d3.nodes[0].node_type, "doc");
        assert_eq!(d3.nodes[0].file.as_deref(), Some("docs/a.md"));
        assert_eq!(d3.nodes[0].details.as_deref(), Some("Tags: rust"));

        assert_eq!(d3.nodes[1].details, None); // no tags

        assert_eq!(d3.links[0].source, "doc:a");
        assert_eq!(d3.links[0].target, "doc:b");
        assert!(d3.links[0].edge_type.contains("LinksTo"));
    }

    // ── code_graph_to_d3 ────────────────────────────────────────────

    #[test]
    fn test_code_graph_to_d3_empty_json() {
        let json = serde_json::json!({});
        let d3 = code_graph_to_d3(&json);
        assert_eq!(d3.title, "Code Codemap");
        assert!(d3.nodes.is_empty());
        assert!(d3.links.is_empty());
    }

    #[test]
    fn test_code_graph_to_d3_files_and_symbols() {
        let json = serde_json::json!({
            "files": [
                { "path": "src/main.rs", "language": "rust" }
            ],
            "symbols": [
                { "name": "main", "kind": "function", "file": "src/main.rs", "line_start": 1, "signature": "fn main()" },
                { "name": "Config", "kind": "Struct", "file": "src/main.rs", "line_start": 10, "signature": "" }
            ],
            "calls": []
        });

        let d3 = code_graph_to_d3(&json);
        // 1 file + 2 symbols = 3 nodes
        assert_eq!(d3.nodes.len(), 3);
        // 2 "defines" links (file -> symbol)
        assert_eq!(d3.links.len(), 2);

        // File node
        assert_eq!(d3.nodes[0].node_type, "file");
        assert_eq!(d3.nodes[0].label, "main.rs");

        // Function symbol
        assert_eq!(d3.nodes[1].node_type, "function");
        assert_eq!(d3.nodes[1].label, "main");
        assert_eq!(d3.nodes[1].line, Some(1));

        // Struct symbol (case-insensitive kind mapping)
        assert_eq!(d3.nodes[2].node_type, "struct");
        assert_eq!(d3.nodes[2].label, "Config");
    }

    #[test]
    fn test_code_graph_to_d3_with_calls() {
        let json = serde_json::json!({
            "files": [{ "path": "src/lib.rs", "language": "rust" }],
            "symbols": [
                { "name": "foo", "kind": "function", "file": "src/lib.rs", "line_start": 1, "signature": "" },
                { "name": "bar", "kind": "function", "file": "src/lib.rs", "line_start": 5, "signature": "" }
            ],
            "calls": [
                { "file": "src/lib.rs", "caller_symbol": "foo", "callee_text": "bar", "resolved_symbol": "symbol:src/lib.rs:bar", "line": 3 }
            ]
        });

        let d3 = code_graph_to_d3(&json);
        // 1 file + 2 symbols = 3 nodes, 2 defines + 1 call = 3 links
        assert_eq!(d3.nodes.len(), 3);
        assert_eq!(d3.links.len(), 3);

        let call_link = d3.links.iter().find(|l| l.edge_type == "calls").unwrap();
        assert_eq!(call_link.source, "symbol:src/lib.rs:foo");
        assert_eq!(call_link.target, "symbol:src/lib.rs:bar");
        assert_eq!(call_link.label.as_deref(), Some("bar"));
    }

    #[test]
    fn test_code_graph_to_d3_unknown_symbol_kind() {
        let json = serde_json::json!({
            "files": [],
            "symbols": [
                { "name": "X", "kind": "Macro", "file": "src/m.rs", "line_start": 1, "signature": "" }
            ],
            "calls": []
        });

        let d3 = code_graph_to_d3(&json);
        assert_eq!(d3.nodes[0].node_type, "other");
    }

    // ── blast_radius_to_d3 ──────────────────────────────────────────

    #[test]
    fn test_blast_radius_to_d3_no_callers() {
        let json = serde_json::json!({
            "symbols": [{ "name": "target_fn", "kind": "function", "file": "src/lib.rs", "line_start": 1, "signature": "" }],
            "calls": []
        });

        let d3 = blast_radius_to_d3(&json, "target_fn", 3);
        assert_eq!(d3.title, "Impact: target_fn");
        assert_eq!(d3.nodes.len(), 1);
        assert_eq!(d3.nodes[0].node_type, "target");
        assert_eq!(d3.nodes[0].label, "target_fn");
        assert!(d3.links.is_empty());
    }

    #[test]
    fn test_blast_radius_to_d3_with_callers() {
        let json = serde_json::json!({
            "symbols": [
                { "name": "target_fn", "kind": "function", "file": "src/lib.rs", "line_start": 10, "signature": "" }
            ],
            "calls": [
                { "file": "src/a.rs", "caller_symbol": "caller_a", "callee_text": "target_fn", "resolved_symbol": "", "line": 5 },
                { "file": "src/b.rs", "caller_symbol": "caller_b", "callee_text": "target_fn", "resolved_symbol": "", "line": 12 }
            ]
        });

        let d3 = blast_radius_to_d3(&json, "target_fn", 3);
        assert_eq!(d3.nodes.len(), 3); // target + 2 callers
        assert_eq!(d3.links.len(), 2);

        assert_eq!(d3.nodes[0].node_type, "target");
        assert_eq!(d3.nodes[1].node_type, "caller");
        assert_eq!(d3.nodes[1].label, "caller_a");
        assert_eq!(d3.nodes[2].label, "caller_b");

        // All links point to the target
        for link in &d3.links {
            assert_eq!(link.target, "symbol:src/lib.rs:target_fn");
            assert_eq!(link.edge_type, "calls");
        }
    }

    #[test]
    fn test_blast_radius_to_d3_target_not_found() {
        let json = serde_json::json!({
            "symbols": [],
            "calls": []
        });

        let d3 = blast_radius_to_d3(&json, "missing", 3);
        assert_eq!(d3.nodes.len(), 1);
        assert_eq!(d3.nodes[0].id, "symbol::missing");
    }

    // ── bridge_to_d3 ────────────────────────────────────────────────

    #[test]
    fn test_bridge_to_d3_empty() {
        let doc = DocGraph::new();
        let code = serde_json::json!({ "symbols": [] });
        let bridge: Vec<BridgeEdge> = Vec::new();
        let d3 = bridge_to_d3(&doc, &code, &bridge);
        assert_eq!(d3.title, "Docs-Code Bridge");
        assert!(d3.nodes.is_empty());
        assert!(d3.links.is_empty());
    }

    #[test]
    fn test_bridge_to_d3_with_edges() {
        let mut doc = DocGraph::new();
        doc.nodes
            .push(make_doc_node("doc:readme", "README", "README.md", vec![]));

        let code = serde_json::json!({
            "symbols": [
                { "name": "Config", "kind": "struct", "file": "src/config.rs", "line_start": 1, "signature": "" }
            ]
        });

        let bridge = vec![BridgeEdge {
            from: "doc:readme".into(),
            to: "Config".into(),
            edge_type: BridgeEdgeType::DocReferencesSymbol,
            confidence: ConfidenceScore::new(85.0, ResolutionMethod::PathMatchOnly),
        }];

        let d3 = bridge_to_d3(&doc, &code, &bridge);
        // 1 doc + 1 symbol = 2 nodes
        assert_eq!(d3.nodes.len(), 2);
        assert_eq!(d3.links.len(), 1);

        assert_eq!(d3.nodes[0].node_type, "doc");
        assert_eq!(d3.nodes[1].node_type, "symbol");
        assert_eq!(d3.links[0].label.as_deref(), Some("85%"));
    }

    // ── deps_to_d3 ──────────────────────────────────────────────────

    #[test]
    fn test_deps_to_d3_empty() {
        let json = serde_json::json!({});
        let d3 = deps_to_d3(&json);
        assert_eq!(d3.title, "Dependency Graph");
        assert!(d3.nodes.is_empty());
        assert!(d3.links.is_empty());
    }

    #[test]
    fn test_deps_to_d3_with_files_and_imports() {
        let json = serde_json::json!({
            "files": [
                { "path": "src/main.rs", "language": "rust" },
                { "path": "src/lib.rs", "language": "rust" }
            ],
            "imports": [
                { "file": "src/main.rs", "resolved_file": "src/lib.rs", "local_name": "my_lib" },
                { "file": "src/main.rs", "resolved_file": "", "local_name": "unresolved" }
            ]
        });

        let d3 = deps_to_d3(&json);
        assert_eq!(d3.nodes.len(), 2);
        // Only resolved import produces a link
        assert_eq!(d3.links.len(), 1);

        assert_eq!(d3.links[0].source, "src/main.rs");
        assert_eq!(d3.links[0].target, "src/lib.rs");
        assert_eq!(d3.links[0].edge_type, "imports");
        assert_eq!(d3.links[0].label.as_deref(), Some("my_lib"));
    }

    #[test]
    fn test_deps_to_d3_node_details() {
        let json = serde_json::json!({
            "files": [{ "path": "crates/foo/src/lib.rs", "language": "rust" }],
            "imports": []
        });

        let d3 = deps_to_d3(&json);
        assert_eq!(d3.nodes[0].label, "lib.rs"); // last path segment
        assert_eq!(d3.nodes[0].details.as_deref(), Some("Language: rust"));
    }

    // ── Edge cases and robustness ────────────────────────────────────

    #[test]
    fn test_code_graph_to_d3_missing_optional_fields() {
        let json = serde_json::json!({
            "files": [{ "path": "src/x.rs" }],
            "symbols": [{ "name": "S" }],
            "calls": []
        });

        let d3 = code_graph_to_d3(&json);
        // Should not panic — defaults used
        assert_eq!(d3.nodes.len(), 2);
        assert_eq!(d3.nodes[0].details.as_deref(), Some("Language: ?"));
    }

    #[test]
    fn test_code_graph_to_d3_call_with_empty_caller() {
        let json = serde_json::json!({
            "files": [],
            "symbols": [],
            "calls": [
                { "file": "src/main.rs", "caller_symbol": "", "callee_text": "print", "resolved_symbol": "" }
            ]
        });

        let d3 = code_graph_to_d3(&json);
        assert_eq!(d3.links.len(), 1);
        assert_eq!(d3.links[0].source, "src/main.rs");
        assert_eq!(d3.links[0].target, "call:print");
    }

    #[test]
    fn test_all_symbol_kinds_map_correctly() {
        let kinds = [
            ("function", "function"),
            ("Function", "function"),
            ("class", "class"),
            ("Class", "class"),
            ("struct", "struct"),
            ("Struct", "struct"),
            ("enum", "enum"),
            ("Enum", "enum"),
            ("trait", "trait"),
            ("Trait", "trait"),
            ("interface", "interface"),
            ("Interface", "interface"),
            ("type", "type"),
            ("Type", "type"),
            ("constant", "constant"),
            ("Constant", "constant"),
            ("module", "module"),
            ("Module", "module"),
            ("method", "method"),
            ("Method", "method"),
            ("unknown_kind", "other"),
        ];

        for (input, expected) in &kinds {
            let json = serde_json::json!({
                "files": [],
                "symbols": [{ "name": "X", "kind": input, "file": "f.rs", "line_start": 1, "signature": "" }],
                "calls": []
            });
            let d3 = code_graph_to_d3(&json);
            assert_eq!(
                d3.nodes[0].node_type, *expected,
                "Kind '{}' should map to '{}', got '{}'",
                input, expected, d3.nodes[0].node_type
            );
        }
    }
}
