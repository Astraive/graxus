//! Document graph data structures and operations.
//!
//! [`DocGraph`] stores document nodes and typed edges (links, tags, headings,
//! backlinks) and provides serialization to JSON.

use serde::{Deserialize, Serialize};
use std::io::BufWriter;
use std::path::Path;

use crate::frontmatter::Frontmatter;
use crate::markdown::{self, Heading, WikiLink};

/// A node in the document graph representing a single markdown document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocNode {
    /// Unique identifier (e.g. `"doc:notes/My Note.md"`).
    pub id: String,
    /// Classification of this node.
    pub node_type: DocNodeType,
    /// Relative file path from the project root.
    pub path: String,
    /// Display title (from frontmatter, first heading, or filename).
    pub title: String,
    /// Tags associated with this document.
    pub tags: Vec<String>,
    /// Parsed YAML frontmatter, if present.
    pub frontmatter: Option<Frontmatter>,
    /// Headings extracted from the document body.
    pub headings: Vec<Heading>,
    /// Wiki links found in the document.
    pub wiki_links: Vec<WikiLink>,
}

/// Classification for nodes in the document graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DocNodeType {
    /// A markdown document file.
    Document,
    /// A heading within a document.
    Heading,
    /// A tag node (created when a tag is referenced by documents).
    Tag,
    /// A concept or entity extracted from content.
    Concept,
}

/// A typed edge connecting two nodes in the document graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocEdge {
    /// Source node ID.
    pub from: String,
    /// Target node ID.
    pub to: String,
    /// Relationship type.
    pub edge_type: DocEdgeType,
}

/// Relationship types between nodes in the document graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DocEdgeType {
    /// Document A links to Document B via a wiki link.
    LinksTo,
    /// Document has a tag.
    HasTag,
    /// Document contains a heading.
    HasHeading,
    /// Document mentions another entity.
    Mentions,
    /// Inverse of LinksTo: Document B is linked from Document A.
    BacklinksTo,
}

/// The complete document graph containing all nodes and edges.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocGraph {
    /// All document nodes in the graph.
    pub nodes: Vec<DocNode>,
    /// All edges connecting nodes.
    pub edges: Vec<DocEdge>,
}

impl DocGraph {
    /// Create an empty document graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a document node to the graph.
    ///
    /// Parses the content for wiki links, tags, and headings, creates
    /// the corresponding node and edges, and appends them to the graph.
    pub fn add_document(
        &mut self,
        path: &Path,
        root: &Path,
        frontmatter: Option<Frontmatter>,
        content: &str,
    ) {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let wiki_links = markdown::extract_wiki_links(content);
        let headings = markdown::extract_headings(content);
        let tags = if let Some(ref fm) = frontmatter {
            fm.tags.clone()
        } else {
            markdown::extract_tags(content)
        };

        let title = frontmatter
            .as_ref()
            .and_then(|fm| fm.title.clone())
            .or_else(|| {
                // Use first heading as title
                headings.first().map(|h| h.text.clone())
            })
            .unwrap_or_else(|| {
                Path::new(&relative)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| relative.clone())
            });

        let node_id = format!("doc:{}", relative);

        // Add edges for wiki links
        for link in &wiki_links {
            self.edges.push(DocEdge {
                from: node_id.clone(),
                to: format!("doc:{}", link.target),
                edge_type: DocEdgeType::LinksTo,
            });
        }

        // Add edges for tags
        for tag in &tags {
            self.edges.push(DocEdge {
                from: node_id.clone(),
                to: format!("tag:{}", tag),
                edge_type: DocEdgeType::HasTag,
            });
        }

        // Add edges for headings
        for heading in &headings {
            self.edges.push(DocEdge {
                from: node_id.clone(),
                to: format!("heading:{}:{}", relative, heading.text),
                edge_type: DocEdgeType::HasHeading,
            });
        }

