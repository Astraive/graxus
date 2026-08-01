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
    // Build lookups once so incremental callers can be resolved against the
    // complete graph, not just symbols extracted from the changed file.
    let mut syms_by_name: HashMap<&str, Vec<&SymbolFact>> = HashMap::new();
    for sym in symbols {
        syms_by_name.entry(sym.name.as_str()).or_default().push(sym);
    }

    let mut syms_by_file: HashMap<&str, Vec<&SymbolFact>> = HashMap::new();
    for sym in symbols {
        syms_by_file.entry(sym.file.as_str()).or_default().push(sym);
    }

    // Import bindings are file-local. A project-wide map keyed only by local
    // name can resolve a changed caller through an unrelated file's import.
    let mut imports_by_file: HashMap<(&str, &str), Vec<&ImportFact>> = HashMap::new();
    for import in imports {
        if let Some(local_name) = import.local_name.as_deref() {
            imports_by_file
                .entry((import.file.as_str(), local_name))
                .or_default()
                .push(import);
        }
    }

    for call in calls.iter_mut() {
        // Re-resolution is intentional: a merge can replace a target symbol
        // or invalidate an import while retaining the call fact.
        call.resolved_symbol = None;
        call.confidence = ConfidenceScore::unresolved();

        let callee = call.callee_text.as_str();
        let callee_name = callee
            .rsplit("::")
            .next()
            .unwrap_or(callee)
            .rsplit('.')
            .next()
            .unwrap_or(callee);

        // 1. Check if callee is a local definition in the same file. Qualified
        // calls must not accidentally bind to an unrelated local symbol with
        // the same final path segment.
        if callee == callee_name {
            if let Some(syms) = syms_by_name.get(callee_name) {
                if let Some(local) = syms.iter().find(|s| s.file == call.file) {
                    call.resolved_symbol = Some(format!("{}::{}", local.file, local.name));
                    call.confidence = ConfidenceScore::local_definition();
                    continue;
                }
            }
        }

        // 2. Check an import belonging to the file containing this call.
        if let Some(imports_for_name) = imports_by_file.get(&(call.file.as_str(), callee_name)) {
            if let Some(import) = imports_for_name
                .iter()
                .find(|import| import.resolved_file.is_some())
            {
                if let Some(resolved_file) = import.resolved_file.as_deref() {
                    let imported_name = import.imported_name.as_deref().unwrap_or(callee_name);
                    if let Some(target) = syms_by_file
                        .get(resolved_file)
                        .and_then(|file_syms| file_syms.iter().find(|s| s.name == imported_name))
                    {
                        call.resolved_symbol = Some(format!("{}::{}", target.file, target.name));
                    } else {
                        // Keep the historical path fallback for modules whose
                        // public API is not represented by a local symbol fact.
                        call.resolved_symbol =
                            Some(format!("{}::{}", resolved_file, imported_name));
                    }
                    call.confidence = ConfidenceScore::named_import_exact();
                    continue;
                }
            }
        }

        // 3. Check if the callee is a known symbol anywhere in the project.
        if let Some(syms) = syms_by_name.get(callee_name) {
            if let Some(first) = syms.first() {
                call.resolved_symbol = Some(format!("{}::{}", first.file, first.name));
                call.confidence = ConfidenceScore::same_project();
                continue;
            }
        }

        // 4. For method calls (obj.method()), try to resolve the object's type.
        if let Some(object) = call.object.as_deref() {
            if let Some(imports_for_object) = imports_by_file.get(&(call.file.as_str(), object)) {
                if let Some(import) = imports_for_object
                    .iter()
                    .find(|import| import.resolved_file.is_some())
                {
                    if let Some(resolved_file) = import.resolved_file.as_deref() {
                        if let Some(method) = syms_by_file
                            .get(resolved_file)
                            .and_then(|file_syms| file_syms.iter().find(|s| s.name == callee_name))
                        {
                            call.resolved_symbol =
                                Some(format!("{}::{}", method.file, method.name));
                        } else {
                            call.resolved_symbol =
                                Some(format!("{}::{}", resolved_file, callee_name));
                        }
                        call.confidence = ConfidenceScore::named_import_exact();
                        continue;
                    }
                }
            }

            // Check if object is a local variable with a known type.
            if let Some(local_syms) = syms_by_name.get(object) {
                if local_syms.iter().any(|s| s.file == call.file) {
                    if let Some(method) = syms_by_file
                        .get(call.file.as_str())
                        .and_then(|file_syms| file_syms.iter().find(|s| s.name == callee_name))
                    {
                        call.resolved_symbol = Some(format!("{}::{}", method.file, method.name));
                        call.confidence = ConfidenceScore::local_definition();
                        continue;
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CallKind, ImportKind};

    fn symbol(file: &str, name: &str) -> SymbolFact {
        SymbolFact {
            file: file.to_string(),
            name: name.to_string(),
            language: "rust".to_string(),
            ..Default::default()
        }
    }

    fn import(file: &str, resolved_file: &str) -> ImportFact {
        ImportFact {
            id: format!("import:{file}"),
            file: file.to_string(),
            language: "rust".to_string(),
            kind: ImportKind::RustUse,
            source: "crate::handler::run".to_string(),
            local_name: Some("run".to_string()),
            imported_name: None,
            resolved_file: Some(resolved_file.to_string()),
            line: 1,
            confidence: ConfidenceScore::named_import_exact(),
        }
    }

    fn call(file: &str, resolved_symbol: Option<&str>) -> CallFact {
        CallFact {
            id: format!("call:{file}"),
            file: file.to_string(),
            language: "rust".to_string(),
            kind: CallKind::FunctionCall,
            caller_symbol: None,
            callee_text: "run".to_string(),
            object: None,
            resolved_symbol: resolved_symbol.map(str::to_string),
            line: 2,
            column: 0,
            confidence: ConfidenceScore::named_import_exact(),
        }
    }

    #[test]
    fn scopes_imported_calls_to_their_calling_file() {
        let symbols = vec![
            symbol("src/one_handler.rs", "run"),
            symbol("src/two_handler.rs", "run"),
        ];
        let imports = vec![
            import("src/one.rs", "src/one_handler.rs"),
            import("src/two.rs", "src/two_handler.rs"),
        ];
        let mut calls = vec![
            call("src/one.rs", Some("stale::run")),
            call("src/two.rs", Some("stale::run")),
        ];

        resolve_calls(&mut calls, &symbols, &imports);

        assert_eq!(
            calls[0].resolved_symbol.as_deref(),
            Some("src/one_handler.rs::run")
        );
        assert_eq!(
            calls[1].resolved_symbol.as_deref(),
            Some("src/two_handler.rs::run")
        );
    }

    #[test]
    fn unresolved_calls_do_not_retain_stale_targets() {
        let mut calls = vec![call("src/main.rs", Some("src/old.rs::run"))];
        resolve_calls(&mut calls, &[], &[]);

        assert!(calls[0].resolved_symbol.is_none());
        assert_eq!(calls[0].confidence, ConfidenceScore::unresolved());
    }
}
