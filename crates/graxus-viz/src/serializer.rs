//! Serialize Graxus graphs to D3-compatible JSON format.

use crate::{D3Graph, D3Link, D3Node};
use graxus_docgraph::graph::DocGraph;

/// Serialize a DocGraph into a D3 graph.
pub fn doc_graph_to_d3(graph: &DocGraph) -> D3Graph {
    let mut d3 = D3Graph::new("Documentation Graph", "Wiki links, tags, and backlinks between docs");

    for node in &graph.nodes {
        d3.nodes.push(D3Node {
            id: node.id.clone(),
            label: node.title.clone(),
            node_type: "doc".to_string(),
            file: Some(node.path.clone()),
            line: None,
            details: if node.tags.is_empty() { None } else { Some(format!("Tags: {}", node.tags.join(", "))) },
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

/// Serialize a CodeGraph (as JSON Value) into a D3 graph.
pub fn code_graph_to_d3(graph: &serde_json::Value) -> D3Graph {
    let mut d3 = D3Graph::new("Code Codemap", "Symbols, imports, and calls in the codebase");

    if let Some(files) = graph.get("files").and_then(|v| v.as_array()) {
        for file in files {
            let path = file.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let lang = file.get("language").and_then(|v| v.as_str()).unwrap_or("?");
            d3.nodes.push(D3Node {
                id: path.to_string(),
                label: path.split('/').last().unwrap_or(path).to_string(),
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

            let sig_str = if sig.is_empty() { String::new() } else { format!("\n{}", sig) };
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
            let callee = call.get("callee_text").and_then(|v| v.as_str()).unwrap_or("");
            let caller = call.get("caller_symbol").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = call.get("resolved_symbol").and_then(|v| v.as_str()).unwrap_or("");

            let from_id = if caller.is_empty() { file.to_string() } else { format!("symbol:{}:{}", file, caller) };
            let to_id = if resolved.is_empty() { format!("call:{}", callee) } else { resolved.to_string() };

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

/// Serialize blast radius for a specific symbol from JSON.
pub fn blast_radius_to_d3(graph: &serde_json::Value, target_symbol: &str, _max_depth: usize) -> D3Graph {
    let mut d3 = D3Graph::new(
        &format!("Impact: {}", target_symbol),
        "Blast radius — transitive callers of this symbol",
    );

    let mut target_file = String::new();
    if let Some(symbols) = graph.get("symbols").and_then(|v| v.as_array()) {
        for sym in symbols {
            if sym.get("name").and_then(|v| v.as_str()) == Some(target_symbol) {
                target_file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("").to_string();
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
                let caller = call.get("caller_symbol").and_then(|v| v.as_str()).unwrap_or("(unknown)");
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

/// Serialize docs-code bridge as D3 graph.
pub fn bridge_to_d3(doc_graph: &DocGraph, code_graph: &serde_json::Value, bridge: &[graxus_agent_api::BridgeEdge]) -> D3Graph {
    let mut d3 = D3Graph::new("Docs-Code Bridge", "Connections between documentation and source code");

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

/// Serialize dependency graph as D3 graph.
pub fn deps_to_d3(graph: &serde_json::Value) -> D3Graph {
    let mut d3 = D3Graph::new("Dependency Graph", "File-to-file import relationships");

    if let Some(files) = graph.get("files").and_then(|v| v.as_array()) {
        for file in files {
            let path = file.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let lang = file.get("language").and_then(|v| v.as_str()).unwrap_or("?");
            d3.nodes.push(D3Node {
                id: path.to_string(),
                label: path.split('/').last().unwrap_or(path).to_string(),
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
            let resolved = imp.get("resolved_file").and_then(|v| v.as_str()).unwrap_or("");
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
