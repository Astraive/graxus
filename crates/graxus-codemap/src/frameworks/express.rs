use std::collections::BTreeSet;

use tree_sitter::Node;

use super::{FrameworkDescriptor, FrameworkResolver};
use crate::facts::RouteFact;

#[derive(Debug, Default, Clone, Copy)]
pub struct ExpressResolver;

impl FrameworkResolver for ExpressResolver {
    fn descriptor(&self) -> FrameworkDescriptor {
        FrameworkDescriptor {
            name: "express",
            language: "javascript",
        }
    }

    fn extract_routes(&self, file: &str, source: &str) -> Vec<RouteFact> {
        self.extract_routes_for_language(file, source, self.descriptor().language)
    }

    fn extract_routes_with_language(
        &self,
        file: &str,
        source: &str,
        language: &str,
    ) -> Vec<RouteFact> {
        self.extract_routes_for_language(file, source, language)
    }
}

impl ExpressResolver {
    fn extract_routes_for_language(
        &self,
        file: &str,
        source: &str,
        language: &str,
    ) -> Vec<RouteFact> {
        let Some(tree) = parse_typescript(source) else {
            return Vec::new();
        };
        if !has_express_dependency(tree.root_node(), source) {
            return Vec::new();
        }

        let mut routes = Vec::new();
        let mut seen = BTreeSet::new();
        visit_nodes(tree.root_node(), &mut |node| {
            let Some((method, path, handler, middleware)) = express_route(node, source) else {
                return;
            };

            if seen.insert((method.to_owned(), path.clone(), handler.clone())) {
                let line = node.start_position().row + 1;
                routes.push(RouteFact {
                    id: format!("route:express:{file}:{line}:{method}:{path}:{handler}"),
                    file: file.to_owned(),
                    language: language.to_owned(),
                    method: method.to_owned(),
                    path,
                    handler,
                    handler_file: None,
                    line,
                    framework: "express".to_owned(),
                    middleware,
                });
            }
        });

        routes.sort_by(|left, right| {
            (
                left.method.as_str(),
                left.path.as_str(),
                left.handler.as_str(),
                left.line,
            )
                .cmp(&(
                    right.method.as_str(),
                    right.path.as_str(),
                    right.handler.as_str(),
                    right.line,
                ))
        });
        routes
    }
}

pub fn resolver() -> impl FrameworkResolver {
    ExpressResolver
}

fn parse_typescript(source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .ok()?;
    parser.parse(source, None)
}

fn has_express_dependency(root: Node<'_>, source: &str) -> bool {
    let mut found = false;
    visit_nodes(root, &mut |node| {
        if found {
            return;
        }

        if node.kind() == "import_statement" {
            if let Some(module) = node
                .child_by_field_name("source")
                .and_then(|module| literal_path(module, source))
            {
                found = module == "express";
            }
        } else if node.kind() == "call_expression" {
            let is_require = node
                .child_by_field_name("function")
                .and_then(|function| source_text(function, source))
                == Some("require");
            if is_require {
                if let Some(module) = node
                    .child_by_field_name("arguments")
                    .and_then(|arguments| arguments.named_child(0))
                    .and_then(|module| literal_path(module, source))
                {
                    found = module == "express";
                }
            }
        }
    });
    found
}

fn visit_nodes<'tree, F>(node: Node<'tree>, f: &mut F)
where
    F: FnMut(Node<'tree>),
{
    f(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_nodes(child, f);
    }
}

fn express_route(
    node: Node<'_>,
    source: &str,
) -> Option<(&'static str, String, String, Vec<String>)> {
    if node.kind() != "call_expression" {
        return None;
    }

    let function = node.child_by_field_name("function")?;
    if function.kind() != "member_expression" {
        return None;
    }

    let receiver = function.child_by_field_name("object")?;
    if !is_express_receiver(source_text(receiver, source)?) {
        return None;
    }

    let property = function.child_by_field_name("property")?;
    let method = http_method(source_text(property, source)?)?;
    let arguments = node.child_by_field_name("arguments")?;
    let path = normalize_path(literal_path(arguments.named_child(0)?, source)?);
    let handler_index = (1..arguments.named_child_count())
        .rev()
        .find_map(|index| handler_name(arguments.named_child(index)?, source).map(|_| index))?;
    let handler = handler_name(arguments.named_child(handler_index)?, source)?;
    let middleware = (1..handler_index)
        .filter_map(|index| handler_name(arguments.named_child(index)?, source))
        .collect();

    Some((method, path, handler, middleware))
}

fn is_express_receiver(receiver: &str) -> bool {
    let name = receiver.rsplit('.').next().unwrap_or(receiver);
    let lower = name.to_ascii_lowercase();
    lower == "app" || lower == "router" || lower.ends_with("app") || lower.ends_with("router")
}

fn http_method(name: &str) -> Option<&'static str> {
    match name {
        "get" => Some("GET"),
        "post" => Some("POST"),
        "put" => Some("PUT"),
        "patch" => Some("PATCH"),
        "delete" => Some("DELETE"),
        "head" => Some("HEAD"),
        "options" => Some("OPTIONS"),
        "all" => Some("ALL"),
        _ => None,
    }
}

