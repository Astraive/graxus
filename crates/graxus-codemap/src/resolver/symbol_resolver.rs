use std::collections::HashMap;

use crate::{CallFact, ConfidenceScore, ImportFact, SymbolFact};

/// Build a lookup from symbol name to all matching symbol facts.
pub fn build_symbol_lookup(symbols: &[SymbolFact]) -> HashMap<String, Vec<&SymbolFact>> {
    let mut map: HashMap<String, Vec<&SymbolFact>> = HashMap::new();
    for sym in symbols {
        map.entry(sym.name.clone()).or_default().push(sym);
    }
    map
}

/// Resolve call sites to their target symbols.
/// Modifies calls in-place, setting `resolved_symbol` and `confidence`.
pub fn resolve_calls(calls: &mut [CallFact], symbols: &[SymbolFact], imports: &[ImportFact]) {
    // Build lookup: name -> list of symbols with that name
    let mut syms_by_name: HashMap<&str, Vec<&SymbolFact>> = HashMap::new();
    for sym in symbols {
        syms_by_name.entry(sym.name.as_str()).or_default().push(sym);
    }

    // Build lookup: local_name -> import (for resolving imported calls)
    let imports_by_local: HashMap<&str, &ImportFact> = imports
        .iter()
        .filter_map(|i| i.local_name.as_deref().map(|n| (n, i)))
        .collect();

    for call in calls.iter_mut() {
        let callee = &call.callee_text;

        // 1. Check if callee is a local definition in the same file
        if let Some(syms) = syms_by_name.get(callee.as_str()) {
            if let Some(local) = syms.iter().find(|s| s.file == call.file) {
                call.resolved_symbol = Some(format!("{}::{}", local.file, local.name));
                call.confidence = ConfidenceScore::local_definition();
                continue;
            }
        }

        // 2. Check if callee is an imported symbol
        if let Some(imp) = imports_by_local.get(callee.as_str()) {
            if let Some(ref resolved_file) = imp.resolved_file {
                let symbol = imp.imported_name.as_deref().unwrap_or(callee);
                call.resolved_symbol = Some(format!("{}::{}", resolved_file, symbol));
                call.confidence = ConfidenceScore::named_import_exact();
                continue;
            }
        }

        // 3. Check if callee is a known symbol anywhere in the project
        if let Some(syms) = syms_by_name.get(callee.as_str()) {
            if let Some(first) = syms.first() {
                call.resolved_symbol = Some(format!("{}::{}", first.file, first.name));
                call.confidence = ConfidenceScore::same_project();
                continue;
            }
        }

        // 4. For method calls, try to resolve the object
        if let Some(ref object) = call.object {
            if let Some(imp) = imports_by_local.get(object.as_str()) {
                if let Some(ref resolved_file) = imp.resolved_file {
                    call.resolved_symbol = Some(format!("{}::{}", resolved_file, callee));
                    call.confidence = ConfidenceScore::named_import_exact();
                    continue;
                }
            }
        }

        // 5. Unknown
        call.confidence = ConfidenceScore::unresolved();
    }
}
