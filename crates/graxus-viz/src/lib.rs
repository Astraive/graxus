//! Graxus Visualization — HTML graph visualization with D3.js

pub mod serializer;
pub mod template;

use serde::{Deserialize, Serialize};

/// A node in the D3 graph visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D3Node {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub details: Option<String>,
}

/// A link (edge) in the D3 graph visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D3Link {
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub label: Option<String>,
}

/// A complete D3 graph ready for HTML rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D3Graph {
    pub nodes: Vec<D3Node>,
    pub links: Vec<D3Link>,
    pub title: String,
    pub description: String,
}

impl D3Graph {
    pub fn new(title: &str, description: &str) -> Self {
        Self {
            nodes: Vec::new(),
            links: Vec::new(),
            title: title.to_string(),
            description: description.to_string(),
        }
    }
}
