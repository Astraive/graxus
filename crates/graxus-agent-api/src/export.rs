//! Agent export — full context dump for AI agent consumption.

use graxus_codemap::CodeGraph;
use graxus_docgraph::graph::DocGraph;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::bridge::BridgeEdge;
use crate::context::{estimate_tokens, ContextBudget};

/// Full export of Graxus knowledge for an AI agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExport {
    pub project_name: String,
    pub doc_graph: DocGraph,
    pub code_graph: CodeGraph,
    pub bridge: Vec<BridgeEdge>,
    pub generated_at: String,
}

impl AgentExport {
    /// Create a new agent export.
    pub fn new(
        project_name: &str,
        doc_graph: DocGraph,
        code_graph: CodeGraph,
        bridge: Vec<BridgeEdge>,
    ) -> Self {
        Self {
            project_name: project_name.to_string(),
            doc_graph,
            code_graph,
            bridge,
            generated_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Save the export to a JSON file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        tracing::info!("Saved agent export to {}", path.display());
        Ok(())
    }

    /// Load an export from a JSON file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let export: AgentExport = serde_json::from_str(&content)?;
        Ok(export)
    }

    /// Get summary stats.
    pub fn stats(&self) -> ExportStats {
        ExportStats {
            doc_nodes: self.doc_graph.nodes.len(),
            doc_edges: self.doc_graph.edges.len(),
            code_files: self.code_graph.files.len(),
            symbols: self.code_graph.symbols.len(),
            imports: self.code_graph.imports.len(),
            calls: self.code_graph.calls.len(),
            bridge_edges: self.bridge.len(),
        }
    }

    /// Create a bounded export that fits within a token budget.
    /// Truncates symbols, imports, calls, and bridge edges to fit.
    pub fn export_bounded(&self, max_tokens: usize) -> AgentExport {
        let mut budget = ContextBudget::new(max_tokens);

        // Budget the high-level counts
        let _ = budget.consume(estimate_tokens(&self.project_name) + 50); // overhead

        // Allocate budget proportionally:
        // 40% symbols, 20% imports, 20% calls, 10% bridge, 10% docs
        let sym_budget = max_tokens * 40 / 100;
        let imp_budget = max_tokens * 20 / 100;
        let call_budget = max_tokens * 20 / 100;
        let bridge_budget = max_tokens * 10 / 100;
        let doc_budget = max_tokens * 10 / 100;

        let mut sym_tok = 0usize;
        let bounded_symbols: Vec<_> = self.code_graph.symbols.iter()
            .filter(|s| {
                let t = estimate_tokens(&s.name) + estimate_tokens(&s.file) + 20;
                if sym_tok + t <= sym_budget { sym_tok += t; true } else { false }
            })
            .cloned()
            .collect();

        let mut imp_tok = 0usize;
        let bounded_imports: Vec<_> = self.code_graph.imports.iter()
            .filter(|i| {
                let t = estimate_tokens(&i.source) + estimate_tokens(&i.file) + 10;
                if imp_tok + t <= imp_budget { imp_tok += t; true } else { false }
            })
            .cloned()
            .collect();

        let mut call_tok = 0usize;
        let bounded_calls: Vec<_> = self.code_graph.calls.iter()
            .filter(|c| {
                let t = estimate_tokens(&c.callee_text) + estimate_tokens(&c.file) + 15;
                if call_tok + t <= call_budget { call_tok += t; true } else { false }
            })
            .cloned()
            .collect();

        let mut bridge_tok = 0usize;
        let bounded_bridge: Vec<_> = self.bridge.iter()
            .filter(|e| {
                let t = estimate_tokens(&e.from) + estimate_tokens(&e.to) + 10;
                if bridge_tok + t <= bridge_budget { bridge_tok += t; true } else { false }
            })
            .cloned()
            .collect();

        // Docs: keep all nodes (usually few), but truncate node content is not needed
        // since DocNode is already compact
        let mut doc_tok = 0usize;
        let bounded_docs: Vec<_> = self.doc_graph.nodes.iter()
            .filter(|d| {
                let t = estimate_tokens(&d.title) + estimate_tokens(&d.path) + 20;
                if doc_tok + t <= doc_budget { doc_tok += t; true } else { false }
            })
            .cloned()
            .collect();

        AgentExport {
            project_name: self.project_name.clone(),
            doc_graph: DocGraph {
                nodes: bounded_docs,
                edges: self.doc_graph.edges.clone(),
            },
            code_graph: graxus_codemap::CodeGraph {
                files: self.code_graph.files.clone(),
                symbols: bounded_symbols,
                imports: bounded_imports,
                calls: bounded_calls,
                edges: self.code_graph.edges.clone(),
                type_hints: self.code_graph.type_hints.clone(),
            },
            bridge: bounded_bridge,
            generated_at: self.generated_at.clone(),
        }
    }
}