        self.nodes.push(DocNode {
            id: node_id,
            node_type: DocNodeType::Document,
            path: relative,
            title,
            tags,
            frontmatter,
            headings,
            wiki_links,
        });
    }

    /// Generate backlinks by inverting existing `LinksTo` edges.
    ///
    /// For each `LinksTo` edge from A to B, creates a `BacklinksTo` edge
    /// from B to A.
    pub fn generate_backlinks(&mut self) {
        // Collect only the string pairs we need — avoids cloning full DocEdge structs.
        let backlink_pairs: Vec<(String, String)> = self
            .edges
            .iter()
            .filter(|e| matches!(e.edge_type, DocEdgeType::LinksTo))
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect();

        for (from, to) in backlink_pairs {
            self.edges.push(DocEdge {
                from: to,
                to: from,
                edge_type: DocEdgeType::BacklinksTo,
            });
        }
    }

    /// Get all backlinks for a document by its node ID.
    ///
    /// Returns references to all nodes that contain links pointing to the
    /// given document. For example, if document A contains `[[B]]`, then
    /// `get_backlinks("doc:B")` returns `[A]`.
    ///
    /// BacklinksTo edges are stored with `from = target_doc` and
    /// `to = source_doc` (the inverse of LinksTo), so we match on `e.from`.
    pub fn get_backlinks(&self, doc_id: &str) -> Vec<&DocNode> {
        self.edges
            .iter()
            .filter(|e| matches!(e.edge_type, DocEdgeType::BacklinksTo) && e.from == doc_id)
            .filter_map(|e| self.nodes.iter().find(|n| n.id == e.to))
            .collect()
    }

    /// Get all unique tags across all documents in the graph.
    ///
    /// Returns a sorted, deduplicated list of tag strings.
    pub fn get_all_tags(&self) -> Vec<String> {
        // Use iter().cloned() instead of cloning each node's tag Vec.
        let mut tags: Vec<String> = self
            .nodes
            .iter()
            .flat_map(|n| n.tags.iter().cloned())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// Find a document node by its relative file path.
    pub fn find_by_path(&self, path: &str) -> Option<&DocNode> {
        self.nodes.iter().find(|n| n.path == path)
    }

    /// Save the graph to JSON files in the output directory.
    ///
    /// Writes three files using buffered I/O:
    /// - `graph.json` — the full graph (nodes + edges)
    /// - `nodes.json` — just the node list
    /// - `edges.json` — just the edge list
    pub fn save(&self, output_dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(output_dir)?;

        {
            let file = std::fs::File::create(output_dir.join("graph.json"))?;
            let writer = BufWriter::new(file);
            serde_json::to_writer_pretty(writer, self)?;
        }

        {
            let file = std::fs::File::create(output_dir.join("nodes.json"))?;
            let writer = BufWriter::new(file);
            serde_json::to_writer_pretty(writer, &self.nodes)?;
        }

        {
            let file = std::fs::File::create(output_dir.join("edges.json"))?;
            let writer = BufWriter::new(file);
            serde_json::to_writer_pretty(writer, &self.edges)?;
        }

        Ok(())
    }

    /// Load the graph from a JSON file.
    ///
    /// Reads `graph.json` from the given directory.
    pub fn load(output_dir: &Path) -> anyhow::Result<Self> {
        let graph_path = output_dir.join("graph.json");
        let content = std::fs::read_to_string(graph_path)?;
        let graph: DocGraph = serde_json::from_str(&content)?;
        Ok(graph)
    }

    /// Remove all data (node and edges) for the given document path.
    pub fn remove_document(&mut self, path: &str) {
        // Find node IDs to remove
        let ids_to_remove: std::collections::HashSet<String> = self
            .nodes
            .iter()
            .filter(|n| n.path == path)
            .map(|n| n.id.clone())
            .collect();

        self.nodes.retain(|n| n.path != path);
        self.edges
            .retain(|e| !ids_to_remove.contains(&e.from) && !ids_to_remove.contains(&e.to));
    }

    /// Merge another DocGraph into this one.
    ///
    /// Documents in `other` that already exist in `self` will have their data
    /// replaced (effectively an update). New documents are appended.
    pub fn merge(&mut self, other: DocGraph) {
        // Remove existing data for documents that are being updated
        let other_paths: std::collections::HashSet<&str> =
            other.nodes.iter().map(|n| n.path.as_str()).collect();
        for path in &other_paths {
            self.remove_document(path);
        }

        // Append all data from other
        self.nodes.extend(other.nodes);
        self.edges.extend(other.edges);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from("/project")
    }

    fn make_doc(content: &str) -> (PathBuf, Option<Frontmatter>, String) {
        let path = PathBuf::from("/project/notes/Test.md");
        let fm = crate::frontmatter::parse(content).0;
        (path, fm, content.to_string())
    }

    // ── Graph construction ────────────────────────────────────────

    #[test]
    fn new_graph_is_empty() {
        let g = DocGraph::new();
        assert!(g.nodes.is_empty());
        assert!(g.edges.is_empty());
    }

    #[test]
    fn add_single_document() {
        let mut g = DocGraph::new();
        let content = "---\ntitle: Hello\ntags:\n  - dev\n---\n# Heading\nBody with [[Link]].";
        let (path, fm, body) = make_doc(content);
        g.add_document(&path, &root(), fm, &body);

        assert_eq!(g.nodes.len(), 1);
        let node = &g.nodes[0];
        assert_eq!(node.title, "Hello");
        assert_eq!(node.tags, vec!["dev"]);
        assert_eq!(node.wiki_links.len(), 1);
        assert_eq!(node.wiki_links[0].target, "Link");
        assert_eq!(node.headings.len(), 1);
        assert_eq!(node.headings[0].text, "Heading");
    }

    #[test]
    fn add_multiple_documents() {
        let mut g = DocGraph::new();

        let path_a = PathBuf::from("/project/notes/A.md");
        g.add_document(&path_a, &root(), None, "Body [[B]].");

        let path_b = PathBuf::from("/project/notes/B.md");
        g.add_document(&path_b, &root(), None, "Body.");

        assert_eq!(g.nodes.len(), 2);
        // A links to B
        let links_to: Vec<_> = g
            .edges
            .iter()
            .filter(|e| matches!(e.edge_type, DocEdgeType::LinksTo))
            .collect();
        assert_eq!(links_to.len(), 1);
        assert_eq!(links_to[0].from, "doc:notes/A.md");
        assert_eq!(links_to[0].to, "doc:B");
    }

    #[test]
    fn document_without_frontmatter_uses_heading_as_title() {
        let mut g = DocGraph::new();
        let (path, fm, body) = make_doc("# My Heading\nContent.");
        g.add_document(&path, &root(), fm, &body);

        assert_eq!(g.nodes[0].title, "My Heading");
    }

    #[test]
    fn document_without_heading_uses_filename() {
        let mut g = DocGraph::new();
        let (path, fm, body) = make_doc("Just body text.");
        g.add_document(&path, &root(), fm, &body);

        assert_eq!(g.nodes[0].title, "Test");
    }

    #[test]
    fn empty_document() {
        let mut g = DocGraph::new();
        let (path, fm, body) = make_doc("");
        g.add_document(&path, &root(), fm, &body);

        assert_eq!(g.nodes.len(), 1);
        assert!(g.nodes[0].wiki_links.is_empty());
        assert!(g.nodes[0].headings.is_empty());
        assert!(g.nodes[0].tags.is_empty());
    }

    #[test]
    fn no_frontmatter() {
        let mut g = DocGraph::new();
        let (path, fm, body) = make_doc("# Tagged\n#rust #go\nContent.");
        g.add_document(&path, &root(), fm, &body);

        // Without frontmatter, inline tags are extracted
        assert!(g.nodes[0].tags.contains(&"rust".to_string()));
        assert!(g.nodes[0].tags.contains(&"go".to_string()));
    }

    // ── Backlinks ─────────────────────────────────────────────────

    #[test]
    fn backlinks_generated() {
        let mut g = DocGraph::new();

        let path_a = PathBuf::from("/project/A.md");
        g.add_document(&path_a, &root(), None, "Link to [[B]].");

        let path_b = PathBuf::from("/project/B.md");
        g.add_document(&path_b, &root(), None, "No links.");

        g.generate_backlinks();

        let backlinks = g.get_backlinks("doc:B");
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].id, "doc:A.md");
    }

    #[test]
    fn backlinks_none() {
        let mut g = DocGraph::new();
        let path = PathBuf::from("/project/A.md");
        g.add_document(&path, &root(), None, "No links.");
        g.generate_backlinks();

        assert!(g.get_backlinks("doc:A.md").is_empty());
    }

    #[test]
    fn backlinks_transitive() {
        let mut g = DocGraph::new();

        let path_a = PathBuf::from("/project/A.md");
        g.add_document(&path_a, &root(), None, "Link to [[B]].");

        let path_b = PathBuf::from("/project/B.md");
        g.add_document(&path_b, &root(), None, "Link to [[C]].");

        let path_c = PathBuf::from("/project/C.md");
        g.add_document(&path_c, &root(), None, "No links.");

        g.generate_backlinks();

        // B has a backlink from A
        let bl_b = g.get_backlinks("doc:B");
        assert_eq!(bl_b.len(), 1);
        assert_eq!(bl_b[0].id, "doc:A.md");

        // C has a backlink from B
        let bl_c = g.get_backlinks("doc:C");
        assert_eq!(bl_c.len(), 1);
        assert_eq!(bl_c[0].id, "doc:B.md");

        // A has no backlinks
        let bl_a = g.get_backlinks("doc:A.md");
        assert!(bl_a.is_empty());
    }

    // ── Tags ──────────────────────────────────────────────────────

    #[test]
    fn get_all_tags_deduplicates() {
        let mut g = DocGraph::new();

        let path_a = PathBuf::from("/project/A.md");
        g.add_document(&path_a, &root(), None, "#rust #tag1");

        let path_b = PathBuf::from("/project/B.md");
        g.add_document(&path_b, &root(), None, "#rust #tag2");

        let tags = g.get_all_tags();
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"tag1".to_string()));
        assert!(tags.contains(&"tag2".to_string()));
        // "rust" appears in both docs but should be deduplicated
        assert_eq!(tags.iter().filter(|t| *t == "rust").count(), 1);
    }

    #[test]
    fn get_all_tags_from_frontmatter() {
        let mut g = DocGraph::new();
        let content = "---\ntitle: X\ntags:\n  - pm\n  - design\n---\nBody.";
        let (path, fm, body) = make_doc(content);
        g.add_document(&path, &root(), fm, &body);

        let tags = g.get_all_tags();
        assert!(tags.contains(&"pm".to_string()));
        assert!(tags.contains(&"design".to_string()));
    }

    // ── Find by path ──────────────────────────────────────────────

    #[test]
    fn find_by_path_found() {
        let mut g = DocGraph::new();
        let path = PathBuf::from("/project/notes/FindMe.md");
        g.add_document(&path, &root(), None, "Content.");

        let node = g.find_by_path("notes/FindMe.md");
        assert!(node.is_some());
        assert_eq!(node.unwrap().title, "FindMe");
    }

    #[test]
    fn find_by_path_not_found() {
        let g = DocGraph::new();
        assert!(g.find_by_path("nonexistent.md").is_none());
    }

    // ── Save / Load round-trip ────────────────────────────────────

    #[test]
    fn save_and_load_roundtrip() {
        let mut g = DocGraph::new();
        let path = PathBuf::from("/project/A.md");
        g.add_document(&path, &root(), None, "# Title\n#tag1\n[[B]]");

        let tmp = std::env::temp_dir().join("graxus_docgraph_test_roundtrip");
        let _ = std::fs::remove_dir_all(&tmp); // clean up from prior runs
        g.save(&tmp).unwrap();

        // Verify files exist
        assert!(tmp.join("graph.json").exists());
        assert!(tmp.join("nodes.json").exists());
        assert!(tmp.join("edges.json").exists());

        // Load and verify round-trip
        let loaded = DocGraph::load(&tmp).unwrap();
        assert_eq!(loaded.nodes.len(), g.nodes.len());
        assert_eq!(loaded.edges.len(), g.edges.len());
        assert_eq!(loaded.nodes[0].title, "Title");

        // Clean up
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ── Wiki link edge cases ──────────────────────────────────────

    #[test]
    fn malformed_wiki_link_no_crash() {
        let mut g = DocGraph::new();
        // Unclosed [[ with no ]] at all
        let (path, fm, body) = make_doc("Broken [[link without closing.");
        g.add_document(&path, &root(), fm, &body);

        // No valid link extracted — unclosed brackets are ignored
        assert_eq!(g.nodes[0].wiki_links.len(), 0);
    }

    #[test]
    fn nested_tags_in_content() {
        let mut g = DocGraph::new();
        let (path, fm, body) = make_doc("Tags: #project/frontend #backend/api");
        g.add_document(&path, &root(), fm, &body);

        assert!(g.nodes[0].tags.contains(&"project/frontend".to_string()));
        assert!(g.nodes[0].tags.contains(&"backend/api".to_string()));
    }

    #[test]
    fn edge_count_correct() {
        let mut g = DocGraph::new();
        let (path, fm, body) = make_doc("---\ntitle: T\ntags:\n  - a\n---\n# H1\n[[Other]]");
        g.add_document(&path, &root(), fm, &body);

        // 1 LinksTo + 1 HasTag + 1 HasHeading = 3 edges
        assert_eq!(g.edges.len(), 3);
    }
}
