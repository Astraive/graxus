use std::collections::HashSet;

use tree_sitter::{Node, Parser};

use crate::facts::RouteFact;

use super::{FrameworkDescriptor, FrameworkResolver};

#[derive(Debug, Default, Clone, Copy)]
pub struct FiberResolver;

impl FrameworkResolver for FiberResolver {
    fn descriptor(&self) -> FrameworkDescriptor {
        FrameworkDescriptor {
            name: "fiber",
            language: "go",
        }
    }

    fn extract_routes(&self, file: &str, source: &str) -> Vec<RouteFact> {
        extract_go_routes(file, source, self.descriptor().name)
    }
}

pub fn resolver() -> impl FrameworkResolver {
    FiberResolver
}

fn extract_go_routes(file: &str, source: &str, framework: &str) -> Vec<RouteFact> {
    let mut parser = Parser::new();
    let language: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
    if parser.set_language(&language).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut package_names = HashSet::new();
    collect_import_evidence(tree.root_node(), source, &mut package_names);
    if package_names.is_empty() {
        return Vec::new();
    }

    let mut framework_receivers = HashSet::new();
    collect_framework_receivers(
        tree.root_node(),
        source,
        &package_names,
        &mut framework_receivers,
    );

    let mut routes = Vec::new();
    let mut seen = HashSet::new();
    collect_routes(
        tree.root_node(),
        source,
        file,
        framework,
        &framework_receivers,
        &mut routes,
        &mut seen,
    );
    routes
}

