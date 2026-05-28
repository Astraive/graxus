use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::frontmatter::Frontmatter;
use crate::markdown::{self, Heading, WikiLink};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocNode {
    pub id: String,
    pub node_type: DocNodeType,
    pub path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub frontmatter: Option<Frontmatter>,
    pub headings: Vec<Heading>,
    pub wiki_links: Vec<WikiLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DocNodeType {
    Document,
    Heading,
    Tag,
    Concept,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocEdge {
    pub from: String,
    pub to: String,
    pub edge_type: DocEdgeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DocEdgeType {
    LinksTo,
    HasTag,
    HasHeading,
    Mentions,
    BacklinksTo,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocGraph {
    pub nodes: Vec<DocNode>,
    pub edges: Vec<DocEdge>,
}

impl DocGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a document node to the graph.
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

    /// Generate backlinks by inverting LinksTo edges.
    pub fn generate_backlinks(&mut self) {
        let links_to: Vec<DocEdge> = self
            .edges
            .iter()
            .filter(|e| matches!(e.edge_type, DocEdgeType::LinksTo))
            .cloned()
            .collect();

        for edge in links_to {
            self.edges.push(DocEdge {
                from: edge.to,
                to: edge.from,
                edge_type: DocEdgeType::BacklinksTo,
            });
        }
    }

    /// Get all backlinks for a document.
    pub fn get_backlinks(&self, doc_id: &str) -> Vec<&DocNode> {
        self.edges
            .iter()
            .filter(|e| matches!(e.edge_type, DocEdgeType::BacklinksTo) && e.to == doc_id)
            .filter_map(|e| self.nodes.iter().find(|n| n.id == e.from))
            .collect()
    }

    /// Get all tags in the graph.
    pub fn get_all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .nodes
            .iter()
            .flat_map(|n| n.tags.clone())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// Find a document by path.
    pub fn find_by_path(&self, path: &str) -> Option<&DocNode> {
        self.nodes.iter().find(|n| n.path == path)
    }

    /// Save the graph to JSON files.
    pub fn save(&self, output_dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(output_dir)?;

        let graph_path = output_dir.join("graph.json");
        let graph_json = serde_json::to_string_pretty(self)?;
        std::fs::write(&graph_path, graph_json)?;

        let nodes_path = output_dir.join("nodes.json");
        let nodes_json = serde_json::to_string_pretty(&self.nodes)?;
        std::fs::write(&nodes_path, nodes_json)?;

        let edges_path = output_dir.join("edges.json");
        let edges_json = serde_json::to_string_pretty(&self.edges)?;
        std::fs::write(&edges_path, edges_json)?;

        Ok(())
    }

    /// Load the graph from JSON.
    pub fn load(output_dir: &Path) -> anyhow::Result<Self> {
        let graph_path = output_dir.join("graph.json");
        let content = std::fs::read_to_string(graph_path)?;
        let graph: DocGraph = serde_json::from_str(&content)?;
        Ok(graph)
    }
}
