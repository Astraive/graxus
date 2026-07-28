use std::collections::HashSet;

use tree_sitter::{Node, Parser};

use crate::facts::RouteFact;

use super::{FrameworkDescriptor, FrameworkResolver};

#[derive(Debug, Default, Clone, Copy)]
pub struct AspNetResolver;

impl FrameworkResolver for AspNetResolver {
    fn descriptor(&self) -> FrameworkDescriptor {
        FrameworkDescriptor {
            name: "aspnet",
            language: "csharp",
        }
    }

    fn extract_routes(&self, file: &str, source: &str) -> Vec<RouteFact> {
        let mut parser = Parser::new();
        let language: tree_sitter::Language = tree_sitter_c_sharp::LANGUAGE.into();
        if parser.set_language(&language).is_err() {
            return Vec::new();
        }

        let Some(tree) = parser.parse(source, None) else {
            return Vec::new();
        };

        let mut routes = Vec::new();
        let mut seen = HashSet::new();
        let mut cursor = tree.walk();

        'traverse: loop {
            let node = cursor.node();
            match node.kind() {
                "invocation_expression" => {
                    if let Some(route) = minimal_api_route(node, source) {
                        push_route(&mut routes, &mut seen, file, route);
                    }
                }
                "attribute" => {
                    if let Some(route) = controller_attribute_route(node, source) {
                        push_route(&mut routes, &mut seen, file, route);
                    }
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                continue;
            }

            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    break 'traverse;
                }
            }
        }

        routes
    }
}

struct ExtractedRoute {
    method: &'static str,
    path: String,
    handler: String,
    line: usize,
}

fn minimal_api_route(node: Node<'_>, source: &str) -> Option<ExtractedRoute> {
    let function = node.child_by_field_name("function")?;
    if function.kind() != "member_access_expression" {
        return None;
    }

    let method = http_method(member_name(function, source)?)?;
    let arguments = node.child_by_field_name("arguments")?;
    let path_argument = arguments
        .named_child(0)
        .filter(|argument| argument.kind() == "argument")?;
    let handler_argument_node = arguments
        .named_child(1)
        .filter(|argument| argument.kind() == "argument")?;
    let path = string_argument(path_argument, source)?;

    // The first argument is the pattern and the second is the endpoint handler.
    // A lambda is a valid endpoint handler but has no stable method identifier.
    let handler = handler_argument(handler_argument_node, source).unwrap_or_default();

    Some(ExtractedRoute {
        method,
        path: normalize_path(&path),
        handler,
        line: node.start_position().row + 1,
    })
}

fn controller_attribute_route(node: Node<'_>, source: &str) -> Option<ExtractedRoute> {
    let (method, path) = http_attribute(node, source)?;
    let method_declaration = ancestor_of_kind(node, "method_declaration")?;
    let handler = method_declaration
        .child_by_field_name("name")
        .and_then(|name| source.get(name.byte_range()))
        .filter(|name| !name.is_empty())?
        .to_string();

    Some(ExtractedRoute {
        method,
        path,
        handler,
        line: node.start_position().row + 1,
    })
}

fn push_route(
    routes: &mut Vec<RouteFact>,
    seen: &mut HashSet<(String, String, String)>,
    file: &str,
    route: ExtractedRoute,
) {
    let key = (
        route.method.to_string(),
        route.path.clone(),
        route.handler.clone(),
    );
    if !seen.insert(key) {
        return;
    }

    routes.push(RouteFact {
        id: String::new(),
        file: file.to_string(),
        language: "csharp".to_string(),
        method: route.method.to_string(),
        path: route.path,
        handler: route.handler,
        handler_file: None,
        line: route.line,
        framework: "aspnet".to_string(),
        middleware: Vec::new(),
    });
}

fn http_method(name: &str) -> Option<&'static str> {
    match name {
        "MapGet" | "HttpGet" | "HttpGetAttribute" => Some("GET"),
        "MapPost" | "HttpPost" | "HttpPostAttribute" => Some("POST"),
        "MapPut" | "HttpPut" | "HttpPutAttribute" => Some("PUT"),
        "MapDelete" | "HttpDelete" | "HttpDeleteAttribute" => Some("DELETE"),
        "MapPatch" | "HttpPatch" | "HttpPatchAttribute" => Some("PATCH"),
        "MapHead" | "HttpHead" | "HttpHeadAttribute" => Some("HEAD"),
        _ => None,
    }
}

fn http_attribute(node: Node<'_>, source: &str) -> Option<(&'static str, String)> {
    let text = source.get(node.byte_range())?.trim();
    let name_end = text.find('(').unwrap_or(text.len());
    let name = text[..name_end]
        .trim()
        .rsplit('.')
        .next()
        .filter(|name| !name.is_empty())?;
    let method = http_method(name)?;

    let Some(open_paren) = text.find('(') else {
        return Some((method, "/".to_string()));
    };
    if !text.ends_with(')') {
        return None;
    }

    let arguments = text.get(open_paren + 1..text.len() - 1)?.trim();
    if arguments.is_empty() {
        return Some((method, "/".to_string()));
    }

    string_literal_prefix(arguments).map(|(path, _)| (method, normalize_path(&path)))
}

