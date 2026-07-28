use std::collections::BTreeSet;

use tree_sitter::Node;

use crate::facts::RouteFact;

use super::{FrameworkDescriptor, FrameworkResolver};

#[derive(Debug, Default, Clone, Copy)]
pub struct AxumResolver;

impl FrameworkResolver for AxumResolver {
    fn descriptor(&self) -> FrameworkDescriptor {
        FrameworkDescriptor {
            name: "axum",
            language: "rust",
        }
    }

    fn extract_routes(&self, file: &str, source: &str) -> Vec<RouteFact> {
        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_rust::LANGUAGE.into();
        if parser.set_language(&language).is_err() {
            return Vec::new();
        }
        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };
        if !has_axum_evidence(tree.root_node(), source) {
            return Vec::new();
        }

        let mut routes = Vec::new();
        let mut seen = BTreeSet::new();
        collect_route_calls(tree.root_node(), source, file, &mut routes, &mut seen);
        routes
    }
}

pub fn resolver() -> impl FrameworkResolver {
    AxumResolver
}

fn has_axum_evidence(node: Node<'_>, source: &str) -> bool {
    match node.kind() {
        "use_declaration" | "extern_crate_declaration"
            if imports_crate(node_text(node, source).unwrap_or_default(), "axum") =>
        {
            return true;
        }
        "scoped_identifier"
            if node_text(node, source).is_some_and(|path| path.starts_with("axum::")) =>
        {
            return true;
        }
        _ => {}
    }

    (0..node.named_child_count()).any(|index| {
        node.named_child(index)
            .is_some_and(|child| has_axum_evidence(child, source))
    })
}

fn imports_crate(declaration: &str, crate_name: &str) -> bool {
    let declaration = declaration.trim_start();
    let root = declaration
        .strip_prefix("use ")
        .or_else(|| declaration.strip_prefix("extern crate "))
        .map(str::trim_start)
        .and_then(|path| {
            path.split(|character: char| {
                character == ':'
                    || character == '{'
                    || character == ';'
                    || character.is_ascii_whitespace()
            })
            .next()
        });
    root == Some(crate_name)
}

fn collect_route_calls(
    node: Node<'_>,
    source: &str,
    file: &str,
    routes: &mut Vec<RouteFact>,
    seen: &mut BTreeSet<(String, String, String)>,
) {
    if let Some((path, router)) = axum_route_arguments(node, source) {
        let mut handlers = Vec::new();
        collect_method_handlers(router, source, &mut handlers);
        for (method, handler) in handlers {
            push_route(
                routes,
                seen,
                file,
                node.start_position().row + 1,
                method,
                path.clone(),
                handler,
            );
        }
    }

    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            collect_route_calls(child, source, file, routes, seen);
        }
    }
}

fn axum_route_arguments<'tree>(node: Node<'tree>, source: &str) -> Option<(String, Node<'tree>)> {
    if node.kind() != "call_expression" {
        return None;
    }

    let function = node.child_by_field_name("function")?;
    if function.kind() != "field_expression"
        || node_text(function.child_by_field_name("field")?, source)? != "route"
    {
        return None;
    }

    let arguments = node.child_by_field_name("arguments")?;
    let path = string_literal_value(node_text(arguments.named_child(0)?, source)?)?;
    Some((path, arguments.named_child(1)?))
}

fn collect_method_handlers(node: Node<'_>, source: &str, handlers: &mut Vec<(String, String)>) {
    if node.kind() != "call_expression" {
        return;
    }

    let Some(function) = node.child_by_field_name("function") else {
        return;
    };
    if function.kind() == "field_expression" {
        if let Some(receiver) = function.child_by_field_name("value") {
            collect_method_handlers(receiver, source, handlers);
        }
    }

    let Some(method) = callable_name(function, source) else {
        return;
    };
    if !is_http_method(&method) {
        return;
    }

    let Some(arguments) = node.child_by_field_name("arguments") else {
        return;
    };
    let Some(handler) = arguments
        .named_child(0)
        .and_then(|argument| handler_name(argument, source))
    else {
        return;
    };
    handlers.push((method.to_uppercase(), handler));
}

fn callable_name(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" | "field_identifier" => node_text(node, source).map(str::to_owned),
        "scoped_identifier" => node_text(node, source)
            .and_then(|name| name.rsplit("::").next())
            .map(str::to_owned),
        "field_expression" => node
            .child_by_field_name("field")
            .and_then(|field| callable_name(field, source)),
        "generic_function" => node
            .child_by_field_name("function")
            .and_then(|function| callable_name(function, source)),
        _ => None,
    }
}

fn handler_name(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" | "scoped_identifier" | "field_expression" | "generic_function" => {
            node_text(node, source)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
        }
        _ => None,
    }
}

fn is_http_method(name: &str) -> bool {
    matches!(
        name,
        "get" | "post" | "put" | "patch" | "delete" | "head" | "options"
    )
}

