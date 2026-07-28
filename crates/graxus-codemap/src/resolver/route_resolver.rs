use std::collections::HashSet;

use crate::{facts::RouteFact, SymbolFact};

/// Resolve each framework route handler to its defining source file.
///
/// A same-file definition is always preferred. Cross-file linking is only
/// accepted when a handler name has one unambiguous definition; guessing among
/// overloads or same-named functions would mislead downstream impact analysis.
pub fn resolve_routes(mut routes: Vec<RouteFact>, symbols: &[SymbolFact]) -> Vec<RouteFact> {
    for route in &mut routes {
        if route.handler_file.is_some() || route.handler.is_empty() {
            continue;
        }

        let mut matches = symbols.iter().filter(|symbol| symbol.name == route.handler);
        let same_file = matches.clone().find(|symbol| symbol.file == route.file);
        route.handler_file = same_file
            .or_else(|| {
                let first = matches.next()?;
                matches.next().is_none().then_some(first)
            })
            .map(|symbol| symbol.file.clone());
    }

    routes.sort_by(|left, right| {
        (
            left.file.as_str(),
            left.framework.as_str(),
            left.method.as_str(),
            left.path.as_str(),
            left.handler.as_str(),
            left.line,
        )
            .cmp(&(
                right.file.as_str(),
                right.framework.as_str(),
                right.method.as_str(),
                right.path.as_str(),
                right.handler.as_str(),
                right.line,
            ))
    });

    let mut seen = HashSet::new();
    routes.retain(|route| {
        seen.insert((
            route.file.clone(),
            route.framework.clone(),
            route.method.clone(),
            route.path.clone(),
            route.handler.clone(),
            route.line,
        ))
    });
    routes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SymbolKind, Visibility};

    fn symbol(file: &str, name: &str) -> SymbolFact {
        SymbolFact {
            file: file.into(),
            name: name.into(),
            kind: SymbolKind::Function,
            visibility: Visibility::Public,
            ..Default::default()
        }
    }

    fn route(file: &str, handler: &str) -> RouteFact {
        RouteFact {
            id: String::new(),
            file: file.into(),
            language: "rust".into(),
            method: "GET".into(),
            path: "/users".into(),
            handler: handler.into(),
            handler_file: None,
            line: 1,
            framework: "axum".into(),
            middleware: Vec::new(),
        }
    }

    #[test]
    fn resolves_same_file_before_cross_file_matches() {
        let routes = resolve_routes(
            vec![route("src/api.rs", "list_users")],
            &[
                symbol("src/other.rs", "list_users"),
                symbol("src/api.rs", "list_users"),
            ],
        );

        assert_eq!(routes[0].handler_file.as_deref(), Some("src/api.rs"));
    }

    #[test]
    fn leaves_ambiguous_cross_file_handler_unresolved() {
        let routes = resolve_routes(
            vec![route("src/api.rs", "list_users")],
            &[
                symbol("src/a.rs", "list_users"),
                symbol("src/b.rs", "list_users"),
            ],
        );

        assert_eq!(routes[0].handler_file, None);
    }
}