fn member_name<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    if let Some(name) = node.child_by_field_name("name") {
        if name.kind() == "generic_name" {
            let identifier = name
                .named_child(0)
                .filter(|identifier| identifier.kind() == "identifier")?;
            return source.get(identifier.byte_range());
        }
        return source
            .get(name.byte_range())
            .filter(|name| !name.is_empty());
    }

    source
        .get(node.byte_range())?
        .rsplit('.')
        .next()
        .and_then(|name| name.split('<').next())
        .filter(|name| !name.is_empty())
}

fn string_argument(node: Node<'_>, source: &str) -> Option<String> {
    let text = source.get(node.byte_range())?.trim();
    string_literal_prefix(text).and_then(|(value, rest)| rest.trim().is_empty().then_some(value))
}

fn handler_argument(node: Node<'_>, source: &str) -> Option<String> {
    let expression = node.named_child(0)?;
    match expression.kind() {
        "identifier" => source.get(expression.byte_range()).map(ToString::to_string),
        "member_access_expression" => member_name(expression, source).map(ToString::to_string),
        _ => None,
    }
}

fn ancestor_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == kind {
            return Some(parent);
        }
        current = parent.parent();
    }
    None
}

fn string_literal_prefix(text: &str) -> Option<(String, &str)> {
    let text = text.trim_start();

    if let Some(rest) = text.strip_prefix("@\"") {
        let mut value = String::new();
        let mut start = 0;

        while let Some(relative_quote) = rest.get(start..)?.find('"') {
            let quote = start + relative_quote;
            if rest.get(quote + 1..)?.starts_with('"') {
                value.push_str(rest.get(start..quote)?);
                value.push('"');
                start = quote + 2;
            } else {
                value.push_str(rest.get(start..quote)?);
                return Some((value, rest.get(quote + 1..)?));
            }
        }

        return None;
    }

    let rest = text.strip_prefix('"')?;
    let mut escaped = false;
    for (index, character) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '"' => {
                let value = unescape_csharp_string(rest.get(..index)?);
                return Some((value, rest.get(index + character.len_utf8()..)?));
            }
            _ => {}
        }
    }
    None
}

fn unescape_csharp_string(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut characters = value.chars();

    while let Some(character) = characters.next() {
        if character != '\\' {
            result.push(character);
            continue;
        }

        let Some(escaped) = characters.next() else {
            result.push('\\');
            break;
        };
        match escaped {
            '\'' => result.push('\''),
            '"' => result.push('"'),
            '\\' => result.push('\\'),
            '0' => result.push('\0'),
            'a' => result.push('\u{7}'),
            'b' => result.push('\u{8}'),
            'f' => result.push('\u{c}'),
            'n' => result.push('\n'),
            'r' => result.push('\r'),
            't' => result.push('\t'),
            'v' => result.push('\u{b}'),
            _ => {
                result.push('\\');
                result.push(escaped);
            }
        }
    }

    result
}

fn normalize_path(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        "/".to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

pub fn resolver() -> impl FrameworkResolver {
    AspNetResolver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_minimal_api_routes_without_duplicates() {
        let source = r#"
var app = builder.Build();
app.MapGet("/users", GetUsers);
app.MapPost("users", UserHandlers.Create);
app.MapPut("/users/{id}", UpdateUser);
app.MapDelete("/users/{id}", DeleteUser);
app.MapGet("/users", GetUsers);
"#;

        let routes = AspNetResolver.extract_routes("Program.cs", source);
        let extracted: Vec<_> = routes
            .iter()
            .map(|route| {
                (
                    route.method.as_str(),
                    route.path.as_str(),
                    route.framework.as_str(),
                    route.file.as_str(),
                    route.handler.as_str(),
                )
            })
            .collect();

        assert_eq!(
            extracted,
            vec![
                ("GET", "/users", "aspnet", "Program.cs", "GetUsers"),
                ("POST", "/users", "aspnet", "Program.cs", "Create"),
                ("PUT", "/users/{id}", "aspnet", "Program.cs", "UpdateUser"),
                (
                    "DELETE",
                    "/users/{id}",
                    "aspnet",
                    "Program.cs",
                    "DeleteUser",
                ),
            ]
        );
    }

    #[test]
    fn extracts_controller_attribute_routes() {
        let source = r#"
public class UsersController {
    [HttpGet("/users")]
    public IActionResult GetUsers() => Ok();

    [HttpPost]
    public IActionResult CreateUser() => Ok();
}
"#;

        let routes = AspNetResolver.extract_routes("UsersController.cs", source);
        let extracted: Vec<_> = routes
            .iter()
            .map(|route| {
                (
                    route.method.as_str(),
                    route.path.as_str(),
                    route.framework.as_str(),
                    route.file.as_str(),
                    route.handler.as_str(),
                )
            })
            .collect();

        assert_eq!(
            extracted,
            vec![
                ("GET", "/users", "aspnet", "UsersController.cs", "GetUsers",),
                ("POST", "/", "aspnet", "UsersController.cs", "CreateUser",),
            ]
        );
    }

    #[test]
    fn ignores_incomplete_source_without_panicking() {
        let source = r#"
app.MapGet("/users",
[HttpGet("/users")]
public IActionResult
"#;

        assert!(AspNetResolver
            .extract_routes("Broken.cs", source)
            .is_empty());
    }
}
