use std::collections::BTreeSet;

use tree_sitter::Node;

use super::{FrameworkDescriptor, FrameworkResolver};
use crate::facts::RouteFact;

#[derive(Debug, Default, Clone, Copy)]
pub struct NextJsResolver;

impl FrameworkResolver for NextJsResolver {
    fn descriptor(&self) -> FrameworkDescriptor {
        FrameworkDescriptor {
            name: "nextjs",
            language: "typescript",
        }
    }

    fn extract_routes(&self, file: &str, source: &str) -> Vec<RouteFact> {
        let Some((path, language)) = next_route_path(file) else {
            return Vec::new();
        };
        let Some(tree) = parse_typescript(source) else {
            return Vec::new();
        };

        let mut routes = Vec::new();
        let mut seen = BTreeSet::new();
        visit_nodes(tree.root_node(), &mut |node| {
            let Some(method) = exported_route_method(node, source) else {
                return;
            };
            let Some(handler) = node
                .child_by_field_name("name")
                .and_then(|name| source_text(name, source))
                .map(str::to_owned)
            else {
                return;
            };

            if seen.insert((method.to_owned(), path.clone(), handler.clone())) {
                let line = node.start_position().row + 1;
                routes.push(RouteFact {
                    id: format!("route:nextjs:{file}:{line}:{method}:{path}:{handler}"),
                    file: file.to_owned(),
                    language: language.to_owned(),
                    method: method.to_owned(),
                    path: path.clone(),
                    handler,
                    handler_file: None,
                    line,
                    framework: "nextjs".to_owned(),
                    middleware: Vec::new(),
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
    NextJsResolver
}

fn parse_typescript(source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .ok()?;
    parser.parse(source, None)
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

fn exported_route_method(node: Node<'_>, source: &str) -> Option<&'static str> {
    if node.kind() != "function_declaration" || node.parent()?.kind() != "export_statement" {
        return None;
    }

    let name = node
        .child_by_field_name("name")
        .and_then(|name| source_text(name, source))?;
    match name {
        "GET" => Some("GET"),
        "POST" => Some("POST"),
        "PUT" => Some("PUT"),
        "PATCH" => Some("PATCH"),
        "DELETE" => Some("DELETE"),
        "HEAD" => Some("HEAD"),
        "OPTIONS" => Some("OPTIONS"),
        _ => None,
    }
}

fn next_route_path(file: &str) -> Option<(String, &'static str)> {
    let normalized = file.replace('\\', "/");
    let mut parts: Vec<_> = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    let filename = parts.pop()?;
    let language = match filename {
        "route.js" | "route.jsx" | "route.mjs" => "javascript",
        "route.ts" | "route.tsx" | "route.mts" => "typescript",
        _ => return None,
    };

    let app_directory = parts.iter().rposition(|part| *part == "app")?;
    let mut path_segments = Vec::new();
    for segment in &parts[app_directory + 1..] {
        if segment.starts_with('(') && segment.ends_with(')') {
            continue;
        }
        path_segments.push(next_path_segment(segment));
    }

    if path_segments.is_empty() {
        Some(("/".to_owned(), language))
    } else {
        Some((format!("/{}", path_segments.join("/")), language))
    }
}

fn next_path_segment(segment: &str) -> String {
    if let Some(name) = segment
        .strip_prefix("[[...")
        .and_then(|name| name.strip_suffix("]]"))
    {
        return format!("*{name}");
    }
    if let Some(name) = segment
        .strip_prefix("[...")
        .and_then(|name| name.strip_suffix(']'))
    {
        return format!("*{name}");
    }
    if let Some(name) = segment
        .strip_prefix('[')
        .and_then(|name| name.strip_suffix(']'))
    {
        return format!(":{name}");
    }

    segment.to_owned()
}

fn source_text<'source>(node: Node<'_>, source: &'source str) -> Option<&'source str> {
    source.get(node.byte_range())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_exported_next_route_handlers() {
        let source = r#"
            export async function GET(request: Request) {
                return Response.json({});
            }

            export function POST() {
                return new Response();
            }

            function PUT() {}
            export const DELETE = () => new Response();
        "#;

        let routes = resolver().extract_routes("src/app/api/users/route.ts", source);

        assert_eq!(routes.len(), 2);
        assert_eq!(
            routes
                .iter()
                .map(|route| (
                    route.method.as_str(),
                    route.path.as_str(),
                    route.handler.as_str(),
                    route.language.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("GET", "/api/users", "GET", "typescript"),
                ("POST", "/api/users", "POST", "typescript")
            ]
        );
        assert!(routes.iter().all(|route| {
            route.framework == "nextjs" && route.file == "src/app/api/users/route.ts"
        }));
    }

    #[test]
    fn extracts_javascript_route_language_and_metadata() {
        let source = r#"
            export async function GET(request) {
                return Response.json({});
            }
        "#;

        let routes = resolver().extract_routes("src/app/api/users/route.js", source);

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].language, "javascript");
        assert_eq!(routes[0].path, "/api/users");
        assert_eq!(routes[0].handler, "GET");
    }

    #[test]
    fn normalizes_dynamic_route_segments_and_ignores_non_route_files() {
        let source = "export async function GET() {}";

        let routes = resolver().extract_routes("app/users/[id]/route.ts", source);

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].path, "/users/:id");
        assert!(resolver()
            .extract_routes("app/users/[id]/handler.ts", source)
            .is_empty());
        assert!(resolver().extract_routes("route.ts", source).is_empty());
    }
}
