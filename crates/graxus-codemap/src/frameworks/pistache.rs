use std::collections::HashSet;

use crate::facts::RouteFact;

use super::{FrameworkDescriptor, FrameworkResolver};

#[derive(Debug, Default, Clone, Copy)]
pub struct PistacheResolver;

impl FrameworkResolver for PistacheResolver {
    fn descriptor(&self) -> FrameworkDescriptor {
        FrameworkDescriptor {
            name: "pistache",
            language: "cpp",
        }
    }

    fn extract_routes(&self, file: &str, source: &str) -> Vec<RouteFact> {
        let descriptor = self.descriptor();
        let mut routes = Vec::new();
        let mut seen = HashSet::new();

        for start in invocation_offsets(source, "Routes") {
            let method_start = start + "Routes".len();
            if source.as_bytes().get(method_start..method_start + 2) != Some(b"::") {
                continue;
            }
            let method_start = method_start + 2;
            let method_end = identifier_end(source, method_start);
            let Some(method) = http_method(&source[method_start..method_end]) else {
                continue;
            };

            let open = skip_whitespace(source, method_end);
            if source.as_bytes().get(open) != Some(&b'(') {
                continue;
            }
            let Some(close) = matching_delimiter(source, open, b'(', b')') else {
                continue;
            };

            let arguments = split_top_level(&source[open + 1..close]);
            let Some(path) = arguments.get(1).and_then(|argument| literal_path(argument)) else {
                continue;
            };
            let handler = arguments
                .get(2)
                .map_or_else(String::new, |argument| bound_handler(argument));

            push_route(
                &mut routes,
                &mut seen,
                descriptor,
                file,
                line_number(source, start),
                method.to_string(),
                path,
                handler,
            );
        }

        routes
    }
}

pub fn resolver() -> impl FrameworkResolver {
    PistacheResolver
}

fn push_route(
    routes: &mut Vec<RouteFact>,
    seen: &mut HashSet<String>,
    descriptor: FrameworkDescriptor,
    file: &str,
    line: usize,
    method: String,
    path: String,
    handler: String,
) {
    let key = format!("{method}\u{1f}{path}\u{1f}{handler}");
    if !seen.insert(key) {
        return;
    }

    routes.push(RouteFact {
        id: format!(
            "{}:{file}:{line}:{method}:{path}:{handler}",
            descriptor.name
        ),
        file: file.to_string(),
        language: descriptor.language.to_string(),
        method,
        path,
        handler,
        handler_file: None,
        line,
        framework: descriptor.name.to_string(),
        middleware: Vec::new(),
    });
}

fn bound_handler(argument: &str) -> String {
    let argument = argument.trim();
    let Some(bind_start) = argument.find("Routes::bind") else {
        return argument.to_string();
    };
    let open = skip_whitespace(argument, bind_start + "Routes::bind".len());
    if argument.as_bytes().get(open) != Some(&b'(') {
        return argument.to_string();
    }
    let Some(close) = matching_delimiter(argument, open, b'(', b')') else {
        return argument.to_string();
    };
    if !argument[close + 1..].trim().is_empty() {
        return argument.to_string();
    }

    split_top_level(&argument[open + 1..close])
        .first()
        .map_or_else(String::new, |handler| handler.trim().to_string())
}

fn invocation_offsets(source: &str, target: &str) -> Vec<usize> {
    let bytes = source.as_bytes();
    let target = target.as_bytes();
    let mut offsets = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if bytes.get(index..index + 2) == Some(b"//") {
            index = skip_line_comment(bytes, index + 2);
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"/*") {
            index = skip_block_comment(bytes, index + 2);
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"R\"") {
            index = skip_raw_string(source, index);
            continue;
        }
        if matches!(bytes.get(index), Some(b'"' | b'\'')) {
            index = skip_quoted_string(bytes, index);
            continue;
        }
        if bytes.get(index..index + target.len()) == Some(target)
            && !is_identifier_byte(bytes.get(index.wrapping_sub(1)).copied())
            && !is_identifier_byte(bytes.get(index + target.len()).copied())
            && !is_preprocessor_position(source, index)
        {
            offsets.push(index);
            index += target.len();
            continue;
        }

        index += 1;
    }

    offsets
}

fn matching_delimiter(source: &str, open: usize, opening: u8, closing: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(open) != Some(&opening) {
        return None;
    }

    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        if bytes.get(index..index + 2) == Some(b"//") {
            index = skip_line_comment(bytes, index + 2);
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"/*") {
            index = skip_block_comment(bytes, index + 2);
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"R\"") {
            index = skip_raw_string(source, index);
            continue;
        }
        if matches!(bytes.get(index), Some(b'"' | b'\'')) {
            index = skip_quoted_string(bytes, index);
            continue;
        }
        if bytes[index] == opening {
            depth += 1;
        } else if bytes[index] == closing {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
        index += 1;
    }

    None
}