fn push_route(
    routes: &mut Vec<RouteFact>,
    seen: &mut BTreeSet<(String, String, String)>,
    file: &str,
    line: usize,
    method: String,
    path: String,
    handler: String,
) {
    if !seen.insert((method.clone(), path.clone(), handler.clone())) {
        return;
    }

    routes.push(RouteFact {
        id: String::new(),
        file: file.to_owned(),
        language: "rust".to_owned(),
        method,
        path,
        handler,
        handler_file: None,
        line,
        framework: "axum".to_owned(),
        middleware: Vec::new(),
    });
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    node.utf8_text(source.as_bytes()).ok()
}

fn string_literal_value(literal: &str) -> Option<String> {
    let literal = literal.trim();
    if let Some(raw) = literal.strip_prefix('r') {
        let hash_count = raw.bytes().take_while(|byte| *byte == b'#').count();
        let quoted = raw.get(hash_count..)?;
        if !quoted.starts_with('"') {
            return None;
        }
        let suffix = format!("\"{}", "#".repeat(hash_count));
        let content = quoted.strip_prefix('"')?.strip_suffix(suffix.as_str())?;
        return Some(content.to_owned());
    }

    let content = literal.strip_prefix('"')?.strip_suffix('"')?;
    unescape_rust_string(content)
}

fn unescape_rust_string(content: &str) -> Option<String> {
    let mut value = String::with_capacity(content.len());
    let mut chars = content.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            value.push(character);
            continue;
        }

        match chars.next()? {
            '\\' => value.push('\\'),
            '"' => value.push('"'),
            '\'' => value.push('\''),
            'n' => value.push('\n'),
            'r' => value.push('\r'),
            't' => value.push('\t'),
            '0' => value.push('\0'),
            '\n' => {
                while matches!(chars.clone().next(), Some(' ' | '\t' | '\r' | '\n')) {
                    chars.next();
                }
            }
            'x' => {
                let high = chars.next()?.to_digit(16)?;
                let low = chars.next()?.to_digit(16)?;
                value.push(char::from_u32((high << 4) | low)?);
            }
            'u' => {
                if chars.next()? != '{' {
                    return None;
                }
                let mut digits = String::new();
                loop {
                    let digit = chars.next()?;
                    if digit == '}' {
                        break;
                    }
                    if digit != '_' && !digit.is_ascii_hexdigit() {
                        return None;
                    }
                    digits.push(digit);
                }
                if digits.is_empty() {
                    return None;
                }
                value.push(char::from_u32(
                    u32::from_str_radix(&digits.replace('_', ""), 16).ok()?,
                )?);
            }
            _ => return None,
        }
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_chained_axum_routes() {
        let source = r#"use axum::{routing::get, Router};

fn routes() {
    Router::new().route(
        "/users",
        get(list_users)
            .post(create_user)
            .put(replace_user)
            .patch(update_user)
            .delete(delete_user),
    );
}"#;

        let routes = resolver().extract_routes("src/routes.rs", source);
        let values: Vec<_> = routes
            .iter()
            .map(|route| {
                (
                    route.id.as_str(),
                    route.file.as_str(),
                    route.language.as_str(),
                    route.method.as_str(),
                    route.path.as_str(),
                    route.handler.as_str(),
                    route.handler_file.as_deref(),
                    route.line,
                    route.framework.as_str(),
                    route.middleware.is_empty(),
                )
            })
            .collect();

        assert_eq!(
            values,
            vec![
                (
                    "",
                    "src/routes.rs",
                    "rust",
                    "GET",
                    "/users",
                    "list_users",
                    None,
                    4,
                    "axum",
                    true,
                ),
                (
                    "",
                    "src/routes.rs",
                    "rust",
                    "POST",
                    "/users",
                    "create_user",
                    None,
                    4,
                    "axum",
                    true,
                ),
                (
                    "",
                    "src/routes.rs",
                    "rust",
                    "PUT",
                    "/users",
                    "replace_user",
                    None,
                    4,
                    "axum",
                    true,
                ),
                (
                    "",
                    "src/routes.rs",
                    "rust",
                    "PATCH",
                    "/users",
                    "update_user",
                    None,
                    4,
                    "axum",
                    true,
                ),
                (
                    "",
                    "src/routes.rs",
                    "rust",
                    "DELETE",
                    "/users",
                    "delete_user",
                    None,
                    4,
                    "axum",
                    true,
                ),
            ]
        );
    }

    #[test]
    fn requires_axum_source_evidence() {
        assert!(resolver()
            .extract_routes(
                "src/routes.rs",
                r#"fn routes() { Router::new().route("/users", get(list_users)); }"#,
            )
            .is_empty());
    }

    #[test]
    fn ignores_incomplete_axum_syntax() {
        assert!(resolver()
            .extract_routes(
                "src/routes.rs",
                "use axum::Router;\nRouter::new().route(\"/users\", get(",
            )
            .is_empty());
    }
}
