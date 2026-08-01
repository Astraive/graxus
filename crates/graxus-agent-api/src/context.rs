//! Context query engine — assembles structured context for AI agents.

use graxus_codemap::facts::{DIFact, RouteFact, TypeImplFact};
use graxus_codemap::{CallFact, CodeGraph, ImportFact, SymbolFact};
use graxus_docgraph::graph::{DocGraph, DocNode};
use serde::{Deserialize, Serialize};

use crate::bridge::{BridgeBuilder, BridgeEdge, BridgeEdgeType};

/// Type of context query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextQueryType {
    TextSearch,
    FileContext,
    SymbolContext,
    TopicContext,
}

/// A context query request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextQuery {
    pub query: String,
    pub query_type: ContextQueryType,
}

/// Structured context returned to AI agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub query: String,
    pub docs: Vec<DocNode>,
    pub code: Vec<SymbolFact>,
    pub imports: Vec<ImportFact>,
    pub calls: Vec<CallFact>,
    /// HTTP endpoints relevant to the query.
    #[serde(default)]
    pub routes: Vec<RouteFact>,
    /// Trait/interface/inheritance relationships relevant to the query.
    #[serde(default)]
    pub type_impls: Vec<TypeImplFact>,
    /// Dependency-injection bindings relevant to the query.
    #[serde(default)]
    pub di_bindings: Vec<DIFact>,
    pub related_files: Vec<String>,
    pub bridge_edges: Vec<BridgeEdge>,
    pub warnings: Vec<String>,
}

// ── Token Budget ──────────────────────────────────────────────────────────

/// Estimate tokens for a string.
///
/// Uses character count (not byte length) to handle multi-byte UTF-8 correctly.
/// Approximation: ~4 characters per token for English text.
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

/// Priority score for a context item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Import, // related via import
    Bridge, // connected via bridge
    Fuzzy,  // fuzzy/substring match
    Prefix, // prefix match
    Exact,  // exact name match (highest)
}

/// Token budget for bounded context queries.
///
/// Tracks remaining token capacity as items are added. Use [`consume`](Self::consume)
/// to attempt adding items within the budget.
pub struct ContextBudget {
    pub max_tokens: usize,
    pub used_tokens: usize,
}

impl ContextBudget {
    /// Create a new budget with the given maximum token limit.
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            used_tokens: 0,
        }
    }

    /// Return the number of tokens still available.
    pub fn remaining(&self) -> usize {
        self.max_tokens.saturating_sub(self.used_tokens)
    }

    /// Try to consume tokens. Returns false if would exceed budget.
    pub fn consume(&mut self, tokens: usize) -> bool {
        if tokens > self.max_tokens.saturating_sub(self.used_tokens) {
            false
        } else {
            self.used_tokens += tokens;
            true
        }
    }
}

/// A priority-scored context item for budget-aware assembly.
#[derive(Debug, Clone)]
pub struct ScoredItem<T> {
    pub item: T,
    pub priority: Priority,
    pub tokens: usize,
}

/// Engine for querying project context.
pub struct ContextEngine {
    doc_graph: DocGraph,
    code_graph: CodeGraph,
    bridge: Vec<BridgeEdge>,
}

impl ContextEngine {
    /// Create a new context engine from doc graph, code graph, and bridge edges.
    pub fn new(doc_graph: DocGraph, code_graph: CodeGraph, bridge: Vec<BridgeEdge>) -> Self {
        Self {
            doc_graph,
            code_graph,
            bridge,
        }
    }

    /// Build a context engine by automatically constructing the bridge.
    pub fn build(doc_graph: DocGraph, code_graph: CodeGraph) -> anyhow::Result<Self> {
        let bridge = BridgeBuilder::build(&doc_graph, &code_graph)?;
        Ok(Self::new(doc_graph, code_graph, bridge))
    }

