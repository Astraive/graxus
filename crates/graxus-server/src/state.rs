use anyhow::Result;
use std::path::PathBuf;

use graxus_agent_api::bridge::{BridgeBuilder, BridgeEdge};
use graxus_codemap::CodeGraph;
use graxus_core::config::GraxusConfig;
use graxus_core::workspace;
use graxus_docgraph::graph::DocGraph;

/// Shared server state holding loaded graphs and indexes.
pub struct ServerState {
    pub root: PathBuf,
    pub config: GraxusConfig,
    pub doc_graph: Option<DocGraph>,
    pub code_graph: Option<CodeGraph>,
    pub bridge: Option<Vec<BridgeEdge>>,
}

impl ServerState {
    /// Load state from the project root.
    pub fn load(root: PathBuf) -> Result<Self> {
        let config = GraxusConfig::load(&root)?;

        let docs_dir = workspace::docs_dir(&root);
        let doc_graph = if docs_dir.join("graph.json").exists() {
            Some(DocGraph::load(&docs_dir)?)
        } else {
            None
        };

        let code_dir = workspace::code_dir(&root);
        let code_graph = if code_dir.join("codemap.json").exists() {
            let content = std::fs::read_to_string(code_dir.join("codemap.json"))?;
            Some(serde_json::from_str(&content)?)
        } else {
            None
        };

        let bridge = if let (Some(ref dg), Some(ref cg)) = (&doc_graph, &code_graph) {
            Some(BridgeBuilder::build(dg, cg)?)
        } else {
            None
        };

        tracing::info!("Server state loaded from {}", root.display());
        Ok(Self {
            root,
            config,
            doc_graph,
            code_graph,
            bridge,
        })
    }

    /// Reload all graphs from disk.
    pub fn reload(&mut self) -> Result<()> {
        let new_state = Self::load(self.root.clone())?;
        self.config = new_state.config;
        self.doc_graph = new_state.doc_graph;
        self.code_graph = new_state.code_graph;
        self.bridge = new_state.bridge;
        tracing::info!("Server state reloaded");
        Ok(())
    }
}