fn collect_import_evidence(node: Node<'_>, source: &str, package_names: &mut HashSet<String>) {
    if node.kind() == "import_spec" {
        if let Some(path_node) = node.child_by_field_name("path") {
            if let Some(path) = go_string_literal(path_node, source) {
                if is_fiber_import(&path) {
                    if let Some(package_name) =
                        import_package_name(node, path_node, source, "fiber")
                    {
                        package_names.insert(package_name);
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_import_evidence(child, source, package_names);
    }
}

fn collect_framework_receivers(
    node: Node<'_>,
    source: &str,
    package_names: &HashSet<String>,
    receivers: &mut HashSet<String>,
) {
    match node.kind() {
        "short_var_declaration" | "assignment_statement" => {
            if let (Some(left), Some(right)) = (
                node.child_by_field_name("left"),
                node.child_by_field_name("right"),
            ) {
                if is_framework_constructor(right, source, package_names) {
                    if let Some(name) = first_identifier(left, source) {
                        receivers.insert(name);
                    }
                }
            }
        }
        "var_spec" => {
            if contains_framework_constructor(node, source, package_names)
                || contains_framework_type(node, source, package_names)
            {
                if let Some(name) = first_identifier(node, source) {
                    receivers.insert(name);
                }
            }
        }
        "parameter_declaration" => {
            if contains_framework_type(node, source, package_names) {
                if let Some(name) = first_identifier(node, source) {
                    receivers.insert(name);
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_framework_receivers(child, source, package_names, receivers);
    }
}

fn collect_routes(
    node: Node<'_>,
    source: &str,
    file: &str,
    framework: &str,
    framework_receivers: &HashSet<String>,
    routes: &mut Vec<RouteFact>,
    seen: &mut HashSet<(String, String, String)>,
) {
    if node.kind() == "call_expression" {
        if let Some((receiver, method, path, handler)) = route_parts(node, source) {
            if framework_receivers.contains(&receiver) {
                let key = (method.clone(), path.clone(), handler.clone());
                if seen.insert(key) {
                    routes.push(RouteFact {
                        id: String::new(),
                        file: file.to_string(),
                        language: "go".to_string(),
                        method,
                        path,
                        handler,
                        handler_file: None,
                        line: node.start_position().row + 1,
                        framework: framework.to_string(),
                        middleware: Vec::new(),
                    });
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_routes(
            child,
            source,
            file,
            framework,
            framework_receivers,
            routes,
            seen,
        );
    }
}

fn route_parts(node: Node<'_>, source: &str) -> Option<(String, String, String, String)> {
    let function = node.child_by_field_name("function")?;
    if function.kind() != "selector_expression" {
        return None;
    }
    let receiver = function.child_by_field_name("operand")?;
    let selector = function.child_by_field_name("field")?;
    if receiver.kind() != "identifier" || selector.kind() != "field_identifier" {
        return None;
    }

    let method = normalized_method(node_text(selector, source)?)?;
    let arguments = node.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    let mut arguments = arguments.named_children(&mut cursor);
    let path = arguments.next()?;
    let handler = arguments.last()?;
    if handler.kind() != "identifier" {
        return None;
    }

    Some((
        node_text(receiver, source)?.to_string(),
        method,
        go_string_literal(path, source)?,
        node_text(handler, source)?.to_string(),
    ))
}

fn normalized_method(method: &str) -> Option<String> {
    let method = method.to_ascii_uppercase();
    matches!(
        method.as_str(),
        "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS"
    )
    .then_some(method)
}

fn is_framework_constructor(node: Node<'_>, source: &str, package_names: &HashSet<String>) -> bool {
    let expression = if node.kind() == "expression_list" {
        node.named_child(0)
    } else {
        Some(node)
    };
    let Some(expression) = expression else {
        return false;
    };
    if expression.kind() != "call_expression" {
        return false;
    }

    let Some(function) = expression.child_by_field_name("function") else {
        return false;
    };
    if function.kind() != "selector_expression" {
        return false;
    }
    let (Some(package), Some(method)) = (
        function.child_by_field_name("operand"),
        function.child_by_field_name("field"),
    ) else {
        return false;
    };
    package.kind() == "identifier"
        && method.kind() == "field_identifier"
        && node_text(package, source).is_some_and(|name| package_names.contains(name))
        && node_text(method, source).is_some_and(|name| name == "New")
}

fn contains_framework_constructor(
    node: Node<'_>,
    source: &str,
    package_names: &HashSet<String>,
) -> bool {
    let mut cursor = node.walk();
    let has_constructor = node
        .named_children(&mut cursor)
        .any(|child| is_framework_constructor(child, source, package_names));
    has_constructor
}

fn contains_framework_type(node: Node<'_>, source: &str, package_names: &HashSet<String>) -> bool {
    let Some(text) = node_text(node, source) else {
        return false;
    };
    package_names.iter().any(|package_name| {
        text.contains(&format!("{package_name}.App"))
            || text.contains(&format!("{package_name}.Router"))
    })
}

fn first_identifier(node: Node<'_>, source: &str) -> Option<String> {
    let identifier = if node.kind() == "identifier" {
        node
    } else {
        let mut cursor = node.walk();
        let first_named = node
            .named_children(&mut cursor)
            .find(|child| child.kind() == "identifier");
        first_named?
    };
    node_text(identifier, source)
        .filter(|name| *name != "_")
        .map(ToString::to_string)
}

fn import_package_name(
    import_spec: Node<'_>,
    path: Node<'_>,
    source: &str,
    default_name: &str,
) -> Option<String> {
    let prefix = source
        .get(import_spec.start_byte()..path.start_byte())?
        .trim();
    match prefix {
        "" => Some(default_name.to_string()),
        "_" | "." => None,
        alias if is_go_identifier(alias) => Some(alias.to_string()),
        _ => None,
    }
}

fn is_go_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn is_fiber_import(path: &str) -> bool {
    matches!(
        path,
        "github.com/gofiber/fiber" | "github.com/gofiber/fiber/v2"
    )
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    source.get(node.byte_range())
}

fn go_string_literal(node: Node<'_>, source: &str) -> Option<String> {
    if node.kind() != "interpreted_string_literal" {
        return None;
    }
    let literal = node_text(node, source)?;
    let contents = literal.strip_prefix('"')?.strip_suffix('"')?;
    decode_go_string(contents)
}

fn decode_go_string(contents: &str) -> Option<String> {
    let mut result = String::with_capacity(contents.len());
    let mut characters = contents.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            result.push(character);
            continue;
        }

        let escaped = characters.next()?;
        let value = match escaped {
            'a' => '\u{7}',
            'b' => '\u{8}',
            'f' => '\u{C}',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'v' => '\u{B}',
            '\\' => '\\',
            '"' => '"',
            '\'' => '\'',
            'x' => char::from_u32(take_digits(&mut characters, 2, 16)?)?,
            'u' => char::from_u32(take_digits(&mut characters, 4, 16)?)?,
            'U' => char::from_u32(take_digits(&mut characters, 8, 16)?)?,
            '0'..='7' => {
                let first = escaped.to_digit(8)?;
                let rest = take_digits(&mut characters, 2, 8)?;
                char::from_u32((first << 6) | rest)?
            }
            _ => return None,
        };
        result.push(value);
    }
    Some(result)
}

fn take_digits(characters: &mut std::str::Chars<'_>, count: usize, radix: u32) -> Option<u32> {
    let mut value: u32 = 0;
    for _ in 0..count {
        value = value
            .checked_mul(radix)?
            .checked_add(characters.next()?.to_digit(radix)?)?;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_fiber_routes_without_claiming_gin_routes() {
        let source = r#"
package routes

import (
    "github.com/gofiber/fiber/v2"
    "github.com/gin-gonic/gin"
)

func register() {
    app := fiber.New()
    r := gin.Default()
    app.Put("/users/:id", updateUser)
    app.gEt("/health", healthCheck)
    app.Put("/users/:id", updateUser)
    r.GET("/users", listUsers)
    app.POST("/unfinished",
}
"#;

        let routes = FiberResolver.extract_routes("routes.go", source);

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
                ("PUT", "/users/:id", "updateUser"),
                ("GET", "/health", "healthCheck"),
            ]
        );
        assert!(routes.iter().all(|route| {
            route.file == "routes.go"
                && route.language == "go"
                && route.framework == "fiber"
                && route.handler_file.is_none()
                && route.middleware.is_empty()
        }));
    }
}
