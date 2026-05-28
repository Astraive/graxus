//! Context query engine — assembles structured context for AI agents.

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
    pub related_files: Vec<String>,
    pub bridge_edges: Vec<BridgeEdge>,
    pub warnings: Vec<String>,
}

// ── Token Budget ──────────────────────────────────────────────────────────

/// Estimate tokens for a string (rough: chars / 4).
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() + 3) / 4
}

/// Priority score for a context item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Import,   // related via import
    Bridge,   // connected via bridge
    Fuzzy,    // fuzzy/substring match
    Prefix,   // prefix match
    Exact,    // exact name match (highest)
}

/// Token budget for bounded context queries.
pub struct ContextBudget {
    pub max_tokens: usize,
    pub used_tokens: usize,
}

impl ContextBudget {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens, used_tokens: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.max_tokens.saturating_sub(self.used_tokens)
    }

    /// Try to consume tokens. Returns false if would exceed budget.
    pub fn consume(&mut self, tokens: usize) -> bool {
        if self.used_tokens + tokens > self.max_tokens {
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
    pub fn new(
        doc_graph: DocGraph,
        code_graph: CodeGraph,
        bridge: Vec<BridgeEdge>,
    ) -> Self {
        Self {
            doc_graph,
            code_graph,
            bridge,
        }
    }

    /// Build a context engine by automatically constructing the bridge.
    pub fn build(
        doc_graph: DocGraph,
        code_graph: CodeGraph,
    ) -> anyhow::Result<Self> {
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
            related_files: Vec::new(),
            bridge_edges: Vec::new(),
            warnings: Vec::new(),
        };

        // Search docs by title, tags, and path
        for node in &self.doc_graph.nodes {
            if node.title.to_lowercase().contains(&query_lower)
                || node.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
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
        let matched_files: Vec<&str> = context
            .code
            .iter()
            .map(|s| s.file.as_str())
            .collect();

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
            related_files: Vec::new(),
            bridge_edges: Vec::new(),
            warnings: Vec::new(),
        };

        // Symbols defined in this file
        context.code = self.code_graph.symbols_in_file(path).into_iter().cloned().collect();

        // Imports in this file
        context.imports = self.code_graph.imports_in_file(path).into_iter().cloned().collect();

        // Calls in this file
        context.calls = self.code_graph.calls_in_file(path).into_iter().cloned().collect();

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
            if call.callee_text == name
                && !context.calls.iter().any(|c| c.id == call.id)
            {
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
            related_files: Vec::new(),
            bridge_edges: Vec::new(),
            warnings: Vec::new(),
        };

        // Find docs matching the topic
        for node in &self.doc_graph.nodes {
            let matches_title = node.title.to_lowercase().contains(&topic_lower);
            let matches_tag = node.tags.iter().any(|t| t.to_lowercase().contains(&topic_lower));
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
    pub fn query_bounded(&self, query: &str, mut budget: ContextBudget) -> AgentContext {
        let query_lower = query.to_lowercase();

        let mut scored_docs: Vec<ScoredItem<DocNode>> = Vec::new();
        let mut scored_code: Vec<ScoredItem<SymbolFact>> = Vec::new();
        let mut scored_files: Vec<ScoredItem<String>> = Vec::new();
        let _scored_imports: Vec<ScoredItem<ImportFact>> = Vec::new();
        let _scored_calls: Vec<ScoredItem<CallFact>> = Vec::new();
        let mut scored_bridges: Vec<ScoredItem<BridgeEdge>> = Vec::new();
        let mut warnings = Vec::new();

        // Score docs
        for node in &self.doc_graph.nodes {
            let title_lower = node.title.to_lowercase();
            let priority = if title_lower == query_lower {
                Priority::Exact
            } else if title_lower.starts_with(&query_lower) {
                Priority::Prefix
            } else if title_lower.contains(&query_lower)
                || node.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            {
                Priority::Fuzzy
            } else if node.path.to_lowercase().contains(&query_lower) {
                Priority::Fuzzy
            } else {
                continue;
            };
            let tokens = estimate_tokens(&node.title) + node.tags.len() * 5;
            scored_docs.push(ScoredItem { item: node.clone(), priority, tokens });
        }

        // Score code symbols
        for symbol in &self.code_graph.symbols {
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
            scored_code.push(ScoredItem { item: symbol.clone(), priority, tokens });
        }

        // Score files
        for file in &self.code_graph.files {
            if file.path.to_lowercase().contains(&query_lower) {
                let tokens = estimate_tokens(&file.path) + 10;
                scored_files.push(ScoredItem {
                    item: file.path.clone(), priority: Priority::Fuzzy, tokens,
                });
            }
        }

        // Score bridge edges
        for edge in &self.bridge {
            if edge.from.to_lowercase().contains(&query_lower)
                || edge.to.to_lowercase().contains(&query_lower)
            {
                let tokens = estimate_tokens(&edge.from) + estimate_tokens(&edge.to) + 10;
                scored_bridges.push(ScoredItem {
                    item: edge.clone(), priority: Priority::Bridge, tokens,
                });
            }
        }

        // Sort all by priority (highest first)
        scored_docs.sort_by(|a, b| b.priority.cmp(&a.priority));
        scored_code.sort_by(|a, b| b.priority.cmp(&a.priority));
        scored_files.sort_by(|a, b| b.priority.cmp(&a.priority));
        scored_bridges.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Interleave items by priority, filling budget
        let mut context = AgentContext {
            query: query.to_string(),
            docs: Vec::new(),
            code: Vec::new(),
            imports: Vec::new(),
            calls: Vec::new(),
            related_files: Vec::new(),
            bridge_edges: Vec::new(),
            warnings: Vec::new(),
        };

        // Flatten all scored items into a single priority-sorted stream
        let mut all_scored: Vec<(&str, Priority, usize)> = Vec::new(); // (kind, priority, index)
        for (i, s) in scored_docs.iter().enumerate() { all_scored.push(("doc", s.priority, i)); }
        for (i, s) in scored_code.iter().enumerate() { all_scored.push(("code", s.priority, i)); }
        for (i, s) in scored_files.iter().enumerate() { all_scored.push(("file", s.priority, i)); }
        for (i, s) in scored_bridges.iter().enumerate() { all_scored.push(("bridge", s.priority, i)); }
        all_scored.sort_by(|a, b| b.1.cmp(&a.1));

        for (kind, _priority, idx) in all_scored {
            match kind {
                "doc" => {
                    let item = &scored_docs[idx];
                    if budget.consume(item.tokens) {
                        context.docs.push(item.item.clone());
                    }
                }
                "code" => {
                    let item = &scored_code[idx];
                    if budget.consume(item.tokens) {
                        context.code.push(item.item.clone());
                    }
                }
                "file" => {
                    let item = &scored_files[idx];
                    if budget.consume(item.tokens) {
                        context.related_files.push(item.item.clone());
                    }
                }
                "bridge" => {
                    let item = &scored_bridges[idx];
                    if budget.consume(item.tokens) {
                        context.bridge_edges.push(item.item.clone());
                    }
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
            warnings.push("Context truncated due to token budget".to_string());
        }
        context.warnings = warnings;

        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hello"), 2);  // 5 chars / 4 rounded up
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
    fn test_priority_ordering() {
        assert!(Priority::Exact > Priority::Prefix);
        assert!(Priority::Prefix > Priority::Fuzzy);
        assert!(Priority::Fuzzy > Priority::Bridge);
        assert!(Priority::Bridge > Priority::Import);
    }

    #[test]
    fn test_scored_item_sort() {
        let mut items = vec![
            ScoredItem { item: "a", priority: Priority::Fuzzy, tokens: 10 },
            ScoredItem { item: "b", priority: Priority::Exact, tokens: 10 },
            ScoredItem { item: "c", priority: Priority::Bridge, tokens: 10 },
        ];
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
        assert_eq!(items[0].item, "b"); // Exact first
        assert_eq!(items[1].item, "a"); // Fuzzy second
        assert_eq!(items[2].item, "c"); // Bridge third
    }
}
