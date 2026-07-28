use std::collections::HashSet;

use tree_sitter::{Node, Parser};

use super::{FrameworkDescriptor, FrameworkResolver};
use crate::facts::RouteFact;

#[derive(Debug, Default, Clone, Copy)]
pub struct FastApiResolver;

impl FrameworkResolver for FastApiResolver {
    fn descriptor(&self) -> FrameworkDescriptor {
        FrameworkDescriptor {
            name: "fastapi",
            language: "python",
        }
    }

    fn extract_routes(&self, file: &str, source: &str) -> Vec<RouteFact> {
        let Some(tree) = parse_python(source) else {
            return Vec::new();
        };
        if !imports_framework(tree.root_node(), source, "fastapi") {
            return Vec::new();
        }

        let mut definitions = Vec::new();
        collect_decorated_definitions(tree.root_node(), &mut definitions);

        let mut routes = Vec::new();
        let mut seen = HashSet::new();
        for definition in definitions {
            let Some(function) = decorated_function(definition) else {
                continue;
            };
            let Some(handler) = function
                .child_by_field_name("name")
                .and_then(|name| node_text(source, name))
            else {
                continue;
            };

            let mut cursor = definition.walk();
            for decorator in definition.named_children(&mut cursor) {
                if decorator.kind() != "decorator" {
                    continue;
                }
                let Some((callee, arguments)) =
                    node_text(source, decorator).and_then(decorator_call)
                else {
                    continue;
                };
                let Some(path) = literal_argument(&arguments, &["path"]) else {
                    continue;
                };

                for method in fastapi_methods(callee, &arguments) {
                    let method = method.to_ascii_uppercase();
                    if method.is_empty()
                        || !seen.insert((method.clone(), path.clone(), handler.to_string()))
                    {
                        continue;
                    }

                    routes.push(RouteFact {
                        id: String::new(),
                        file: file.to_string(),
                        language: "python".to_string(),
                        method,
                        path: path.clone(),
                        handler: handler.to_string(),
                        handler_file: Some(file.to_string()),
                        line: decorator.start_position().row + 1,
                        framework: "fastapi".to_string(),
                        middleware: Vec::new(),
                    });
                }
            }
        }

        routes
    }
}

pub fn resolver() -> impl FrameworkResolver {
    FastApiResolver
}

fn parse_python(source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    let language: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    parser.set_language(&language).ok()?;
    parser.parse(source, None)
}

fn imports_framework(node: Node<'_>, source: &str, framework: &str) -> bool {
    if matches!(node.kind(), "import_statement" | "import_from_statement")
        && node_text(source, node)
            .is_some_and(|statement| import_mentions_framework(statement, framework))
    {
        return true;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if imports_framework(child, source, framework) {
            return true;
        }
    }
    false
}

fn import_mentions_framework(statement: &str, framework: &str) -> bool {
    let statement = statement.trim();
    if let Some(rest) = statement.strip_prefix("from ") {
        return rest
            .split_ascii_whitespace()
            .next()
            .is_some_and(|module| is_framework_module(module, framework));
    }
    if let Some(rest) = statement.strip_prefix("import ") {
        return rest.split(',').any(|part| {
            part.trim()
                .split_ascii_whitespace()
                .next()
                .is_some_and(|module| is_framework_module(module, framework))
        });
    }
    false
}

fn is_framework_module(module: &str, framework: &str) -> bool {
    module == framework
        || module
            .strip_prefix(framework)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn collect_decorated_definitions<'tree>(node: Node<'tree>, output: &mut Vec<Node<'tree>>) {
    if node.kind() == "decorated_definition" {
        output.push(node);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_decorated_definitions(child, output);
    }
}

fn decorated_function(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    let function = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "function_definition");
    function
}

fn node_text<'source>(source: &'source str, node: Node<'_>) -> Option<&'source str> {
    source.get(node.byte_range())
}

fn decorator_call(decorator: &str) -> Option<(&str, Vec<&str>)> {
    let decorator = decorator.trim().strip_prefix('@')?.trim();
    let open = decorator.find('(')?;
    let callee = decorator[..open].trim();
    if callee.is_empty() {
        return None;
    }
    let arguments = delimited_content(&decorator[open..])?;
    Some((callee, split_top_level(arguments)))
}

fn fastapi_methods(callee: &str, arguments: &[&str]) -> Vec<String> {
    let Some((_, method)) = callee.rsplit_once('.') else {
        return Vec::new();
    };

    match method.to_ascii_lowercase().as_str() {
        "api_route" => named_argument(arguments, "methods")
            .map(literal_methods)
            .unwrap_or_default(),
        "get" | "post" | "put" | "delete" | "patch" | "head" | "options" | "trace" => {
            vec![method.to_string()]
        }
        _ => Vec::new(),
    }
}

fn literal_argument(arguments: &[&str], names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(value) = named_argument(arguments, name) {
            return literal_string(value);
        }
    }
    arguments
        .first()
        .and_then(|argument| literal_string(argument))
}

fn named_argument<'a>(arguments: &[&'a str], name: &str) -> Option<&'a str> {
    arguments.iter().find_map(|argument| {
        let remainder = argument.trim().strip_prefix(name)?.trim_start();
        remainder.strip_prefix('=').map(str::trim)
    })
}

fn literal_methods(value: &str) -> Vec<String> {
    if let Some(method) = literal_string(value) {
        return vec![method];
    }

    let Some(values) = delimited_content(value) else {
        return Vec::new();
    };
    split_top_level(values)
        .into_iter()
        .filter_map(literal_string)
        .collect()
}