fn literal_path(node: Node<'_>, source: &str) -> Option<String> {
    let raw = source_text(node, source)?;
    match node.kind() {
        "string" if raw.len() >= 2 => Some(raw[1..raw.len() - 1].to_owned()),
        "template_string" if raw.len() >= 2 && !raw.contains("${") => {
            Some(raw[1..raw.len() - 1].to_owned())
        }
        _ => None,
    }
}

fn normalize_path(path: String) -> String {
    if path.is_empty() {
        "/".to_owned()
    } else if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    }
}

fn handler_name(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" | "member_expression" => source_text(node, source).map(str::to_owned),
        "function_expression" => node
            .child_by_field_name("name")
            .and_then(|name| source_text(name, source))
            .map(str::to_owned),
        _ => None,
    }
}

fn source_text<'source>(node: Node<'_>, source: &'source str) -> Option<&'source str> {
    source.get(node.byte_range())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_literal_app_and_router_routes_once() {
        let source = r#"
            import express from "express";
            app.get("/users", list_users);
            router.post('/users', require_auth, create_user);
            app.get("/users", list_users);
            app.get(dynamic_path, ignored);
            client.get("/not-an-express-route", ignored);
        "#;

        let routes = resolver().extract_routes("src/routes.js", source);

        assert_eq!(routes.len(), 2);
        assert_eq!(
            routes
                .iter()
                .map(|route| (
                    route.method.as_str(),
                    route.path.as_str(),
                    route.handler.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("GET", "/users", "list_users"),
                ("POST", "/users", "create_user")
            ]
        );
        assert_eq!(
            routes
                .iter()
                .find(|route| route.method == "POST")
                .map(|route| route.middleware.clone()),
            Some(vec!["require_auth".to_owned()])
        );
        assert!(routes.iter().all(|route| route.language == "javascript"));
        assert!(routes
            .iter()
            .all(|route| route.framework == "express" && route.file == "src/routes.js"));
    }

    #[test]
    fn extracts_typescript_routes_with_dispatch_language() {
        let source = r#"
            import express from "express";
            type Handler = () => void;
            const app = express();
            app.get("/users", list_users);
        "#;

        let direct = resolver().extract_routes_with_language("src/routes.ts", source, "typescript");
        assert_eq!(direct.len(), 1);
        assert_eq!(direct[0].language, "typescript");
        assert_eq!(direct[0].path, "/users");
        assert_eq!(direct[0].handler, "list_users");

        let dispatched = super::super::extract_routes("src/routes.ts", source, "typescript");
        assert_eq!(
            dispatched
                .iter()
                .filter(|route| route.language == "typescript")
                .count(),
            1
        );
        assert_eq!(dispatched[0].path, "/users");
        assert_eq!(dispatched[0].handler, "list_users");
    }

    #[test]
    fn tolerates_incomplete_route_registration() {
        let routes = resolver().extract_routes(
            "src/routes.ts",
            "const express = require('express'); app.get('/users', list_users); app.get(",
        );

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].method, "GET");
        assert_eq!(routes[0].path, "/users");
    }

    #[test]
    fn requires_literal_express_package_evidence() {
        let routes = resolver().extract_routes("src/routes.ts", "app.get('/users', list_users);");

        assert!(routes.is_empty());
    }
}
