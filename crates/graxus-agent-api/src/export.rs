//! Agent export — full context dump for AI agent consumption.

use graxus_codemap::CodeGraph;
use graxus_docgraph::graph::DocGraph;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::bridge::BridgeEdge;
use crate::context::estimate_tokens;

const EXPORT_OVERHEAD_TOKENS: usize = 100;

/// Clone complete facts in stable order until their category allocation is full.
///
/// Ordering by a normalized fact key makes selection independent of collection
/// insertion order, while cloning the selected value keeps each fact intact.
fn bounded_facts<T: Clone>(
    facts: &[T],
    allocation: usize,
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
    tokens: impl Fn(&T) -> usize,
) -> Vec<T> {
    let mut ordered: Vec<&T> = facts.iter().collect();
    ordered.sort_unstable_by(|left, right| compare(left, right));

    let mut used = 0usize;
    ordered
        .into_iter()
        .filter_map(|fact| {
            let fact_tokens = tokens(fact);
            if used + fact_tokens <= allocation {
                used += fact_tokens;
                Some(fact.clone())
            } else {
                None
            }
        })
        .collect()
}

fn serialized_fact_tokens<T: Serialize>(fact: &T, overhead: usize) -> usize {
    serde_json::to_string(fact)
        .map(|json| estimate_tokens(&json) + overhead)
        .unwrap_or(overhead)
}

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
            routes: self.code_graph.routes.len(),
            type_impls: self.code_graph.type_impls.len(),
            di_bindings: self.code_graph.di_bindings.len(),
        }
    }

    /// Create a bounded export that fits semantic and structural facts into
    /// deterministic category allocations without splitting individual facts.
    pub fn export_bounded(&self, max_tokens: usize) -> AgentExport {
        let content_budget =
            max_tokens.saturating_sub(estimate_tokens(&self.project_name) + EXPORT_OVERHEAD_TOKENS);

        // Every emitted collection receives an independent allocation. The
        // allocations deliberately include structural collections (files,
        // edges, parser provenance, and documentation edges), not just the
        // normalized semantic facts.
        let sym_budget = content_budget * 20 / 100;
        let imp_budget = content_budget * 8 / 100;
        let call_budget = content_budget * 8 / 100;
        let route_budget = content_budget * 15 / 100;
        let type_impl_budget = content_budget * 10 / 100;
        let di_binding_budget = content_budget * 10 / 100;
        let bridge_budget = content_budget * 5 / 100;
        let doc_budget = content_budget * 5 / 100;
        let doc_edge_budget = content_budget * 3 / 100;
        let file_budget = content_budget * 5 / 100;
        let code_edge_budget = content_budget * 3 / 100;
        let parser_budget = content_budget * 8 / 100;

        let bounded_symbols = bounded_facts(
            &self.code_graph.symbols,
            sym_budget,
            |left, right| {
                left.id
                    .cmp(&right.id)
                    .then(left.file.cmp(&right.file))
                    .then(left.name.cmp(&right.name))
            },
            |symbol| serialized_fact_tokens(symbol, 20),
        );

        let bounded_imports = bounded_facts(
            &self.code_graph.imports,
            imp_budget,
            |left, right| {
                left.id
                    .cmp(&right.id)
                    .then(left.file.cmp(&right.file))
                    .then(left.source.cmp(&right.source))
            },
            |import| serialized_fact_tokens(import, 15),
        );

        let bounded_calls = bounded_facts(
            &self.code_graph.calls,
            call_budget,
            |left, right| {
                left.id
                    .cmp(&right.id)
                    .then(left.file.cmp(&right.file))
                    .then(left.callee_text.cmp(&right.callee_text))
            },
            |call| serialized_fact_tokens(call, 15),
        );

        let bounded_routes = bounded_facts(
            &self.code_graph.routes,
            route_budget,
            |left, right| {
                left.id
                    .cmp(&right.id)
                    .then(left.file.cmp(&right.file))
                    .then(left.method.cmp(&right.method))
                    .then(left.path.cmp(&right.path))
                    .then(left.handler.cmp(&right.handler))
            },
            |route| serialized_fact_tokens(route, 15),
        );
        let bounded_type_impls = bounded_facts(
            &self.code_graph.type_impls,
            type_impl_budget,
            |left, right| {
                left.id
                    .cmp(&right.id)
                    .then(left.file.cmp(&right.file))
                    .then(left.implementing_type.cmp(&right.implementing_type))
                    .then(left.trait_or_interface.cmp(&right.trait_or_interface))
            },
            |type_impl| serialized_fact_tokens(type_impl, 15),
        );
        let bounded_di_bindings = bounded_facts(
            &self.code_graph.di_bindings,
            di_binding_budget,
            |left, right| {
                left.id
                    .cmp(&right.id)
                    .then(left.file.cmp(&right.file))
                    .then(left.abstract_type.cmp(&right.abstract_type))
                    .then(left.concrete_type.cmp(&right.concrete_type))
            },
            |binding| serialized_fact_tokens(binding, 15),
        );

        let bounded_bridge = bounded_facts(
            &self.bridge,
            bridge_budget,
            |left, right| {
                left.from
                    .cmp(&right.from)
                    .then(left.to.cmp(&right.to))
                    .then(format!("{:?}", left.edge_type).cmp(&format!("{:?}", right.edge_type)))
            },
            |edge| serialized_fact_tokens(edge, 15),
        );

        let bounded_docs = bounded_facts(
            &self.doc_graph.nodes,
            doc_budget,
            |left, right| left.id.cmp(&right.id).then(left.path.cmp(&right.path)),
            |doc| serialized_fact_tokens(doc, 20),
        );

        // Parser results are raw parser provenance, not a second representation
        // of semantic facts. Keep only parser facts for retained normalized
        // symbols, imports, and calls, then budget the complete result,
        // including backend metadata and diagnostics.
        let retained_fact_ids = bounded_symbols
            .iter()
            .map(|fact| fact.id.clone())
            .chain(bounded_imports.iter().map(|fact| fact.id.clone()))
            .chain(bounded_calls.iter().map(|fact| fact.id.clone()))
            .collect::<std::collections::HashSet<_>>();
        let parser_candidates = self
            .code_graph
            .parser_results
            .iter()
            .cloned()
            .filter_map(|mut result| {
                result
                    .facts
                    .retain(|fact| retained_fact_ids.contains(fact.id.as_str()));
                (!result.facts.is_empty()).then_some(result)
            })
            .collect::<Vec<_>>();
        let bounded_parser_results = bounded_facts(
            &parser_candidates,
            parser_budget,
            |left, right| left.file.cmp(&right.file),
            |result| serialized_fact_tokens(result, 20),
        );

        let retained_files = bounded_symbols
            .iter()
            .map(|fact| fact.file.as_str())
            .chain(bounded_imports.iter().map(|fact| fact.file.as_str()))
            .chain(bounded_calls.iter().map(|fact| fact.file.as_str()))
            .chain(bounded_routes.iter().map(|fact| fact.file.as_str()))
            .chain(bounded_type_impls.iter().map(|fact| fact.file.as_str()))
            .chain(bounded_di_bindings.iter().map(|fact| fact.file.as_str()))
            .chain(
                bounded_parser_results
                    .iter()
                    .map(|result| result.file.as_str()),
            )
            .collect::<std::collections::HashSet<_>>();
        let bounded_files = bounded_facts(
            &self
                .code_graph
                .files
                .iter()
                .filter(|file| retained_files.contains(file.path.as_str()))
                .cloned()
                .collect::<Vec<_>>(),
            file_budget,
            |left, right| left.path.cmp(&right.path),
            |file| serialized_fact_tokens(file, 10),
        );

        let retained_nodes = bounded_symbols
            .iter()
            .map(|fact| fact.id.as_str())
            .chain(bounded_imports.iter().map(|fact| fact.id.as_str()))
            .chain(bounded_calls.iter().map(|fact| fact.id.as_str()))
            .chain(bounded_files.iter().map(|file| file.path.as_str()))
            .collect::<std::collections::HashSet<_>>();
        let bounded_edges = bounded_facts(
            &self
                .code_graph
                .edges
                .iter()
                .filter(|edge| {
                    retained_nodes.contains(edge.from.as_str())
                        && retained_nodes.contains(edge.to.as_str())
                })
                .cloned()
                .collect::<Vec<_>>(),
            code_edge_budget,
            |left, right| {
                left.from
                    .cmp(&right.from)
                    .then(left.to.cmp(&right.to))
                    .then(format!("{:?}", left.edge_type).cmp(&format!("{:?}", right.edge_type)))
            },
            |edge| serialized_fact_tokens(edge, 10),
        );

        let bounded_doc_ids = bounded_docs
            .iter()
            .map(|doc| doc.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let bounded_doc_edges = bounded_facts(
            &self
                .doc_graph
                .edges
                .iter()
                .filter(|edge| bounded_doc_ids.contains(edge.from.as_str()))
                .cloned()
                .collect::<Vec<_>>(),
            doc_edge_budget,
            |left, right| {
                left.from
                    .cmp(&right.from)
                    .then(left.to.cmp(&right.to))
                    .then(format!("{:?}", left.edge_type).cmp(&format!("{:?}", right.edge_type)))
            },
            |edge| serialized_fact_tokens(edge, 10),
        );
        let mut bounded_code_graph = graxus_codemap::CodeGraph::from_parts(
            bounded_files,
            bounded_symbols,
            bounded_imports,
            bounded_calls,
            bounded_routes,
            bounded_type_impls,
            bounded_di_bindings,
            bounded_edges,
            Vec::new(),
            Vec::new(),
        );
        bounded_code_graph.parser_results = bounded_parser_results;

        AgentExport {
            project_name: self.project_name.clone(),
            doc_graph: DocGraph {
                nodes: bounded_docs,
                edges: bounded_doc_edges,
            },
            code_graph: bounded_code_graph,
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
    /// HTTP route facts.
    #[serde(default)]
    pub routes: usize,
    /// Trait/interface/inheritance relationship facts.
    #[serde(default)]
    pub type_impls: usize,
    /// Dependency-injection binding facts.
    #[serde(default)]
    pub di_bindings: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use graxus_codemap::facts::{DIFact, ImplKind, RouteFact, TypeImplFact};
    use graxus_codemap::{
        CallFact, CallKind, CodeEdge, CodeEdgeType, ConfidenceScore, FileNode, FileParserResult,
        ImportFact, ImportKind, ParserDiagnostic, ParserFact, ParserFactKind, ResolutionMethod,
        SymbolFact, SymbolKind, Visibility,
    };
    use graxus_core::ParserBackend;

    fn make_export() -> AgentExport {
        let doc_graph = DocGraph {
            nodes: vec![graxus_docgraph::graph::DocNode {
                id: "doc:README".into(),
                node_type: graxus_docgraph::graph::DocNodeType::Document,
                path: "README.md".into(),
                title: "README".into(),
                tags: vec![],
                frontmatter: None,
                headings: vec![],
                wiki_links: vec![],
            }],
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
                ..Default::default()
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
            routes: vec![
                RouteFact {
                    id: "route:z-auth".into(),
                    file: "src/auth.rs".into(),
                    language: "rust".into(),
                    method: "POST".into(),
                    path: "/auth/session".into(),
                    handler: "create_session".into(),
                    handler_file: Some("src/auth_handlers.rs".into()),
                    line: 18,
                    framework: "axum".into(),
                    middleware: vec!["require_auth".into()],
                },
                RouteFact {
                    id: "route:a-auth".into(),
                    file: "src/auth.rs".into(),
                    language: "rust".into(),
                    method: "GET".into(),
                    path: "/auth/session".into(),
                    handler: "current_session".into(),
                    handler_file: Some("src/auth_handlers.rs".into()),
                    line: 12,
                    framework: "axum".into(),
                    middleware: vec!["require_auth".into()],
                },
            ],
            type_impls: vec![TypeImplFact {
                id: "type-impl:auth-service".into(),
                file: "src/auth.rs".into(),
                language: "rust".into(),
                implementing_type: "AuthService".into(),
                trait_or_interface: "AuthContract".into(),
                line: 24,
                kind: ImplKind::TraitImpl,
            }],
            di_bindings: vec![DIFact {
                id: "di:auth-service".into(),
                file: "src/container.rs".into(),
                language: "rust".into(),
                abstract_type: "AuthContract".into(),
                concrete_type: "AuthService".into(),
                lifetime: Some("singleton".into()),
                line: 10,
                framework: "shuttle".into(),
            }],
            edges: vec![],
            type_hints: vec![],
            variables: vec![],
            decorators: vec![],
            macros: vec![],
            parser_results: vec![FileParserResult {
                file: "src/0.rs".into(),
                requested_backend: ParserBackend::Ripex,
                used_backend: ParserBackend::Ripex,
                fallback_reason: None,
                diagnostics: vec![],
                facts: vec![ParserFact {
                    id: "sym0".into(),
                    kind: ParserFactKind::Symbol,
                    data: serde_json::json!({"name": "func_0", "is_async": true}),
                }],
            }],
            indexes: std::sync::OnceLock::new(),
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
        assert_eq!(bounded.code_graph.parser_results.len(), 1);
        assert_eq!(bounded.code_graph.parser_results[0].facts.len(), 1);
        assert_eq!(bounded.code_graph.routes.len(), 2);
        assert_eq!(bounded.code_graph.type_impls.len(), 1);
        assert_eq!(bounded.code_graph.di_bindings.len(), 1);
        assert_eq!(
            bounded.code_graph.routes[0].middleware,
            vec!["require_auth".to_string()]
        );
    }

    #[test]
    fn test_export_bounded_limits_structural_collections_and_metadata() {
        let mut export = make_export();
        export.code_graph.files = (0..100)
            .map(|i| FileNode {
                path: format!("src/{i}.rs"),
                language: "rust".into(),
                hash: "abc".into(),
                size: 100,
            })
            .collect();
        export.code_graph.edges = (0..100)
            .map(|_| CodeEdge {
                from: "sym0".into(),
                to: "sym0".into(),
                edge_type: CodeEdgeType::Contains,
            })
            .collect();
        export.doc_graph.edges = (0..100)
            .map(|i| graxus_docgraph::graph::DocEdge {
                from: "doc:README".into(),
                to: format!("tag:{i}"),
                edge_type: graxus_docgraph::graph::DocEdgeType::HasTag,
            })
            .collect();
        export.code_graph.parser_results[0].diagnostics = vec![
            ParserDiagnostic {
                code: "E001".into(),
                message: "diagnostic payload ".repeat(100),
                line: 1,
                column: 1,
            };
            20
        ];

        let max_tokens = 2_000;
        let bounded = export.export_bounded(max_tokens);
        let serialized = serde_json::to_string(&bounded).expect("serialize bounded export");

        assert!(bounded.code_graph.files.len() < export.code_graph.files.len());
        assert!(bounded.code_graph.edges.len() < export.code_graph.edges.len());
        assert!(bounded.doc_graph.edges.len() < export.doc_graph.edges.len());
        let input_diagnostics = export.code_graph.parser_results[0].diagnostics.len();
        let output_diagnostics = bounded
            .code_graph
            .parser_results
            .iter()
            .map(|result| result.diagnostics.len())
            .sum::<usize>();
        assert!(output_diagnostics < input_diagnostics);
        assert!(estimate_tokens(&serialized) <= max_tokens);
    }

    #[test]
    fn test_export_stats() {
        let export = make_export();
        let stats = export.stats();
        assert_eq!(stats.doc_nodes, 1);
        assert_eq!(stats.symbols, 100);
        assert_eq!(stats.imports, 1);
        assert_eq!(stats.calls, 1);
        assert_eq!(stats.routes, 2);
        assert_eq!(stats.type_impls, 1);
        assert_eq!(stats.di_bindings, 1);
    }

    #[test]
    fn test_semantic_facts_round_trip_and_bounded_selection_are_deterministic() {
        let export = make_export();
        let ordinary: AgentExport =
            serde_json::from_str(&serde_json::to_string(&export).expect("serialize export"))
                .expect("deserialize export");
        assert_eq!(ordinary.code_graph.routes.len(), 2);
        assert_eq!(ordinary.code_graph.type_impls.len(), 1);
        assert_eq!(ordinary.code_graph.di_bindings.len(), 1);
        assert_eq!(
            ordinary.code_graph.di_bindings[0].lifetime.as_deref(),
            Some("singleton")
        );

        // Use an allocation that fits exactly one complete route fact. Selection
        // must not depend on the source collection's reverse lexical ordering.
        let largest_route = export
            .code_graph
            .routes
            .iter()
            .map(|route| serialized_fact_tokens(route, 15))
            .max()
            .expect("route fixture");
        let content_budget = (largest_route * 100).div_ceil(15);
        let max_tokens =
            content_budget + estimate_tokens(&export.project_name) + EXPORT_OVERHEAD_TOKENS;

        let first = export.export_bounded(max_tokens);
        let second = export.export_bounded(max_tokens);
        assert_eq!(first.code_graph.routes.len(), 1);
        assert_eq!(first.code_graph.routes[0].id, "route:a-auth");
        assert_eq!(
            serde_json::to_value(&first.code_graph.routes[0]).expect("serialize route"),
            serde_json::to_value(&export.code_graph.routes[1]).expect("serialize route")
        );
        assert_eq!(
            first
                .code_graph
                .routes
                .iter()
                .map(|route| route.id.as_str())
                .collect::<Vec<_>>(),
            second
                .code_graph
                .routes
                .iter()
                .map(|route| route.id.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_export_stats_default_missing_semantic_counts() {
        let stats: ExportStats = serde_json::from_value(serde_json::json!({
            "doc_nodes": 1,
            "doc_edges": 0,
            "code_files": 1,
            "symbols": 2,
            "imports": 3,
            "calls": 4,
            "bridge_edges": 5
        }))
        .expect("deserialize legacy stats");

        assert_eq!(stats.routes, 0);
        assert_eq!(stats.type_impls, 0);
        assert_eq!(stats.di_bindings, 0);
    }
}
