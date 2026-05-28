//! Docs-code bridge — connects documentation nodes to code symbols and files.

use graxus_codemap::{CodeGraph, ConfidenceScore, ResolutionMethod};
use graxus_docgraph::graph::{DocGraph, DocNode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Type of bridge edge connecting docs to code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BridgeEdgeType {
    DocDescribesCode,
    DocReferencesSymbol,
    DocMentionsPath,
    CodeHasDoc,
    DocMayBeStale,
}

/// A bridge edge connecting a doc node to a code element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeEdge {
    pub from: String,
    pub to: String,
    pub edge_type: BridgeEdgeType,
    pub confidence: ConfidenceScore,
}

/// Builder for creating bridge edges between docs and code.
pub struct BridgeBuilder;

impl BridgeBuilder {
    /// Build all bridge edges from a doc graph and code graph.
    pub fn build(doc_graph: &DocGraph, code_graph: &CodeGraph) -> anyhow::Result<Vec<BridgeEdge>> {
        let mut edges = Vec::new();

        let file_path_re = Regex::new(r"(?:src/|lib/|crates/|app/)[\w/.-]+\.\w+")?;
        let code_paths: HashSet<&str> = code_graph.file_paths().into_iter().collect();
        let symbol_names: HashSet<&str> = code_graph
            .symbols
            .iter()
            .map(|s| s.name.as_str())
            .collect();

        for node in &doc_graph.nodes {
            // 1. Explicit file paths from frontmatter related_code
            Self::add_frontmatter_links(&mut edges, node, code_graph);

            // 2. File paths mentioned in document content (from headings, links)
            Self::add_path_mentions(&mut edges, node, &file_path_re, &code_paths);

            // 3. Symbol name mentions in doc title, headings, and tags
            Self::add_symbol_mentions(&mut edges, node, &symbol_names);

            // 4. Headings matching module/file names
            Self::add_heading_matches(&mut edges, node, code_graph);
        }

        // 5. Detect stale docs (references to missing files/symbols)
        Self::detect_stale(&mut edges, code_graph);

        tracing::info!("Built bridge: {} edges", edges.len());
        Ok(edges)
    }

