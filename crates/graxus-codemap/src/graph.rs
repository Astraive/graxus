//! Call graph traversal utilities.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::{CodeGraph, SymbolFact};

/// Call graph with caller/callee relationships between symbols.
pub struct CallGraph {
    /// symbol_key -> Vec<symbol_key> (who calls this symbol)
    callers: HashMap<String, Vec<String>>,
    /// symbol_key -> Vec<symbol_key> (what this symbol calls)
    callees: HashMap<String, Vec<String>>,
}

/// Make a canonical symbol key from file + name in "file::name" format.
fn symbol_key(file: &str, name: &str) -> String {
    format!("{}::{}", file, name)
}

impl CallGraph {
    /// Build a CallGraph from a CodeGraph.
    pub fn from_code_graph(graph: &CodeGraph) -> Self {
        let mut callers: HashMap<String, Vec<String>> = HashMap::new();
        let mut callees: HashMap<String, Vec<String>> = HashMap::new();

        // Index all symbols by name for caller lookup
        let symbol_map: HashMap<&str, Vec<&SymbolFact>> = {
            let mut m: HashMap<&str, Vec<&SymbolFact>> = HashMap::new();
            for sym in &graph.symbols {
                m.entry(sym.name.as_str()).or_default().push(sym);
            }
            m
        };

        for call in &graph.calls {
            if let Some(ref resolved) = call.resolved_symbol {
                // resolved_symbol is already in "file::name" format
                let callee_key = resolved.clone();

                // Find the caller symbol: the function that contains this call
                let caller_key = call.caller_symbol.as_deref().map(|s| s.to_string());

                if let Some(ref caller) = caller_key {
                    callees
                        .entry(caller.clone())
                        .or_default()
                        .push(callee_key.clone());
                    callers
                        .entry(callee_key.clone())
                        .or_default()
                        .push(caller.clone());
                }
            } else {
                // Try to resolve by callee_text
                if let Some(syms) = symbol_map.get(call.callee_text.as_str()) {
                    for sym in syms {
                        let callee_key = symbol_key(&sym.file, &sym.name);
                        let caller_key = call.caller_symbol.as_deref().map(|s| s.to_string());
                        if let Some(ref caller) = caller_key {
                            callees
                                .entry(caller.clone())
                                .or_default()
                                .push(callee_key.clone());
                            callers.entry(callee_key).or_default().push(caller.clone());
                        }
                    }
                }
            }
        }

        // Deduplicate
        for v in callers.values_mut() {
            v.sort();
            v.dedup();
        }
        for v in callees.values_mut() {
            v.sort();
            v.dedup();
        }

        Self { callers, callees }
    }

    /// Get who calls this symbol.
    pub fn callers_of(&self, symbol: &str) -> Vec<&str> {
        self.callers
            .get(symbol)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get what this symbol calls.
    pub fn callees_of(&self, symbol: &str) -> Vec<&str> {
        self.callees
            .get(symbol)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// BFS upward: who calls this symbol transitively.
    pub fn transitive_callers(&self, symbol: &str, max_depth: usize) -> Vec<(String, usize)> {
        self.bfs(symbol, max_depth, true)
    }

    /// BFS downward: what does this symbol call transitively.
    pub fn transitive_callees(&self, symbol: &str, max_depth: usize) -> Vec<(String, usize)> {
        self.bfs(symbol, max_depth, false)
    }

    /// Blast radius: all symbols transitively affected by changes to this symbol.
    pub fn blast_radius(&self, symbol: &str, max_depth: usize) -> HashSet<String> {
        self.transitive_callers(symbol, max_depth)
            .into_iter()
            .map(|(s, _)| s)
            .collect()
    }

    /// Find cycles in the call graph.
    pub fn cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();
        let mut path = Vec::new();

        for node in self.callees.keys() {
            if !visited.contains(node) {
                self.dfs_cycles(node, &mut visited, &mut in_stack, &mut path, &mut cycles);
            }
        }
        cycles
    }

    fn bfs(&self, start: &str, max_depth: usize, reverse: bool) -> Vec<(String, usize)> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((start.to_string(), 0usize));
        visited.insert(start.to_string());

        while let Some((node, depth)) = queue.pop_front() {
            if depth > 0 {
                result.push((node.clone(), depth));
            }
            if depth >= max_depth {
                continue;
            }
            let neighbors = if reverse {
                self.callers.get(&node).cloned().unwrap_or_default()
            } else {
                self.callees.get(&node).cloned().unwrap_or_default()
            };
            for neighbor in neighbors {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor.clone());
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }
        result
    }

