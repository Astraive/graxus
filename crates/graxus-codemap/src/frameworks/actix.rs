use std::collections::BTreeSet;

use tree_sitter::Node;

use crate::facts::RouteFact;

use super::{FrameworkDescriptor, FrameworkResolver};

#[derive(Debug, Default, Clone, Copy)]
pub struct ActixResolver;

impl FrameworkResolver for ActixResolver {
    fn descriptor(&self) -> FrameworkDescriptor {
        FrameworkDescriptor {
            name: "actix",
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
        if !has_actix_evidence(tree.root_node(), source) {
            return Vec::new();
        }

        let mut routes = Vec::new();
        let mut seen = BTreeSet::new();
        collect_functions(tree.root_node(), source, file, &mut routes, &mut seen);
        routes
    }
}

pub fn resolver() -> impl FrameworkResolver {
    ActixResolver
}

fn has_actix_evidence(node: Node<'_>, source: &str) -> bool {
    match node.kind() {
        "use_declaration" | "extern_crate_declaration"
            if imports_crate(node_text(node, source).unwrap_or_default(), "actix_web") =>
        {
            return true;
        }
        "attribute_item"
            if node_text(node, source)
                .is_some_and(|attribute| attribute.trim_start().starts_with("#[actix_web::")) =>
        {
            return true;
        }
        "scoped_identifier"
            if node_text(node, source).is_some_and(|path| path.starts_with("actix_web::")) =>
        {
            return true;
        }
        _ => {}
    }

    (0..node.named_child_count()).any(|index| {
        node.named_child(index)
            .is_some_and(|child| has_actix_evidence(child, source))
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

fn collect_functions(
    node: Node<'_>,
    source: &str,
    file: &str,
    routes: &mut Vec<RouteFact>,
    seen: &mut BTreeSet<(String, String, String)>,
) {
    let mut attributes = Vec::new();
    for index in 0..node.named_child_count() {
        let Some(child) = node.named_child(index) else {
            continue;
        };
        match child.kind() {
            "attribute_item" => attributes.push(child),
            "function_item" => {
                extract_function_routes(child, &attributes, source, file, routes, seen);
                attributes.clear();
            }
            _ => {
                collect_functions(child, source, file, routes, seen);
                attributes.clear();
            }
        }
    }
}

fn extract_function_routes(
    function: Node<'_>,
    attributes: &[Node<'_>],
    source: &str,
    file: &str,
    routes: &mut Vec<RouteFact>,
    seen: &mut BTreeSet<(String, String, String)>,
) {
    let Some(handler) = function
        .child_by_field_name("name")
        .and_then(|name| node_text(name, source))
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return;
    };

    for &attribute in attributes {
        extract_attribute_route(attribute, handler, source, file, routes, seen);
    }
    for index in 0..function.named_child_count() {
        let Some(attribute) = function.named_child(index) else {
            continue;
        };
        if attribute.kind() == "attribute_item" {
            extract_attribute_route(attribute, handler, source, file, routes, seen);
        }
    }
}

fn extract_attribute_route(
    attribute: Node<'_>,
    handler: &str,
    source: &str,
    file: &str,
    routes: &mut Vec<RouteFact>,
    seen: &mut BTreeSet<(String, String, String)>,
) {
    let Some((method, path)) = node_text(attribute, source).and_then(attribute_route) else {
        return;
    };
    push_route(
        routes,
        seen,
        file,
        attribute.start_position().row + 1,
        method,
        path,
        handler.to_owned(),
    );
}

fn attribute_route(attribute: &str) -> Option<(String, String)> {
    let attribute = attribute.trim();
    let inner = attribute.strip_prefix("#[")?.trim_start();
    let name_end = inner.find(|character: char| {
        character == '(' || character == ']' || character.is_ascii_whitespace()
    })?;
    let name = inner.get(..name_end)?.rsplit("::").next()?;
    if !is_http_method(name) {
        return None;
    }

    let arguments = inner.get(name_end..)?.trim_start().strip_prefix('(')?;
    let path = leading_string_literal(arguments)?;
    Some((name.to_uppercase(), path))
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
        framework: "actix".to_owned(),
        middleware: Vec::new(),
    });
}

fn leading_string_literal(input: &str) -> Option<String> {
    let input = input.trim_start();
    if input.starts_with('"') {
        let mut escaped = false;
        for (index, character) in input.char_indices().skip(1) {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                return string_literal_value(input.get(..=index)?);
            }
        }
        return None;
    }

    let raw = input.strip_prefix('r')?;
    let hash_count = raw.bytes().take_while(|byte| *byte == b'#').count();
    let quoted = raw.get(hash_count..)?;
    if !quoted.starts_with('"') {
        return None;
    }
    let suffix = format!("\"{}", "#".repeat(hash_count));
    let end = quoted.get(1..)?.find(suffix.as_str())? + 1 + suffix.len();
    string_literal_value(input.get(..(1 + hash_count + end))?)
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

fn node_text<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    node.utf8_text(source.as_bytes()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_actix_attribute_routes() {
        let source = r#"use actix_web::{delete, get, patch, post, put};

#[get("/users")]
async fn list_users() {}

#[actix_web::post("/users")]
async fn create_user() {}

#[put("/users/{id}")]
async fn replace_user() {}

#[patch("/users/{id}")]
async fn update_user() {}

#[delete("/users/{id}")]
async fn delete_user() {}"#;

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
                    3,
                    "actix",
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
                    6,
                    "actix",
                    true,
                ),
                (
                    "",
                    "src/routes.rs",
                    "rust",
                    "PUT",
                    "/users/{id}",
                    "replace_user",
                    None,
                    9,
                    "actix",
                    true,
                ),
                (
                    "",
                    "src/routes.rs",
                    "rust",
                    "PATCH",
                    "/users/{id}",
                    "update_user",
                    None,
                    12,
                    "actix",
                    true,
                ),
                (
                    "",
                    "src/routes.rs",
                    "rust",
                    "DELETE",
                    "/users/{id}",
                    "delete_user",
                    None,
                    15,
                    "actix",
                    true,
                ),
            ]
        );
    }

    #[test]
    fn requires_actix_source_evidence() {
        assert!(resolver()
            .extract_routes("src/routes.rs", "#[get(\"/users\")]\nfn list_users() {}")
            .is_empty());
    }

    #[test]
    fn ignores_incomplete_actix_syntax() {
        assert!(resolver()
            .extract_routes("src/routes.rs", "use actix_web::get;\n#[get(\"/users\")]",)
            .is_empty());
    }
}