    /// Get all bridge edges that reference a specific code file.
    pub fn docs_for_code<'a>(bridge: &'a [BridgeEdge], code_path: &str) -> Vec<&'a BridgeEdge> {
        bridge
            .iter()
            .filter(|e| {
                (e.to == code_path || e.to.starts_with(&format!("{}::", code_path)))
                    && matches!(
                        e.edge_type,
                        BridgeEdgeType::DocDescribesCode
                            | BridgeEdgeType::DocReferencesSymbol
                            | BridgeEdgeType::DocMentionsPath
                    )
            })
            .collect()
    }

    /// Get all bridge edges that reference a specific doc.
    pub fn code_for_doc<'a>(bridge: &'a [BridgeEdge], doc_path: &str) -> Vec<&'a BridgeEdge> {
        bridge
            .iter()
            .filter(|e| {
                e.from == doc_path
                    && matches!(
                        e.edge_type,
                        BridgeEdgeType::DocDescribesCode
                            | BridgeEdgeType::DocReferencesSymbol
                            | BridgeEdgeType::DocMentionsPath
                    )
            })
            .collect()
    }

    /// Get all docs that may be stale.
    pub fn stale_docs<'a>(bridge: &'a [BridgeEdge]) -> Vec<&'a BridgeEdge> {
        bridge
            .iter()
            .filter(|e| matches!(e.edge_type, BridgeEdgeType::DocMayBeStale))
            .collect()
    }

    fn add_frontmatter_links(edges: &mut Vec<BridgeEdge>, node: &DocNode, code_graph: &CodeGraph) {
        if let Some(ref fm) = node.frontmatter {
            for code_path in &fm.related_code {
                if code_graph.has_file(code_path) {
                    edges.push(BridgeEdge {
                        from: node.id.clone(),
                        to: code_path.clone(),
                        edge_type: BridgeEdgeType::DocDescribesCode,
                        confidence: ConfidenceScore::new(85.0, ResolutionMethod::PathMatchOnly),
                    });
                } else {
                    edges.push(BridgeEdge {
                        from: node.id.clone(),
                        to: code_path.clone(),
                        edge_type: BridgeEdgeType::DocMayBeStale,
                        confidence: ConfidenceScore::new(85.0, ResolutionMethod::PathMatchOnly),
                    });
                }
            }

            for symbol_name in &fm.symbols {
                if code_graph.find_symbol(symbol_name).is_some() {
                    edges.push(BridgeEdge {
                        from: node.id.clone(),
                        to: symbol_name.clone(),
                        edge_type: BridgeEdgeType::DocReferencesSymbol,
                        confidence: ConfidenceScore::new(85.0, ResolutionMethod::PathMatchOnly),
                    });
                }
            }
        }
    }

    fn add_path_mentions(
        edges: &mut Vec<BridgeEdge>,
        node: &DocNode,
        file_path_re: &Regex,
        code_paths: &HashSet<&str>,
    ) {
        // Search in headings text
        let mut search_text = String::new();
        for heading in &node.headings {
            search_text.push_str(&heading.text);
            search_text.push(' ');
        }
        // Search in wiki link targets
        for link in &node.wiki_links {
            search_text.push_str(&link.target);
            search_text.push(' ');
        }

        for cap in file_path_re.captures_iter(&search_text) {
            let path = &cap[0];
            if code_paths.contains(path) {
                edges.push(BridgeEdge {
                    from: node.id.clone(),
                    to: path.to_string(),
                    edge_type: BridgeEdgeType::DocMentionsPath,
                    confidence: ConfidenceScore::new(65.0, ResolutionMethod::PathMatchOnly),
                });
            }
        }
    }

    fn add_symbol_mentions(
        edges: &mut Vec<BridgeEdge>,
        node: &DocNode,
        symbol_names: &HashSet<&str>,
    ) {
        // Check doc title
        for word in node.title.split_whitespace() {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
            if symbol_names.contains(clean) && clean.len() > 2 {
                edges.push(BridgeEdge {
                    from: node.id.clone(),
                    to: clean.to_string(),
                    edge_type: BridgeEdgeType::DocReferencesSymbol,
                    confidence: ConfidenceScore::new(40.0, ResolutionMethod::FuzzySymbolMatch),
                });
            }
        }

        // Check heading text
        for heading in &node.headings {
            for word in heading.text.split_whitespace() {
                let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                if symbol_names.contains(clean) && clean.len() > 2 {
                    edges.push(BridgeEdge {
                        from: node.id.clone(),
                        to: clean.to_string(),
                        edge_type: BridgeEdgeType::DocReferencesSymbol,
                        confidence: ConfidenceScore::new(40.0, ResolutionMethod::FuzzySymbolMatch),
                    });
                }
            }
        }
    }

    fn add_heading_matches(edges: &mut Vec<BridgeEdge>, node: &DocNode, code_graph: &CodeGraph) {
        for heading in &node.headings {
            let normalized = heading.text.to_lowercase().replace(' ', "_");
            // Check if any file stem matches the heading
            for file in &code_graph.files {
                let stem = std::path::Path::new(&file.path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if stem == normalized || stem.replace('_', "") == normalized.replace('_', "") {
                    edges.push(BridgeEdge {
                        from: node.id.clone(),
                        to: file.path.clone(),
                        edge_type: BridgeEdgeType::DocDescribesCode,
                        confidence: ConfidenceScore::new(40.0, ResolutionMethod::FuzzySymbolMatch),
                    });
                }
            }
        }
    }

    fn detect_stale(edges: &mut Vec<BridgeEdge>, code_graph: &CodeGraph) {
        let stale: Vec<BridgeEdge> = edges
            .iter()
            .filter(|e| matches!(e.edge_type, BridgeEdgeType::DocReferencesSymbol | BridgeEdgeType::DocMentionsPath))
            .filter(|e| !code_graph.has_file(&e.to) && code_graph.find_symbol(&e.to).is_none())
            .map(|e| BridgeEdge {
                from: e.from.clone(),
                to: e.to.clone(),
                edge_type: BridgeEdgeType::DocMayBeStale,
                confidence: ConfidenceScore::new(85.0, ResolutionMethod::PathMatchOnly),
            })
            .collect();
        edges.extend(stale);
    }
}
