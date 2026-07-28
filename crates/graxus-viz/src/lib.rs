//! Graxus Visualization — HTML graph visualization with D3.js
//!
//! Provides data structures for D3.js force-directed graphs and functions
//! to serialize various Graxus graph types into D3-compatible format,
//! then render them as standalone HTML files.

pub mod serializer;
pub mod template;

use serde::{Deserialize, Serialize};

/// Maximum number of nodes allowed in a D3 graph to prevent browser hangs.
pub const MAX_NODES: usize = 10_000;

/// Maximum number of links allowed in a D3 graph to prevent browser hangs.
pub const MAX_LINKS: usize = 50_000;

/// A node in the D3 graph visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D3Node {
    /// Unique identifier for the node (e.g. file path, symbol ID).
    pub id: String,
    /// Human-readable display label shown next to the node.
    pub label: String,
    /// Category of the node (e.g. "function", "file", "doc", "struct").
    pub node_type: String,
    /// Optional source file path associated with this node.
    pub file: Option<String>,
    /// Optional line number in the source file.
    pub line: Option<usize>,
    /// Optional additional details shown in the detail panel.
    pub details: Option<String>,
}

/// A link (edge) in the D3 graph visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D3Link {
    /// ID of the source node.
    pub source: String,
    /// ID of the target node.
    pub target: String,
    /// Category of the relationship (e.g. "calls", "imports", "defines").
    pub edge_type: String,
    /// Optional human-readable label for the edge.
    pub label: Option<String>,
}

/// A complete D3 graph ready for HTML rendering.
///
/// Contains nodes, links, and metadata. Call [`D3Graph::sanitize`] before
/// rendering to enforce size limits and strip HTML from IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D3Graph {
    /// All nodes in the graph.
    pub nodes: Vec<D3Node>,
    /// All directed links between nodes.
    pub links: Vec<D3Link>,
    /// Title displayed in the graph header.
    pub title: String,
    /// Description displayed in the graph header.
    pub description: String,
}

impl D3Graph {
    /// Create a new empty graph with the given title and description.
    pub fn new(title: &str, description: &str) -> Self {
        Self {
            nodes: Vec::new(),
            links: Vec::new(),
            title: title.to_string(),
            description: description.to_string(),
        }
    }

    /// Sanitize the graph in-place: strip HTML from IDs/edge types and
    /// enforce [`MAX_NODES`] / [`MAX_LINKS`] limits.
    ///
    /// Returns the number of items that were modified or removed.
    pub fn sanitize(&mut self) -> usize {
        let mut changes = 0;

        // Strip HTML-sensitive characters from node IDs
        for node in &mut self.nodes {
            let clean = sanitize_html_chars(&node.id);
            if clean != node.id {
                node.id = clean;
                changes += 1;
            }
        }

        // Strip HTML-sensitive characters from edge types and labels
        for link in &mut self.links {
            let clean_type = sanitize_html_chars(&link.edge_type);
            if clean_type != link.edge_type {
                link.edge_type = clean_type;
                changes += 1;
            }
            if let Some(ref lbl) = link.label {
                let clean_label = sanitize_html_chars(lbl);
                if clean_label != *lbl {
                    link.label = Some(clean_label);
                    changes += 1;
                }
            }
        }

        // Enforce max node limit
        if self.nodes.len() > MAX_NODES {
            let removed = self.nodes.len() - MAX_NODES;
            self.nodes.truncate(MAX_NODES);
            changes += removed;
        }

        // Enforce max link limit
        if self.links.len() > MAX_LINKS {
            let removed = self.links.len() - MAX_LINKS;
            self.links.truncate(MAX_LINKS);
            changes += removed;
        }

        changes
    }

    /// Validate the graph without modifying it. Returns a list of issues found.
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();

        if self.nodes.len() > MAX_NODES {
            issues.push(format!(
                "Too many nodes: {} (max {})",
                self.nodes.len(),
                MAX_NODES
            ));
        }
        if self.links.len() > MAX_LINKS {
            issues.push(format!(
                "Too many links: {} (max {})",
                self.links.len(),
                MAX_LINKS
            ));
        }

        for node in &self.nodes {
            if contains_html(&node.id) {
                issues.push(format!("Node ID contains HTML: {:?}", node.id));
            }
        }

        for link in &self.links {
            if contains_html(&link.edge_type) {
                issues.push(format!("Edge type contains HTML: {:?}", link.edge_type));
            }
        }

        issues
    }
}

/// Strip HTML-sensitive characters (`<`, `>`, `&`, `"`, `'`) from a string.
fn sanitize_html_chars(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '<' | '>' | '&' | '"' | '\''))
        .collect()
}

