use std::collections::HashSet;

use tree_sitter::{Node, Parser};

use super::{FrameworkDescriptor, FrameworkResolver};
use crate::facts::RouteFact;

#[derive(Debug, Default, Clone, Copy)]
pub struct DjangoResolver;

impl FrameworkResolver for DjangoResolver {
    fn descriptor(&self) -> FrameworkDescriptor {
        FrameworkDescriptor {
            name: "django",
            language: "python",
        }
    }

    fn extract_routes(&self, file: &str, source: &str) -> Vec<RouteFact> {
        let Some(tree) = parse_python(source) else {
            return Vec::new();
        };
        if !imports_framework(tree.root_node(), source, "django") {
            return Vec::new();
        }

        let mut calls = Vec::new();
        collect_calls(tree.root_node(), &mut calls);

        let mut routes = Vec::new();
        let mut seen = HashSet::new();
        for call in calls {
            let Some((callee, arguments)) = node_text(source, call).and_then(call_parts) else {
                continue;
            };
            if !matches!(callee.rsplit('.').next(), Some("path" | "re_path")) {
                continue;
            }
            let Some(path) = literal_argument(&arguments, &["route"]) else {
                continue;
            };
            let Some(handler) = handler_argument(&arguments) else {
                continue;
            };
            if !seen.insert((path.clone(), handler.clone())) {
                continue;
            }

            routes.push(RouteFact {
                id: String::new(),
                file: file.to_string(),
                language: "python".to_string(),
                method: "*".to_string(),
                path,
                handler,
                handler_file: None,
                line: call.start_position().row + 1,
                framework: "django".to_string(),
                middleware: Vec::new(),
            });
        }

        routes
    }
}

pub fn resolver() -> impl FrameworkResolver {
    DjangoResolver
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

fn collect_calls<'tree>(node: Node<'tree>, output: &mut Vec<Node<'tree>>) {
    if node.kind() == "call" {
        output.push(node);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_calls(child, output);
    }
}

fn node_text<'source>(source: &'source str, node: Node<'_>) -> Option<&'source str> {
    source.get(node.byte_range())
}

fn call_parts(call: &str) -> Option<(&str, Vec<&str>)> {
    let open = call.find('(')?;
    let callee = call[..open].trim();
    if callee.is_empty() {
        return None;
    }
    let arguments = delimited_content(&call[open..])?;
    Some((callee, split_top_level(arguments)))
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

fn handler_argument(arguments: &[&str]) -> Option<String> {
    let handler = named_argument(arguments, "view").or_else(|| {
        arguments
            .get(1)
            .copied()
            .filter(|argument| !is_keyword_argument(argument))
    })?;
    let handler = handler.trim();
    if handler.is_empty()
        || handler
            .find('(')
            .and_then(|open| handler[..open].trim().rsplit('.').next())
            == Some("include")
    {
        return None;
    }
    Some(handler.to_string())
}

fn named_argument<'a>(arguments: &[&'a str], name: &str) -> Option<&'a str> {
    arguments.iter().find_map(|argument| {
        let remainder = argument.trim().strip_prefix(name)?.trim_start();
        remainder.strip_prefix('=').map(str::trim)
    })
}

fn is_keyword_argument(argument: &str) -> bool {
    let argument = argument.trim_start();
    let bytes = argument.as_bytes();
    let Some(first) = bytes.first() else {
        return false;
    };
    if !(*first == b'_' || first.is_ascii_alphabetic()) {
        return false;
    }

    let mut index = 1usize;
    while index < bytes.len() && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric()) {
        index += 1;
    }
    bytes[index..].trim_ascii_start().starts_with(b"=")
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
    use super::{DjangoResolver, FrameworkResolver};

    #[test]
    fn extracts_django_path_and_re_path_registrations() {
        let source = r#"
from django.urls import path, re_path
urlpatterns = [
    path("health/", health),
    re_path(r"^reports/(?P<year>[0-9]{4})/$", views.report, name="report"),
    path(route="about/", view=about),
    path("health/", health),
]
"#;

        let routes = DjangoResolver.extract_routes("urls.py", source);

        assert_eq!(routes.len(), 3);
        assert!(routes.iter().any(|route| {
            route.method == "*"
                && route.path == "health/"
                && route.handler == "health"
                && route.file == "urls.py"
                && route.framework == "django"
        }));
        assert!(routes.iter().any(|route| {
            route.path == "^reports/(?P<year>[0-9]{4})/$" && route.handler == "views.report"
        }));
        assert!(routes
            .iter()
            .any(|route| route.path == "about/" && route.handler == "about"));
    }

    #[test]
    fn requires_django_import_and_ignores_malformed_registrations() {
        let malformed = DjangoResolver.extract_routes(
            "urls.py",
            "from django.urls import path\nurlpatterns = [path(\"broken\",",
        );
        let foreign = DjangoResolver.extract_routes(
            "urls.py",
            "from flask import Flask\nurlpatterns = [path(\"health/\", health)]",
        );

        assert!(malformed.is_empty());
        assert!(foreign.is_empty());
    }
}