fn delimited_content(text: &str) -> Option<&str> {
    let text = text.trim();
    let bytes = text.as_bytes();
    let (&opening, _) = bytes.split_first()?;
    if !matches!(opening, b'(' | b'[' | b'{') {
        return None;
    }
    let closing = match opening {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        _ => return None,
    };

    let mut depth = 0usize;
    let mut quote = None;
    let mut triple = false;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(quote_byte) = quote {
            if byte == b'\\' {
                index = (index + 2).min(bytes.len());
                continue;
            }
            if triple
                && index + 2 < bytes.len()
                && bytes[index + 1] == quote_byte
                && bytes[index + 2] == quote_byte
            {
                quote = None;
                triple = false;
                index += 3;
                continue;
            }
            if !triple && byte == quote_byte {
                quote = None;
            }
            index += 1;
            continue;
        }

        match byte {
            b'\'' | b'"' => {
                quote = Some(byte);
                triple =
                    index + 2 < bytes.len() && bytes[index + 1] == byte && bytes[index + 2] == byte;
                index += if triple { 3 } else { 1 };
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                index += 1;
            }
            b')' | b']' | b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    if byte != closing || !text[index + 1..].trim().is_empty() {
                        return None;
                    }
                    return text.get(1..index);
                }
                index += 1;
            }
            _ => index += 1,
        }
    }

    None
}

fn split_top_level(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut arguments = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut quote = None;
    let mut triple = false;
    let mut index = 0usize;

    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(quote_byte) = quote {
            if byte == b'\\' {
                index = (index + 2).min(bytes.len());
                continue;
            }
            if triple
                && index + 2 < bytes.len()
                && bytes[index + 1] == quote_byte
                && bytes[index + 2] == quote_byte
            {
                quote = None;
                triple = false;
                index += 3;
                continue;
            }
            if !triple && byte == quote_byte {
                quote = None;
            }
            index += 1;
            continue;
        }

        match byte {
            b'\'' | b'"' => {
                quote = Some(byte);
                triple =
                    index + 2 < bytes.len() && bytes[index + 1] == byte && bytes[index + 2] == byte;
                index += if triple { 3 } else { 1 };
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                index += 1;
            }
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            b',' if depth == 0 => {
                arguments.push(&text[start..index]);
                start = index + 1;
                index += 1;
            }
            _ => index += 1,
        }
    }

    arguments.push(&text[start..]);
    arguments
}

fn literal_string(value: &str) -> Option<String> {
    let value = value.trim();
    let bytes = value.as_bytes();
    let mut start = 0usize;
    while start < bytes.len() && bytes[start].is_ascii_alphabetic() {
        start += 1;
    }
    let quote = *bytes.get(start)?;
    if !matches!(quote, b'\'' | b'"')
        || bytes[..start]
            .iter()
            .any(|prefix| !matches!(prefix, b'r' | b'R' | b'u' | b'U'))
    {
        return None;
    }

    let triple = start + 2 < bytes.len() && bytes[start + 1] == quote && bytes[start + 2] == quote;
    let content_start = start + if triple { 3 } else { 1 };
    let mut index = content_start;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
            continue;
        }
        if triple
            && index + 2 < bytes.len()
            && bytes[index] == quote
            && bytes[index + 1] == quote
            && bytes[index + 2] == quote
        {
            if value[index + 3..].trim().is_empty() {
                return value.get(content_start..index).map(str::to_string);
            }
            return None;
        }
        if !triple && bytes[index] == quote {
            if value[index + 1..].trim().is_empty() {
                return value.get(content_start..index).map(str::to_string);
            }
            return None;
        }
        index += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{FastApiResolver, FrameworkResolver};

    #[test]
    fn extracts_fastapi_decorators_and_deduplicates_routes() {
        let source = r#"
from fastapi import APIRouter, FastAPI
@app.get("/health")
@app.get("/health")
async def health():
    return {"ok": True}

@router.post("/items/{item_id}")
def create_item(item_id: int):
    return item_id

@router.api_route("/items/{item_id}", methods=["put", "PATCH"])
def update_item(item_id: int):
    return item_id
"#;

        let routes = FastApiResolver.extract_routes("api.py", source);

        assert_eq!(routes.len(), 4);
        assert!(routes.iter().any(|route| {
            route.method == "GET"
                && route.path == "/health"
                && route.handler == "health"
                && route.file == "api.py"
                && route.framework == "fastapi"
        }));
        assert!(routes.iter().any(|route| {
            route.method == "POST"
                && route.path == "/items/{item_id}"
                && route.handler == "create_item"
        }));
        assert!(routes.iter().any(|route| {
            route.method == "PUT"
                && route.path == "/items/{item_id}"
                && route.handler == "update_item"
        }));
        assert!(routes.iter().any(|route| {
            route.method == "PATCH"
                && route.path == "/items/{item_id}"
                && route.handler == "update_item"
        }));
    }

    #[test]
    fn requires_fastapi_import_and_ignores_malformed_decorators() {
        let malformed =
            FastApiResolver.extract_routes("api.py", "from fastapi import FastAPI\n@app.get(\n");
        let foreign = FastApiResolver.extract_routes(
            "api.py",
            "from flask import Flask\n@app.get(\"/health\")\ndef health():\n    pass\n",
        );

        assert!(malformed.is_empty());
        assert!(foreign.is_empty());
    }
}