/// Check whether a string contains HTML-sensitive characters.
fn contains_html(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '<' | '>' | '&' | '"' | '\''))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_d3graph_new() {
        let g = D3Graph::new("Title", "Desc");
        assert!(g.nodes.is_empty());
        assert!(g.links.is_empty());
        assert_eq!(g.title, "Title");
        assert_eq!(g.description, "Desc");
    }

    #[test]
    fn test_sanitize_html_chars_clean() {
        assert_eq!(sanitize_html_chars("normal_id"), "normal_id");
        assert_eq!(sanitize_html_chars("src/main.rs"), "src/main.rs");
        assert_eq!(
            sanitize_html_chars("symbol:file.rs:func"),
            "symbol:file.rs:func"
        );
    }

    #[test]
    fn test_sanitize_html_chars_dirty() {
        assert_eq!(sanitize_html_chars("<script>"), "script");
        assert_eq!(sanitize_html_chars("a&b\"c"), "abc");
        assert_eq!(sanitize_html_chars("it's"), "its");
    }

    #[test]
    fn test_contains_html() {
        assert!(!contains_html("safe_string"));
        assert!(contains_html("<b>bold</b>"));
        assert!(contains_html("a&b"));
        assert!(contains_html(r#"say "hi""#));
    }

    #[test]
    fn test_sanitize_strips_html_from_ids() {
        let mut g = D3Graph::new("T", "D");
        g.nodes.push(D3Node {
            id: "<img onerror=alert(1)>".into(),
            label: "bad".into(),
            node_type: "function".into(),
            file: None,
            line: None,
            details: None,
        });
        let changes = g.sanitize();
        assert_eq!(changes, 1);
        assert_eq!(g.nodes[0].id, "img onerror=alert(1)");
    }

    #[test]
    fn test_sanitize_strips_html_from_edge_types() {
        let mut g = D3Graph::new("T", "D");
        g.links.push(D3Link {
            source: "a".into(),
            target: "b".into(),
            edge_type: "call<script>".into(),
            label: None,
        });
        let changes = g.sanitize();
        assert_eq!(changes, 1);
        assert_eq!(g.links[0].edge_type, "callscript");
    }

    #[test]
    fn test_sanitize_truncates_nodes() {
        let mut g = D3Graph::new("T", "D");
        for i in 0..MAX_NODES + 100 {
            g.nodes.push(D3Node {
                id: format!("n{}", i),
                label: format!("Node {}", i),
                node_type: "function".into(),
                file: None,
                line: None,
                details: None,
            });
        }
        let changes = g.sanitize();
        assert_eq!(changes, 100);
        assert_eq!(g.nodes.len(), MAX_NODES);
    }

    #[test]
    fn test_sanitize_truncates_links() {
        let mut g = D3Graph::new("T", "D");
        for i in 0..MAX_LINKS + 50 {
            g.links.push(D3Link {
                source: format!("a{}", i),
                target: format!("b{}", i),
                edge_type: "calls".into(),
                label: None,
            });
        }
        let changes = g.sanitize();
        assert_eq!(changes, 50);
        assert_eq!(g.links.len(), MAX_LINKS);
    }

    #[test]
    fn test_sanitize_no_changes_for_clean_graph() {
        let mut g = D3Graph::new("Title", "Desc");
        g.nodes.push(D3Node {
            id: "n1".into(),
            label: "Node".into(),
            node_type: "function".into(),
            file: None,
            line: None,
            details: None,
        });
        g.links.push(D3Link {
            source: "n1".into(),
            target: "n1".into(),
            edge_type: "calls".into(),
            label: None,
        });
        assert_eq!(g.sanitize(), 0);
    }

    #[test]
    fn test_validate_clean_graph() {
        let g = D3Graph::new("T", "D");
        assert!(g.validate().is_empty());
    }

    #[test]
    fn test_validate_reports_html_in_ids() {
        let mut g = D3Graph::new("T", "D");
        g.nodes.push(D3Node {
            id: "a<b".into(),
            label: "x".into(),
            node_type: "function".into(),
            file: None,
            line: None,
            details: None,
        });
        let issues = g.validate();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("HTML"));
    }

    #[test]
    fn test_validate_reports_html_in_edge_type() {
        let mut g = D3Graph::new("T", "D");
        g.links.push(D3Link {
            source: "a".into(),
            target: "b".into(),
            edge_type: "call&invoke".into(),
            label: None,
        });
        let issues = g.validate();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("HTML"));
    }

    #[test]
    fn test_validate_reports_too_many_nodes() {
        let mut g = D3Graph::new("T", "D");
        for i in 0..MAX_NODES + 1 {
            g.nodes.push(D3Node {
                id: format!("n{}", i),
                label: format!("N{}", i),
                node_type: "function".into(),
                file: None,
                line: None,
                details: None,
            });
        }
        let issues = g.validate();
        assert!(issues.iter().any(|i| i.contains("Too many nodes")));
    }
}