/// Summary statistics for an export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportStats {
    pub doc_nodes: usize,
    pub doc_edges: usize,
    pub code_files: usize,
    pub symbols: usize,
    pub imports: usize,
    pub calls: usize,
    pub bridge_edges: usize,
}

#[cfg(test)]
mod tests {
    use graxus_codemap::{ConfidenceScore, ResolutionMethod};
    use super::*;
    use graxus_codemap::{FileNode, SymbolFact, SymbolKind, Visibility, ImportFact, ImportKind, CallFact, CallKind, Confidence};

    fn make_export() -> AgentExport {
        let doc_graph = DocGraph {
            nodes: vec![
                graxus_docgraph::graph::DocNode {
                    id: "doc:README".into(),
                    node_type: graxus_docgraph::graph::DocNodeType::Document,
                    path: "README.md".into(),
                    title: "README".into(),
                    tags: vec![],
                    frontmatter: None,
                    headings: vec![],
                    wiki_links: vec![],
                },
            ],
            edges: vec![],
        };

        let mut symbols = Vec::new();
        for i in 0..100 {
            symbols.push(SymbolFact {
                id: format!("sym{}", i),
                file: format!("src/{}.rs", i),
                language: "rust".into(),
                kind: SymbolKind::Function,
                name: format!("func_{}", i),
                exported: true,
                line_start: 1,
                line_end: 10,
                visibility: Visibility::Public,
                signature: format!("fn func_{}()", i),
                is_test: false,
                usage_count: 0,
            });
        }

        let code_graph = graxus_codemap::CodeGraph {
            files: vec![FileNode {
                path: "src/0.rs".into(),
                language: "rust".into(),
                hash: "abc".into(),
                size: 100,
            }],
            symbols,
            imports: vec![ImportFact {
                id: "imp0".into(),
                file: "src/0.rs".into(),
                language: "rust".into(),
                kind: ImportKind::RustUse,
                source: "std::collections::HashMap".into(),
                local_name: Some("HashMap".into()),
                imported_name: None,
                resolved_file: None,
                line: 1,
                confidence: ConfidenceScore::new(85.0, ResolutionMethod::PathMatchOnly),
            }],
            calls: vec![CallFact {
                id: "call0".into(),
                file: "src/0.rs".into(),
                language: "rust".into(),
                kind: CallKind::FunctionCall,
                caller_symbol: None,
                callee_text: "println".into(),
                object: None,
                resolved_symbol: None,
                line: 5,
                column: 4,
                confidence: ConfidenceScore::new(85.0, ResolutionMethod::PathMatchOnly),
            }],
            edges: vec![],
            type_hints: vec![],
        };

        AgentExport::new("test_project", doc_graph, code_graph, vec![])
    }

    #[test]
    fn test_export_bounded_truncates() {
        let export = make_export();
        assert_eq!(export.code_graph.symbols.len(), 100);

        // With a small budget, should truncate symbols
        let bounded = export.export_bounded(500);
        assert!(bounded.code_graph.symbols.len() < 100);
        assert_eq!(bounded.project_name, "test_project");
    }

    #[test]
    fn test_export_bounded_full_budget() {
        let export = make_export();
        // With a huge budget, should keep everything
        let bounded = export.export_bounded(1_000_000);
        assert_eq!(bounded.code_graph.symbols.len(), 100);
    }

    #[test]
    fn test_export_stats() {
        let export = make_export();
        let stats = export.stats();
        assert_eq!(stats.doc_nodes, 1);
        assert_eq!(stats.symbols, 100);
        assert_eq!(stats.imports, 1);
        assert_eq!(stats.calls, 1);
    }
}