fn split_top_level(source: &str) -> Vec<&str> {
    let bytes = source.as_bytes();
    let mut arguments = Vec::new();
    let mut start = 0;
    let mut parentheses = 0usize;
    let mut braces = 0usize;
    let mut brackets = 0usize;
    let mut angles = 0usize;
    let mut index = 0;

    while index < bytes.len() {
        if bytes.get(index..index + 2) == Some(b"//") {
            index = skip_line_comment(bytes, index + 2);
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"/*") {
            index = skip_block_comment(bytes, index + 2);
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"R\"") {
            index = skip_raw_string(source, index);
            continue;
        }
        if matches!(bytes.get(index), Some(b'"' | b'\'')) {
            index = skip_quoted_string(bytes, index);
            continue;
        }

        match bytes[index] {
            b'(' => parentheses += 1,
            b')' => parentheses = parentheses.saturating_sub(1),
            b'{' => braces += 1,
            b'}' => braces = braces.saturating_sub(1),
            b'[' => brackets += 1,
            b']' => brackets = brackets.saturating_sub(1),
            b'<' => angles += 1,
            b'>' => angles = angles.saturating_sub(1),
            b',' if parentheses == 0 && braces == 0 && brackets == 0 && angles == 0 => {
                arguments.push(source[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }

    arguments.push(source[start..].trim());
    arguments
}

fn http_method(word: &str) -> Option<&'static str> {
    if word.eq_ignore_ascii_case("get") {
        Some("GET")
    } else if word.eq_ignore_ascii_case("post") {
        Some("POST")
    } else if word.eq_ignore_ascii_case("put") {
        Some("PUT")
    } else if word.eq_ignore_ascii_case("delete") {
        Some("DELETE")
    } else if word.eq_ignore_ascii_case("patch") {
        Some("PATCH")
    } else if word.eq_ignore_ascii_case("head") {
        Some("HEAD")
    } else if word.eq_ignore_ascii_case("options") {
        Some("OPTIONS")
    } else {
        None
    }
}

fn literal_path(argument: &str) -> Option<String> {
    let (path, remainder) = parse_cpp_string(argument.trim())?;
    remainder.trim().is_empty().then_some(path)
}

fn parse_cpp_string(value: &str) -> Option<(String, &str)> {
    let mut value = value.trim_start();
    for prefix in ["u8", "u", "U", "L"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest;
            break;
        }
    }

    if value.starts_with("R\"") {
        parse_raw_cpp_string(value)
    } else if value.starts_with('"') {
        parse_escaped_cpp_string(value)
    } else {
        None
    }
}

fn parse_raw_cpp_string(value: &str) -> Option<(String, &str)> {
    let rest = value.strip_prefix("R\"")?;
    let open = rest.find('(')?;
    let delimiter = &rest[..open];
    let body = &rest[open + 1..];
    let ending = format!("){delimiter}\"");
    let close = body.find(&ending)?;

    Some((body[..close].to_string(), &body[close + ending.len()..]))
}

fn parse_escaped_cpp_string(value: &str) -> Option<(String, &str)> {
    let mut characters = value.char_indices();
    if characters.next()?.1 != '"' {
        return None;
    }

    let mut decoded = String::new();
    while let Some((offset, character)) = characters.next() {
        if character == '"' {
            return Some((decoded, &value[offset + character.len_utf8()..]));
        }
        if character == '\\' {
            let (_, escaped) = characters.next()?;
            decoded.push(match escaped {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '\\' => '\\',
                '"' => '"',
                '\'' => '\'',
                other => other,
            });
        } else {
            decoded.push(character);
        }
    }

    None
}

fn skip_whitespace(source: &str, mut index: usize) -> usize {
    while source
        .as_bytes()
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    index
}

fn identifier_end(source: &str, mut index: usize) -> usize {
    while is_identifier_byte(source.as_bytes().get(index).copied()) {
        index += 1;
    }
    index
}

fn is_identifier_byte(byte: Option<u8>) -> bool {
    byte.is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn is_preprocessor_position(source: &str, index: usize) -> bool {
    let line_start = source[..index]
        .as_bytes()
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |position| position + 1);
    source[line_start..index].trim_start().starts_with('#')
}

fn skip_line_comment(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

fn skip_block_comment(bytes: &[u8], mut index: usize) -> usize {
    while index + 1 < bytes.len() {
        if bytes[index] == b'*' && bytes[index + 1] == b'/' {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
}

fn skip_quoted_string(bytes: &[u8], mut index: usize) -> usize {
    let quote = bytes[index];
    index += 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
        } else if bytes[index] == quote {
            return index + 1;
        } else {
            index += 1;
        }
    }
    bytes.len()
}

fn skip_raw_string(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut open = start + 2;
    while open < bytes.len() && bytes[open] != b'(' {
        open += 1;
    }
    if open == bytes.len() {
        return bytes.len();
    }

    let delimiter = &source[start + 2..open];
    let ending = format!("){delimiter}\"");
    let body_start = open + 1;
    source[body_start..]
        .find(&ending)
        .map_or(bytes.len(), |close| body_start + close + ending.len())
}

fn line_number(source: &str, offset: usize) -> usize {
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_literal_pistache_bindings_without_duplicates() {
        let source = r#"
Routes::Get(router, "/users", Routes::bind(&Users::list, this));
Pistache::Rest::Routes::Post(
    router,
    "/users",
    Routes::bind(&Users::create, this)
);
Routes::Get(router, "/users", Routes::bind(&Users::list, this));
Routes::Get(router, dynamic_path, Routes::bind(&Users::ignored, this));
"#;

        let routes = resolver().extract_routes("src/users.cpp", source);

        assert_eq!(routes.len(), 2);
        assert!(routes.iter().any(|route| {
            route.method == "GET"
                && route.path == "/users"
                && route.handler == "&Users::list"
                && route.framework == "pistache"
                && route.file == "src/users.cpp"
                && route.line == 2
        }));
        assert!(routes.iter().any(|route| {
            route.method == "POST"
                && route.path == "/users"
                && route.handler == "&Users::create"
                && route.line == 3
        }));
    }
}