    /// General text search across docs and code.
    pub fn query(&self, query: &str) -> AgentContext {
        let query_lower = query.to_lowercase();
        let mut context = AgentContext {
            query: query.to_string(),
            docs: Vec::new(),
            code: Vec::new(),
            imports: Vec::new(),
            calls: Vec::new(),
            routes: Vec::new(),
            type_impls: Vec::new(),
            di_bindings: Vec::new(),
            related_files: Vec::new(),
            bridge_edges: Vec::new(),
            warnings: Vec::new(),
        };

        // Search docs by title, tags, and path
        for node in &self.doc_graph.nodes {
            if node.title.to_lowercase().contains(&query_lower)
                || node
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&query_lower))
                || node.path.to_lowercase().contains(&query_lower)
            {
                context.docs.push(node.clone());
            }
        }

        // Search code symbols by name
        for symbol in &self.code_graph.symbols {
            if symbol.name.to_lowercase().contains(&query_lower) {
                context.code.push(symbol.clone());
            }
        }

        // Search framework and type-system semantics directly. These normalized
        // graph facts are intentionally used as-is; parser-native facts stay in
        // CodeGraph parser results and are not duplicated into agent context.
        for route in &self.code_graph.routes {
            if route.method.to_lowercase().contains(&query_lower)
                || route.path.to_lowercase().contains(&query_lower)
                || route.handler.to_lowercase().contains(&query_lower)
                || route.file.to_lowercase().contains(&query_lower)
                || route.framework.to_lowercase().contains(&query_lower)
                || route.language.to_lowercase().contains(&query_lower)
                || route
                    .handler_file
                    .as_deref()
                    .is_some_and(|file| file.to_lowercase().contains(&query_lower))
                || route
                    .middleware
                    .iter()
                    .any(|middleware| middleware.to_lowercase().contains(&query_lower))
            {
                context.routes.push(route.clone());
                context.related_files.push(route.file.clone());
                if let Some(handler_file) = &route.handler_file {
                    context.related_files.push(handler_file.clone());
                }
            }
        }

        for type_impl in &self.code_graph.type_impls {
            if type_impl.file.to_lowercase().contains(&query_lower)
                || type_impl.language.to_lowercase().contains(&query_lower)
                || type_impl
                    .implementing_type
                    .to_lowercase()
                    .contains(&query_lower)
                || type_impl
                    .trait_or_interface
                    .to_lowercase()
                    .contains(&query_lower)
            {
                context.type_impls.push(type_impl.clone());
                context.related_files.push(type_impl.file.clone());
            }
        }

        for binding in &self.code_graph.di_bindings {
            if binding.file.to_lowercase().contains(&query_lower)
                || binding.language.to_lowercase().contains(&query_lower)
                || binding.abstract_type.to_lowercase().contains(&query_lower)
                || binding.concrete_type.to_lowercase().contains(&query_lower)
                || binding.framework.to_lowercase().contains(&query_lower)
                || binding
                    .lifetime
                    .as_deref()
                    .is_some_and(|lifetime| lifetime.to_lowercase().contains(&query_lower))
            {
                context.di_bindings.push(binding.clone());
                context.related_files.push(binding.file.clone());
            }
        }

        // Search files by path
        for file in &self.code_graph.files {
            if file.path.to_lowercase().contains(&query_lower) {
                context.related_files.push(file.path.clone());
            }
        }

        // Collect related bridge edges
        for edge in &self.bridge {
            if edge.from.to_lowercase().contains(&query_lower)
                || edge.to.to_lowercase().contains(&query_lower)
            {
                context.bridge_edges.push(edge.clone());
            }
        }

        // Collect imports and calls for matched symbols
        let matched_files: Vec<&str> = context.code.iter().map(|s| s.file.as_str()).collect();

        for import in &self.code_graph.imports {
            if matched_files.contains(&import.file.as_str()) {
                context.imports.push(import.clone());
            }
        }

        for call in &self.code_graph.calls {
            if matched_files.contains(&call.file.as_str()) {
                context.calls.push(call.clone());
            }
        }

        context.related_files.sort();
        context.related_files.dedup();

        context
    }

    /// Get everything about a specific file.
    pub fn file_context(&self, path: &str) -> AgentContext {
        let mut context = AgentContext {
            query: path.to_string(),
            docs: Vec::new(),
            code: Vec::new(),
            imports: Vec::new(),
            calls: Vec::new(),
            routes: Vec::new(),
            type_impls: Vec::new(),
            di_bindings: Vec::new(),
            related_files: Vec::new(),
            bridge_edges: Vec::new(),
            warnings: Vec::new(),
        };

        // Symbols defined in this file
        context.code = self
            .code_graph
            .symbols_in_file(path)
            .into_iter()
            .cloned()
            .collect();

        // Imports in this file
        context.imports = self
            .code_graph
            .imports_in_file(path)
            .into_iter()
            .cloned()
            .collect();

        // Calls in this file
        context.calls = self
            .code_graph
            .calls_in_file(path)
            .into_iter()
            .cloned()
            .collect();

        context.routes = self
            .code_graph
            .routes
            .iter()
            .filter(|route| route.file == path || route.handler_file.as_deref() == Some(path))
            .cloned()
            .collect();
        context.type_impls = self
            .code_graph
            .type_impls
            .iter()
            .filter(|type_impl| type_impl.file == path)
            .cloned()
            .collect();
        context.di_bindings = self
            .code_graph
            .di_bindings
            .iter()
            .filter(|binding| binding.file == path)
            .cloned()
            .collect();

        // Docs that describe this file
        let doc_edges = BridgeBuilder::docs_for_code(&self.bridge, path);
        for edge in doc_edges {
            context.bridge_edges.push(edge.clone());
            // Find the doc node
            if let Some(node) = self.doc_graph.nodes.iter().find(|n| n.id == edge.from) {
                if !context.docs.iter().any(|d| d.id == node.id) {
                    context.docs.push(node.clone());
                }
            }
        }

        // Related files from imports
        for import in &context.imports {
            if let Some(ref resolved) = import.resolved_file {
                context.related_files.push(resolved.clone());
            }
        }

        // Add the file itself
        context.related_files.push(path.to_string());
        context.related_files.sort();
        context.related_files.dedup();

        context
    }

    /// Get everything about a specific symbol.
    pub fn symbol_context(&self, name: &str) -> AgentContext {
        let mut context = AgentContext {
            query: name.to_string(),
            docs: Vec::new(),
            code: Vec::new(),
            imports: Vec::new(),
            calls: Vec::new(),
            routes: Vec::new(),
            type_impls: Vec::new(),
            di_bindings: Vec::new(),
            related_files: Vec::new(),
            bridge_edges: Vec::new(),
            warnings: Vec::new(),
        };

        // Find the symbol definition
        let symbols = self.code_graph.find_symbols(name);
        for symbol in &symbols {
            context.code.push((*symbol).clone());
            context.related_files.push(symbol.file.clone());
        }

        // Find calls to this symbol
        if let Some(symbol) = symbols.first() {
            let symbol_id = &symbol.id;
            let callers = self.code_graph.calls_to_symbol(symbol_id);
            for call in callers {
                context.calls.push(call.clone());
                context.related_files.push(call.file.clone());
            }
        }

        // Find all call facts mentioning this symbol name
        for call in &self.code_graph.calls {
            if call.callee_text == name && !context.calls.iter().any(|c| c.id == call.id) {
                context.calls.push(call.clone());
                context.related_files.push(call.file.clone());
            }
        }

        // Find imports of this symbol
        for import in &self.code_graph.imports {
            if import.imported_name.as_deref() == Some(name)
                || import.local_name.as_deref() == Some(name)
            {
                context.imports.push(import.clone());
                context.related_files.push(import.file.clone());
            }
        }

        // Attach semantic facts when the symbol is their endpoint or contract.
        for route in &self.code_graph.routes {
            if route.handler == name {
                context.routes.push(route.clone());
                context.related_files.push(route.file.clone());
                if let Some(handler_file) = &route.handler_file {
                    context.related_files.push(handler_file.clone());
                }
            }
        }
        for type_impl in &self.code_graph.type_impls {
            if type_impl.implementing_type == name || type_impl.trait_or_interface == name {
                context.type_impls.push(type_impl.clone());
                context.related_files.push(type_impl.file.clone());
            }
        }
        for binding in &self.code_graph.di_bindings {
            if binding.abstract_type == name || binding.concrete_type == name {
                context.di_bindings.push(binding.clone());
                context.related_files.push(binding.file.clone());
            }
        }

        // Docs referencing this symbol
        for edge in &self.bridge {
            if matches!(edge.edge_type, BridgeEdgeType::DocReferencesSymbol) && edge.to == name {
                context.bridge_edges.push(edge.clone());
                if let Some(node) = self.doc_graph.nodes.iter().find(|n| n.id == edge.from) {
                    if !context.docs.iter().any(|d| d.id == node.id) {
                        context.docs.push(node.clone());
                    }
                }
            }
        }

        context.related_files.sort();
        context.related_files.dedup();

        context
    }

    /// Get everything about a topic (from docs).
    pub fn topic_context(&self, topic: &str) -> AgentContext {
        let topic_lower = topic.to_lowercase();
        let mut context = AgentContext {
            query: topic.to_string(),
            docs: Vec::new(),
            code: Vec::new(),
            imports: Vec::new(),
            calls: Vec::new(),
            routes: Vec::new(),
            type_impls: Vec::new(),
            di_bindings: Vec::new(),
            related_files: Vec::new(),
            bridge_edges: Vec::new(),
            warnings: Vec::new(),
        };

        // Find docs matching the topic
        for node in &self.doc_graph.nodes {
            let matches_title = node.title.to_lowercase().contains(&topic_lower);
            let matches_tag = node
                .tags
                .iter()
                .any(|t| t.to_lowercase().contains(&topic_lower));
            let matches_heading = node
                .headings
                .iter()
                .any(|h| h.text.to_lowercase().contains(&topic_lower));

            if matches_title || matches_tag || matches_heading {
                context.docs.push(node.clone());
            }
        }

        // Get code referenced by matched docs
        for doc in &context.docs {
            let code_edges = BridgeBuilder::code_for_doc(&self.bridge, &doc.id);
            for edge in code_edges {
                context.bridge_edges.push(edge.clone());
                context.related_files.push(edge.to.clone());
            }
        }

        // Get symbols and imports for related files
        for file_path in &context.related_files {
            for symbol in self.code_graph.symbols_in_file(file_path) {
                context.code.push(symbol.clone());
            }
            for import in self.code_graph.imports_in_file(file_path) {
                context.imports.push(import.clone());
            }
            for call in self.code_graph.calls_in_file(file_path) {
                context.calls.push(call.clone());
            }
        }

        context.related_files.sort();
        context.related_files.dedup();

        for file_path in &context.related_files {
            context.routes.extend(
                self.code_graph
                    .routes
                    .iter()
                    .filter(|route| {
                        route.file == *file_path || route.handler_file.as_deref() == Some(file_path)
                    })
                    .cloned(),
            );
            context.type_impls.extend(
                self.code_graph
                    .type_impls
                    .iter()
                    .filter(|type_impl| type_impl.file == *file_path)
                    .cloned(),
            );
            context.di_bindings.extend(
                self.code_graph
                    .di_bindings
                    .iter()
                    .filter(|binding| binding.file == *file_path)
                    .cloned(),
            );
        }

        context
    }

    /// Get a reference to the doc graph.
    pub fn doc_graph(&self) -> &DocGraph {
        &self.doc_graph
    }

    /// Get a reference to the code graph.
    pub fn code_graph(&self) -> &CodeGraph {
        &self.code_graph
    }

    /// Get a reference to the bridge edges.
    pub fn bridge(&self) -> &[BridgeEdge] {
        &self.bridge
    }

    /// Budget-aware query: scores all matches by priority, fills budget highest-priority first.
    ///
    /// Scores items by matching strength, then fills the token budget from
    /// highest-priority down. Items are cloned only once, directly into the
    /// final context — no intermediate allocations for rejected items.
    pub fn query_bounded(&self, query: &str, mut budget: ContextBudget) -> AgentContext {
        let query_lower = query.to_lowercase();

        // Scored indices: (kind, source_index, priority, estimated_tokens).
        // Kinds: 0=doc, 1=code, 2=file, 3=bridge, 4=route, 5=type impl, 6=DI.
        let mut scored: Vec<(u8, usize, Priority, usize)> = Vec::new();

        // Score docs
        for (i, node) in self.doc_graph.nodes.iter().enumerate() {
            let title_lower = node.title.to_lowercase();
            let priority = if title_lower == query_lower {
                Priority::Exact
            } else if title_lower.starts_with(&query_lower) {
                Priority::Prefix
            } else if title_lower.contains(&query_lower)
                || node
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&query_lower))
                || node.path.to_lowercase().contains(&query_lower)
            {
                Priority::Fuzzy
            } else {
                continue;
            };
            let tokens = estimate_tokens(&node.title) + node.tags.len() * 5;
            scored.push((0, i, priority, tokens));
        }

        // Score code symbols
        for (i, symbol) in self.code_graph.symbols.iter().enumerate() {
            let name_lower = symbol.name.to_lowercase();
            let priority = if name_lower == query_lower {
                Priority::Exact
            } else if name_lower.starts_with(&query_lower) {
                Priority::Prefix
            } else if name_lower.contains(&query_lower) {
                Priority::Fuzzy
            } else {
                continue;
            };
            let tokens = estimate_tokens(&symbol.name) + estimate_tokens(&symbol.file) + 20;
            scored.push((1, i, priority, tokens));
        }

        // Score files
        for (i, file) in self.code_graph.files.iter().enumerate() {
            if file.path.to_lowercase().contains(&query_lower) {
                let tokens = estimate_tokens(&file.path) + 10;
                scored.push((2, i, Priority::Fuzzy, tokens));
            }
        }

        // Score bridge edges
        for (i, edge) in self.bridge.iter().enumerate() {
            if edge.from.to_lowercase().contains(&query_lower)
                || edge.to.to_lowercase().contains(&query_lower)
            {
                let tokens = estimate_tokens(&edge.from) + estimate_tokens(&edge.to) + 10;
                scored.push((3, i, Priority::Bridge, tokens));
            }
        }

        // Score normalized semantic facts using their agent-visible fields.
        for (i, route) in self.code_graph.routes.iter().enumerate() {
            let route_text = [
                route.method.as_str(),
                route.path.as_str(),
                route.handler.as_str(),
                route.file.as_str(),
                route.language.as_str(),
                route.framework.as_str(),
            ];
            if route_text
                .iter()
                .any(|field| field.to_lowercase().contains(&query_lower))
                || route
                    .handler_file
                    .as_deref()
                    .is_some_and(|file| file.to_lowercase().contains(&query_lower))
                || route
                    .middleware
                    .iter()
                    .any(|middleware| middleware.to_lowercase().contains(&query_lower))
            {
                let tokens = estimate_tokens(&route.method)
                    + estimate_tokens(&route.path)
                    + estimate_tokens(&route.handler)
                    + estimate_tokens(&route.file)
                    + estimate_tokens(&route.framework)
                    + estimate_tokens(&route.language)
                    + route
                        .middleware
                        .iter()
                        .map(|middleware| estimate_tokens(middleware))
                        .sum::<usize>()
                    + 15;
                scored.push((4, i, Priority::Fuzzy, tokens));
            }
        }

        for (i, type_impl) in self.code_graph.type_impls.iter().enumerate() {
            if type_impl.file.to_lowercase().contains(&query_lower)
                || type_impl.language.to_lowercase().contains(&query_lower)
                || type_impl
                    .implementing_type
                    .to_lowercase()
                    .contains(&query_lower)
                || type_impl
                    .trait_or_interface
                    .to_lowercase()
                    .contains(&query_lower)
            {
                let tokens = estimate_tokens(&type_impl.implementing_type)
                    + estimate_tokens(&type_impl.trait_or_interface)
                    + estimate_tokens(&type_impl.file)
                    + 15;
                scored.push((5, i, Priority::Fuzzy, tokens));
            }
        }

        for (i, binding) in self.code_graph.di_bindings.iter().enumerate() {
            if binding.file.to_lowercase().contains(&query_lower)
                || binding.language.to_lowercase().contains(&query_lower)
                || binding.abstract_type.to_lowercase().contains(&query_lower)
                || binding.concrete_type.to_lowercase().contains(&query_lower)
                || binding.framework.to_lowercase().contains(&query_lower)
                || binding
                    .lifetime
                    .as_deref()
                    .is_some_and(|lifetime| lifetime.to_lowercase().contains(&query_lower))
            {
                let tokens = estimate_tokens(&binding.abstract_type)
                    + estimate_tokens(&binding.concrete_type)
                    + estimate_tokens(&binding.file)
                    + binding.lifetime.as_deref().map_or(0, estimate_tokens)
                    + 15;
                scored.push((6, i, Priority::Fuzzy, tokens));
            }
        }

        // Sort by priority (highest first), then token cost, fact category, and
        // source position so every bounded query has a deterministic selection.
        scored.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then(a.3.cmp(&b.3))
                .then(a.0.cmp(&b.0))
                .then(a.1.cmp(&b.1))
        });

        let mut context = AgentContext {
            query: query.to_string(),
            docs: Vec::new(),
            code: Vec::new(),
            imports: Vec::new(),
            calls: Vec::new(),
            routes: Vec::new(),
            type_impls: Vec::new(),
            di_bindings: Vec::new(),
            related_files: Vec::new(),
            bridge_edges: Vec::new(),
            warnings: Vec::new(),
        };

        // Fill budget — clone only items that fit
        for (kind, idx, _priority, tokens) in scored {
            match kind {
                0 if budget.consume(tokens) => {
                    context.docs.push(self.doc_graph.nodes[idx].clone());
                }
                1 if budget.consume(tokens) => {
                    context.code.push(self.code_graph.symbols[idx].clone());
                }
                2 if budget.consume(tokens) => {
                    context
                        .related_files
                        .push(self.code_graph.files[idx].path.clone());
                }
                3 if budget.consume(tokens) => {
                    context.bridge_edges.push(self.bridge[idx].clone());
                }
                4 if budget.consume(tokens) => {
                    context.routes.push(self.code_graph.routes[idx].clone());
                }
                5 if budget.consume(tokens) => {
                    context
                        .type_impls
                        .push(self.code_graph.type_impls[idx].clone());
                }
                6 if budget.consume(tokens) => {
                    context
                        .di_bindings
                        .push(self.code_graph.di_bindings[idx].clone());
                }
                _ => {}
            }
        }

        // Collect imports and calls for matched code files (budget-aware)
        let matched_files: Vec<&str> = context.code.iter().map(|s| s.file.as_str()).collect();
        for import in &self.code_graph.imports {
            if matched_files.contains(&import.file.as_str()) {
                let tokens = estimate_tokens(&import.source) + 10;
                if budget.consume(tokens) {
                    context.imports.push(import.clone());
                }
            }
        }
        for call in &self.code_graph.calls {
            if matched_files.contains(&call.file.as_str()) {
                let tokens = estimate_tokens(&call.callee_text) + 10;
                if budget.consume(tokens) {
                    context.calls.push(call.clone());
                }
            }
        }

        context.related_files.sort();
        context.related_files.dedup();

        if budget.remaining() == 0 {
            context
                .warnings
                .push("Context truncated due to token budget".to_string());
        }

        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graxus_codemap::facts::{DIFact, ImplKind, RouteFact, TypeImplFact};
    use graxus_codemap::FileNode;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("goodbye"), 2); // 5 chars / 4 rounded up
        assert_eq!(estimate_tokens("a".repeat(100).as_str()), 25);
    }

    #[test]
    fn test_context_budget_consume() {
        let mut budget = ContextBudget::new(100);
        assert_eq!(budget.remaining(), 100);
        assert!(budget.consume(50));
        assert_eq!(budget.remaining(), 50);
        assert!(!budget.consume(60)); // would exceed
        assert_eq!(budget.remaining(), 50); // unchanged
        assert!(budget.consume(50));
        assert_eq!(budget.remaining(), 0);
        assert!(!budget.consume(1));
    }

    #[test]
    fn test_context_budget_rejects_overflow_without_panicking() {
        let mut budget = ContextBudget::new(usize::MAX);
        assert!(budget.consume(usize::MAX));
        assert!(!budget.consume(1));
        assert_eq!(budget.used_tokens, usize::MAX);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Exact > Priority::Prefix);
        assert!(Priority::Prefix > Priority::Fuzzy);
        assert!(Priority::Fuzzy > Priority::Bridge);
        assert!(Priority::Bridge > Priority::Import);
    }

    #[test]
    fn test_scored_item_sort() {
        let mut items = [
            ScoredItem {
                item: "a",
                priority: Priority::Fuzzy,
                tokens: 10,
            },
            ScoredItem {
                item: "b",
                priority: Priority::Exact,
                tokens: 10,
            },
            ScoredItem {
                item: "c",
                priority: Priority::Bridge,
                tokens: 10,
            },
        ];
        items.sort_by_key(|item| std::cmp::Reverse(item.priority));
        assert_eq!(items[0].item, "b"); // Exact first
        assert_eq!(items[1].item, "a"); // Fuzzy second
        assert_eq!(items[2].item, "c"); // Bridge third
    }
    #[test]
    fn test_semantic_facts_are_exposed_by_context_queries() {
        let graph = CodeGraph::from_parts(
            vec![FileNode {
                path: "src/auth.rs".into(),
                language: "rust".into(),
                hash: "abc".into(),
                size: 100,
            }],
            vec![],
            vec![],
            vec![],
            vec![RouteFact {
                id: "route:auth".into(),
                file: "src/auth.rs".into(),
                language: "rust".into(),
                method: "POST".into(),
                path: "/auth/session".into(),
                handler: "auth_handler".into(),
                handler_file: Some("src/auth.rs".into()),
                line: 12,
                framework: "axum".into(),
                middleware: vec!["require_auth".into()],
            }],
            vec![TypeImplFact {
                id: "type-impl:auth".into(),
                file: "src/auth.rs".into(),
                language: "rust".into(),
                implementing_type: "AuthService".into(),
                trait_or_interface: "AuthContract".into(),
                line: 18,
                kind: ImplKind::TraitImpl,
            }],
            vec![DIFact {
                id: "di:auth".into(),
                file: "src/container.rs".into(),
                language: "rust".into(),
                abstract_type: "AuthContract".into(),
                concrete_type: "AuthService".into(),
                lifetime: Some("singleton".into()),
                line: 8,
                framework: "shuttle".into(),
            }],
            vec![],
            vec![],
            vec![],
        );
        let engine = ContextEngine::new(DocGraph::new(), graph, vec![]);

        let context = engine.query("auth");
        assert_eq!(context.routes[0].path, "/auth/session");
        assert_eq!(context.type_impls[0].trait_or_interface, "AuthContract");
        assert_eq!(context.di_bindings[0].concrete_type, "AuthService");
        assert_eq!(
            context.related_files,
            vec!["src/auth.rs".to_string(), "src/container.rs".to_string()]
        );

        let file_context = engine.file_context("src/auth.rs");
        assert_eq!(file_context.routes.len(), 1);
        assert_eq!(file_context.type_impls.len(), 1);
        assert!(file_context.di_bindings.is_empty());

        let symbol_context = engine.symbol_context("AuthContract");
        assert_eq!(symbol_context.type_impls.len(), 1);
        assert_eq!(symbol_context.di_bindings.len(), 1);

        let bounded = engine.query_bounded("auth", ContextBudget::new(1_000));
        assert_eq!(bounded.routes.len(), 1);
        assert_eq!(bounded.type_impls.len(), 1);
        assert_eq!(bounded.di_bindings.len(), 1);

        let legacy: AgentContext = serde_json::from_value(serde_json::json!({
            "query": "auth",
            "docs": [],
            "code": [],
            "imports": [],
            "calls": [],
            "related_files": [],
            "bridge_edges": [],
            "warnings": []
        }))
        .expect("deserialize legacy context");
        assert!(legacy.routes.is_empty());
        assert!(legacy.type_impls.is_empty());
        assert!(legacy.di_bindings.is_empty());
    }
}