    fn dfs_cycles(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        in_stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(neighbors) = self.callees.get(node) {
            for neighbor in neighbors {
                if in_stack.contains(neighbor) {
                    // Found a cycle
                    if let Some(start) = path.iter().position(|n| n == neighbor) {
                        cycles.push(path[start..].to_vec());
                    }
                } else if !visited.contains(neighbor) {
                    self.dfs_cycles(neighbor, visited, in_stack, path, cycles);
                }
            }
        }

        path.pop();
        in_stack.remove(node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CallFact, CallKind, ConfidenceScore, SymbolFact, SymbolKind, Visibility};

    fn make_call(file: &str, caller: &str, callee: &str, resolved: &str) -> CallFact {
        CallFact {
            id: String::new(),
            file: file.to_string(),
            language: "rust".to_string(),
            kind: CallKind::FunctionCall,
            caller_symbol: Some(caller.to_string()),
            callee_text: callee.to_string(),
            object: None,
            resolved_symbol: Some(resolved.to_string()),
            line: 1,
            column: 0,
            confidence: ConfidenceScore::named_import_exact(),
        }
    }

    fn make_symbol(file: &str, name: &str) -> SymbolFact {
        SymbolFact {
            id: format!("symbol:{}:{}", file, name),
            file: file.to_string(),
            language: "rust".to_string(),
            kind: SymbolKind::Function,
            name: name.to_string(),
            exported: true,
            line_start: 1,
            line_end: 10,
            visibility: Visibility::Public,
            signature: String::new(),
            is_test: false,
            usage_count: 0,
            ..Default::default()
        }
    }

    #[test]
    fn test_callers_of() {
        let graph = CodeGraph {
            files: vec![],
            symbols: vec![make_symbol("a.rs", "foo"), make_symbol("b.rs", "bar")],
            imports: vec![],
            calls: vec![make_call("b.rs", "b.rs::bar", "foo", "a.rs::foo")],
            edges: vec![],
            type_hints: vec![],
            ..Default::default()
        };
        let cg = CallGraph::from_code_graph(&graph);
        let callers = cg.callers_of("a.rs::foo");
        assert_eq!(callers, vec!["b.rs::bar"]);
    }

    #[test]
    fn test_callees_of() {
        let graph = CodeGraph {
            files: vec![],
            symbols: vec![make_symbol("a.rs", "foo"), make_symbol("b.rs", "bar")],
            imports: vec![],
            calls: vec![make_call("a.rs", "a.rs::foo", "bar", "b.rs::bar")],
            edges: vec![],
            type_hints: vec![],
            ..Default::default()
        };
        let cg = CallGraph::from_code_graph(&graph);
        let callees = cg.callees_of("a.rs::foo");
        assert_eq!(callees, vec!["b.rs::bar"]);
    }

    #[test]
    fn test_transitive_callers() {
        let graph = CodeGraph {
            files: vec![],
            symbols: vec![
                make_symbol("x.rs", "a"),
                make_symbol("x.rs", "b"),
                make_symbol("x.rs", "c"),
            ],
            imports: vec![],
            calls: vec![
                make_call("x.rs", "x.rs::b", "a", "x.rs::a"),
                make_call("x.rs", "x.rs::c", "b", "x.rs::b"),
            ],
            edges: vec![],
            type_hints: vec![],
            ..Default::default()
        };
        let cg = CallGraph::from_code_graph(&graph);
        let callers = cg.transitive_callers("x.rs::a", 5);
        let names: Vec<&str> = callers.iter().map(|(s, _)| s.as_str()).collect();
        assert!(names.contains(&"x.rs::b"));
        assert!(names.contains(&"x.rs::c"));
    }

    #[test]
    fn test_blast_radius() {
        let graph = CodeGraph {
            files: vec![],
            symbols: vec![
                make_symbol("x.rs", "a"),
                make_symbol("x.rs", "b"),
                make_symbol("x.rs", "c"),
            ],
            imports: vec![],
            calls: vec![
                make_call("x.rs", "x.rs::b", "a", "x.rs::a"),
                make_call("x.rs", "x.rs::c", "b", "x.rs::b"),
            ],
            edges: vec![],
            type_hints: vec![],
            ..Default::default()
        };
        let cg = CallGraph::from_code_graph(&graph);
        let radius = cg.blast_radius("x.rs::a", 5);
        assert!(radius.contains("x.rs::b"));
        assert!(radius.contains("x.rs::c"));
        assert!(!radius.contains("x.rs::a"));
    }

    #[test]
    fn test_cycles() {
        let graph = CodeGraph {
            files: vec![],
            symbols: vec![make_symbol("x.rs", "a"), make_symbol("x.rs", "b")],
            imports: vec![],
            calls: vec![
                make_call("x.rs", "x.rs::a", "b", "x.rs::b"),
                make_call("x.rs", "x.rs::b", "a", "x.rs::a"),
            ],
            edges: vec![],
            type_hints: vec![],
            ..Default::default()
        };
        let cg = CallGraph::from_code_graph(&graph);
        let cycles = cg.cycles();
        assert!(!cycles.is_empty());
    }
}
