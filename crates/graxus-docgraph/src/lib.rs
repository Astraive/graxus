//! # graxus-docgraph
//!
//! Builds a document graph from Obsidian-compatible markdown files.
//!
//! Extracts wiki links (`[[Note]]`, `[[Note|alias]]`), frontmatter metadata,
//! inline tags (`#tag`, `#nested/tag`), and headings to produce a graph
//! of documents, tags, and their relationships.
//!
//! The graph is serialized to JSON files under `.graxus/docs/`.

pub mod frontmatter;
pub mod graph;
pub mod markdown;

use anyhow::Result;
use std::path::Path;

use graxus_core::{config::GraxusConfig, scanner};

/// Build the docs graph from the project.
pub fn build(root: &Path, config: &GraxusConfig) -> Result<graph::DocGraph> {
    let mut doc_graph = graph::DocGraph::new();

    // Scan for docs files
    let (docs, _code, _config) = scanner::scan_categorized(root, config)?;

    for file in &docs {
        let content = std::fs::read_to_string(&file.path)?;
        let (fm, _body) = frontmatter::parse(&content);
        doc_graph.add_document(&file.path, root, fm, &content);
    }

    doc_graph.generate_backlinks();

    // Save to .graxus/docs/
    let output_dir = root.join(".graxus").join("docs");
    doc_graph.save(&output_dir)?;

    tracing::info!(
        "Built docs graph: {} nodes, {} edges",
        doc_graph.nodes.len(),
        doc_graph.edges.len()
    );

    Ok(doc_graph)
}
