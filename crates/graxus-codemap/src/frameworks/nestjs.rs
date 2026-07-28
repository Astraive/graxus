use std::collections::BTreeSet;

use tree_sitter::Node;

use super::{FrameworkDescriptor, FrameworkResolver};
use crate::facts::RouteFact;

#[derive(Debug, Default, Clone, Copy)]
pub struct NestJsResolver;

impl FrameworkResolver for NestJsResolver {
    fn descriptor(&self) -> FrameworkDescriptor {
        FrameworkDescriptor {
            name: "nestjs",
            language: "typescript",
        }
    }

    fn extract_routes(&self, file: &str, source: &str) -> Vec<RouteFact> {
        let Some(tree) = parse_typescript(source) else {
            return Vec::new();
        };
        if !has_nest_dependency(tree.root_node(), source) {
            return Vec::new();
        }

        let mut routes = Vec::new();
        let mut seen = BTreeSet::new();
        visit_nodes(tree.root_node(), &mut |node| {
            if node.kind() != "class_declaration" {
                return;
            }

            for route in controller_routes(node, source) {
                if seen.insert((
                    route.method.to_owned(),
                    route.path.clone(),
                    route.handler.clone(),
                )) {
                    routes.push(RouteFact {
                        id: format!(
                            "route:nestjs:{file}:{}:{}:{}:{}",
                            route.line, route.method, route.path, route.handler
                        ),
                        file: file.to_owned(),
                        language: "typescript".to_owned(),
                        method: route.method.to_owned(),
                        path: route.path,
                        handler: route.handler,
                        handler_file: None,
                        line: route.line,
                        framework: "nestjs".to_owned(),
                        middleware: Vec::new(),
                    });
                }
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
    NestJsResolver
}

struct ControllerRoute {
    method: &'static str,
    path: String,
    handler: String,
    line: usize,
}

fn parse_typescript(source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .ok()?;
    parser.parse(source, None)
}

fn has_nest_dependency(root: Node<'_>, source: &str) -> bool {
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
                found = module == "@nestjs" || module.starts_with("@nestjs/");
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
                    found = module == "@nestjs" || module.starts_with("@nestjs/");
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

fn controller_routes(class: Node<'_>, source: &str) -> Vec<ControllerRoute> {
    let Some(Some(base_path)) = class_decorators(class)
        .into_iter()
        .find_map(|decorator| controller_path(decorator, source))
    else {
        return Vec::new();
    };
    let Some(body) = class.child_by_field_name("body") else {
        return Vec::new();
    };

    let mut routes = Vec::new();
    let mut pending_decorators = Vec::new();
    let mut cursor = body.walk();
    for member in body.named_children(&mut cursor) {
        match member.kind() {
            "decorator" => pending_decorators.push(member),
            "method_definition" => {
                let decorators = if pending_decorators.is_empty() {
                    class_decorators(member)
                } else {
                    std::mem::take(&mut pending_decorators)
                };
                let Some(handler) = member
                    .child_by_field_name("name")
                    .and_then(|name| source_text(name, source))
                    .map(str::to_owned)
                else {
                    continue;
                };

                for decorator in decorators {
                    let Some((method, subpath)) = route_decorator(decorator, source) else {
                        continue;
                    };
                    routes.push(ControllerRoute {
                        method,
                        path: join_paths(&base_path, &subpath),
                        handler: handler.clone(),
                        line: decorator.start_position().row + 1,
                    });
                }
            }
            _ => pending_decorators.clear(),
        }
    }
    routes
}

fn class_decorators(node: Node<'_>) -> Vec<Node<'_>> {
    let mut decorators = direct_decorators(node);
    if node.kind() == "class_declaration" {
        if let Some(parent) = node.parent() {
            if parent.kind() == "export_statement" {
                decorators.extend(direct_decorators(parent));
            }
        }
    }
    decorators
}

fn direct_decorators(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() == "decorator")
        .collect()
}

fn controller_path(decorator: Node<'_>, source: &str) -> Option<Option<String>> {
    let (function, arguments) = decorator_invocation(decorator)?;
    if source_text(function, source)? != "Controller" {
        return None;
    }

    let path = match arguments.and_then(|arguments| arguments.named_child(0)) {
        Some(argument) => literal_path(argument, source).map(normalize_path),
        None => Some("/".to_owned()),
    };
    Some(path)
}

fn route_decorator(decorator: Node<'_>, source: &str) -> Option<(&'static str, String)> {
    let (function, arguments) = decorator_invocation(decorator)?;
    let method = match source_text(function, source)? {
        "Get" => "GET",
        "Post" => "POST",
        "Put" => "PUT",
        "Patch" => "PATCH",
        "Delete" => "DELETE",
        "Head" => "HEAD",
        "Options" => "OPTIONS",
        "All" => "ALL",
        _ => return None,
    };
    let path = match arguments.and_then(|arguments| arguments.named_child(0)) {
        Some(argument) => normalize_path(literal_path(argument, source)?),
        None => "/".to_owned(),
    };

    Some((method, path))
}

fn decorator_invocation(decorator: Node<'_>) -> Option<(Node<'_>, Option<Node<'_>>)> {
    if decorator.kind() != "decorator" {
        return None;
    }

    let expression = decorator.named_child(0)?;
    match expression.kind() {
        "identifier" => Some((expression, None)),
        "call_expression" => Some((
            expression.child_by_field_name("function")?,
            expression.child_by_field_name("arguments"),
        )),
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

fn join_paths(base: &str, subpath: &str) -> String {
    if base == "/" {
        return subpath.to_owned();
    }
    if subpath == "/" {
        return base.to_owned();
    }

    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        subpath.trim_start_matches('/')
    )
}

fn source_text<'source>(node: Node<'_>, source: &'source str) -> Option<&'source str> {
    source.get(node.byte_range())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_controller_method_decorators() {
        let source = r#"
            import { Controller, Get, Post } from "@nestjs/common";
            @Controller("/users")
            export class UsersController {
                @Get()
                list_users() {}

                @Post(":id")
                create_user() {}

                @Get(dynamic_path)
                dynamic_route() {}
            }
        "#;

        let routes = resolver().extract_routes("src/users.controller.ts", source);

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
                ("POST", "/users/:id", "create_user")
            ]
        );
        assert!(routes.iter().all(|route| {
            route.framework == "nestjs" && route.file == "src/users.controller.ts"
        }));
    }

    #[test]
    fn does_not_duplicate_repeated_method_decorators() {
        let source = r#"
            import { Controller, Get } from "@nestjs/common";
            @Controller("health")
            class HealthController {
                @Get()
                @Get()
                ready() {}
            }
        "#;

        let routes = resolver().extract_routes("src/health.controller.ts", source);

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].method, "GET");
        assert_eq!(routes[0].path, "/health");
        assert_eq!(routes[0].handler, "ready");
    }

    #[test]
    fn requires_literal_nest_package_evidence() {
        let source = r#"
            @Controller("users")
            class UsersController {
                @Get()
                list_users() {}
            }
        "#;

        assert!(resolver()
            .extract_routes("src/users.controller.ts", source)
            .is_empty());
    }
}
