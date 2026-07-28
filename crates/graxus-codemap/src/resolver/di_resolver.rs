use std::collections::BTreeSet;

use tree_sitter::{Node, Parser};

use crate::facts::DIFact;

/// Extract dependency-injection facts from source whose language can be inferred from its file name.
///
/// Files without a recognized extension are scanned for both supported registration forms. This makes
/// the extractor useful for virtual documents while keeping framework-specific extractors available to
/// callers that already know the framework.
pub fn extract_di_bindings(file: &str, source: &str) -> Vec<DIFact> {
    let facts = if file.ends_with(".cs") {
        extract_aspnet_di_bindings(file, source)
    } else if file.ends_with(".ts") || file.ends_with(".tsx") {
        extract_nestjs_di_bindings(file, source)
    } else {
        let mut facts = extract_aspnet_di_bindings(file, source);
        facts.extend(extract_nestjs_di_bindings(file, source));
        facts
    };

    resolve_di_bindings(facts)
}

/// Extract ASP.NET Core's generic service-collection registrations.
///
/// `AddSingleton`, `AddScoped`, and `AddTransient` with one generic argument register a type as
/// itself. With two generic arguments, the first is the contract and the second is its implementation.
/// Other overloads do not state a concrete implementation type and are intentionally ignored.
pub fn extract_aspnet_di_bindings(file: &str, source: &str) -> Vec<DIFact> {
    let Some(tree) = parse_csharp(source) else {
        return Vec::new();
    };

    let mut invocations = Vec::new();
    collect_nodes_of_kind(tree.root_node(), "invocation_expression", &mut invocations);

    let facts = invocations
        .into_iter()
        .filter_map(|invocation| aspnet_registration(file, source, invocation))
        .collect();

    resolve_di_bindings(facts)
}

/// Extract NestJS injectable classes and `useClass` providers declared in `@Module`.
///
/// Nest's default provider scope is singleton. `Scope.REQUEST` and `Scope.TRANSIENT` are normalized
/// to the DI model's `scoped` and `transient` lifetimes respectively. A dynamic scope remains unknown.
pub fn extract_nestjs_di_bindings(file: &str, source: &str) -> Vec<DIFact> {
    let Some(tree) = parse_typescript(source) else {
        return Vec::new();
    };

    let root = tree.root_node();
    let mut classes = Vec::new();
    let mut decorators = Vec::new();
    let mut objects = Vec::new();
    collect_nodes_of_kind(root, "class_declaration", &mut classes);
    collect_nodes_of_kind(root, "decorator", &mut decorators);
    collect_nodes_of_kind(root, "object", &mut objects);

    let mut facts: Vec<_> = classes
        .into_iter()
        .filter_map(|class| nestjs_injectable(file, source, class))
        .collect();

    let module_ranges: Vec<_> = decorators
        .into_iter()
        .filter(|decorator| is_named_decorator(source_fragment(source, *decorator), "Module"))
        .map(|decorator| (decorator.start_byte(), decorator.end_byte()))
        .collect();

    if !module_ranges.is_empty() {
        facts.extend(objects.into_iter().filter_map(|object| {
            let is_module_provider = module_ranges
                .iter()
                .any(|(start, end)| *start <= object.start_byte() && object.end_byte() <= *end);
            is_module_provider
                .then(|| nestjs_use_class_provider(file, source, object))
                .flatten()
        }));
    }

    resolve_di_bindings(facts)
}

/// Retain one fact for each semantic DI binding while preserving first-seen source order.
///
/// Resolver input can contain explicit bindings emitted by another extractor. They are retained, but
/// repeated copies of the same contract-to-implementation registration are removed independently of
/// their generated IDs or source line numbers.
pub fn resolve_di_bindings(di_bindings: Vec<DIFact>) -> Vec<DIFact> {
    let mut seen = BTreeSet::new();

    di_bindings
        .into_iter()
        .filter(|binding| {
            seen.insert((
                binding.file.clone(),
                binding.language.clone(),
                binding.abstract_type.clone(),
                binding.concrete_type.clone(),
                binding.lifetime.clone(),
                binding.framework.clone(),
            ))
        })
        .collect()
}

fn parse_csharp(source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    let language = tree_sitter_c_sharp::LANGUAGE.into();
    parser.set_language(&language).ok()?;
    parser.parse(source, None)
}

fn parse_typescript(source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    let language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    parser.set_language(&language).ok()?;
    parser.parse(source, None)
}

fn collect_nodes_of_kind<'tree>(node: Node<'tree>, kind: &str, nodes: &mut Vec<Node<'tree>>) {
    if node.kind() == kind {
        nodes.push(node);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_nodes_of_kind(child, kind, nodes);
    }
}

fn aspnet_registration(file: &str, source: &str, invocation: Node<'_>) -> Option<DIFact> {
    let function = invocation.child_by_field_name("function")?;
    let function = source_fragment(source, function);

    let (lifetime, type_arguments) = [
        ("AddSingleton", "singleton"),
        ("AddScoped", "scoped"),
        ("AddTransient", "transient"),
    ]
    .into_iter()
    .find_map(|(method, lifetime)| {
        generic_method_arguments(function, method).map(|arguments| (lifetime, arguments))
    })?;

    let (abstract_type, concrete_type) = match type_arguments.as_slice() {
        [service] => (service.clone(), service.clone()),
        [service, implementation] => (service.clone(), implementation.clone()),
        _ => return None,
    };

    Some(new_fact(
        file,
        "csharp",
        abstract_type,
        concrete_type,
        Some(lifetime.to_string()),
        "aspnet",
        line_number(source, invocation.start_byte()),
    ))
}

fn generic_method_arguments(function: &str, method: &str) -> Option<Vec<String>> {
    let method_index = function.rfind(method)?;
    let before = &function[..method_index];
    if before
        .chars()
        .next_back()
        .is_some_and(is_identifier_character)
    {
        return None;
    }

    let after_method = &function[method_index + method.len()..];
    let after_method = after_method.trim_start();
    if !after_method.starts_with('<') {
        return None;
    }

    let closing_angle = matching_angle(after_method)?;
    if !after_method[closing_angle + 1..].trim().is_empty() {
        return None;
    }

    let type_arguments = split_top_level(&after_method[1..closing_angle], ',');
    if type_arguments.is_empty() {
        return None;
    }

    type_arguments.into_iter().map(normalize_type).collect()
}

fn matching_angle(value: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, character) in value.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level(value: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut angles = 0usize;
    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;

    for (index, character) in value.char_indices() {
        match character {
            '<' => angles += 1,
            '>' => angles = angles.saturating_sub(1),
            '(' => parentheses += 1,
            ')' => parentheses = parentheses.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            _ if character == separator
                && angles == 0
                && parentheses == 0
                && brackets == 0
                && braces == 0 =>
            {
                parts.push(&value[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    parts
}

fn normalize_type(value: &str) -> Option<String> {
    let normalized: String = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    (!normalized.is_empty()).then_some(normalized)
}

fn nestjs_injectable(file: &str, source: &str, class: Node<'_>) -> Option<DIFact> {
    let decorator = direct_named_decorator(source, class, "Injectable")?;
    let class_name =
        normalize_binding_reference(source_fragment(source, class.child_by_field_name("name")?))?;
    let lifetime = nestjs_lifetime(source_fragment(source, decorator));

    Some(new_fact(
        file,
        "typescript",
        class_name.clone(),
        class_name,
        lifetime,
        "nestjs",
        line_number(source, class.start_byte()),
    ))
}

fn nestjs_use_class_provider(file: &str, source: &str, object: Node<'_>) -> Option<DIFact> {
    let abstract_type =
        normalize_binding_reference(object_property_value(object, source, "provide")?)?;
    let concrete_type =
        normalize_binding_reference(object_property_value(object, source, "useClass")?)?;
    let lifetime = object_property_value(object, source, "scope")
        .map(nestjs_lifetime_from_scope)
        .unwrap_or_else(|| Some("singleton".to_string()));

    Some(new_fact(
        file,
        "typescript",
        abstract_type,
        concrete_type,
        lifetime,
        "nestjs",
        line_number(source, object.start_byte()),
    ))
}

fn direct_named_decorator<'tree>(
    source: &str,
    node: Node<'tree>,
    name: &str,
) -> Option<Node<'tree>> {
    // TypeScript attaches decorators to an enclosing `export_statement` when
    // a decorated class is exported, while non-exported classes own them.
    let owner = node
        .parent()
        .filter(|parent| parent.kind() == "export_statement")
        .unwrap_or(node);
    let mut cursor = owner.walk();
    let decorator = owner.children(&mut cursor).find(|child| {
        child.kind() == "decorator" && is_named_decorator(source_fragment(source, *child), name)
    });
    decorator
}

fn is_named_decorator(value: &str, name: &str) -> bool {
    let Some(rest) = value.trim().strip_prefix('@') else {
        return false;
    };
    let Some(after_name) = rest.trim_start().strip_prefix(name) else {
        return false;
    };
    let after_name = after_name.trim_start();

    !after_name
        .chars()
        .next()
        .is_some_and(is_identifier_character)
        && (after_name.is_empty() || after_name.starts_with('('))
}

fn object_property_value<'a>(object: Node<'_>, source: &'a str, property: &str) -> Option<&'a str> {
    let mut cursor = object.walk();
    let value = object.children(&mut cursor).find_map(|child| {
        (child.kind() == "pair").then(|| {
            let pair = source_fragment(source, child);
            let separator = top_level_separator(pair, ':')?;
            let key = pair[..separator].trim().trim_matches(['"', '\'']);
            (key == property).then_some(pair[separator + 1..].trim())
        })?
    });
    value
}

fn top_level_separator(value: &str, separator: char) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    let mut parentheses = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;

    for (index, character) in value.char_indices() {
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == delimiter {
                quote = None;
            }
            continue;
        }

        match character {
            '\'' | '"' | '`' => quote = Some(character),
            '(' => parentheses += 1,
            ')' => parentheses = parentheses.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            _ if character == separator && parentheses == 0 && brackets == 0 && braces == 0 => {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn normalize_binding_reference(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() >= 2
        && matches!(value.as_bytes().first(), Some(b'\'' | b'"'))
        && value.as_bytes().first() == value.as_bytes().last()
    {
        let token = &value[1..value.len() - 1];
        return (!token.is_empty()).then_some(token.to_string());
    }

    let normalized: String = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let mut characters = normalized.chars();
    let first = characters.next()?;
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return None;
    }
    characters
        .all(|character| {
            character == '_'
                || character == '$'
                || character == '.'
                || character.is_ascii_alphanumeric()
        })
        .then_some(normalized)
}

fn nestjs_lifetime(decorator: &str) -> Option<String> {
    scope_value(decorator)
        .map(nestjs_lifetime_from_scope)
        .unwrap_or_else(|| Some("singleton".to_string()))
}

fn nestjs_lifetime_from_scope(scope: &str) -> Option<String> {
    match normalize_type(scope).as_deref() {
        Some("Scope.DEFAULT") => Some("singleton".to_string()),
        Some("Scope.REQUEST") => Some("scoped".to_string()),
        Some("Scope.TRANSIENT") => Some("transient".to_string()),
        _ => None,
    }
}

fn scope_value(value: &str) -> Option<&str> {
    let mut offset = 0;
    while let Some(relative_index) = value[offset..].find("scope") {
        let index = offset + relative_index;
        let before = value[..index].chars().next_back();
        let after = &value[index + "scope".len()..];
        if !before.is_some_and(is_identifier_character) {
            let after = after.trim_start();
            if let Some(after_colon) = after.strip_prefix(':') {
                let end = after_colon
                    .find(|character| matches!(character, ',' | '}' | ')' | '\n' | '\r'))
                    .unwrap_or(after_colon.len());
                return Some(after_colon[..end].trim());
            }
        }
        offset = index + "scope".len();
    }
    None
}

fn new_fact(
    file: &str,
    language: &str,
    abstract_type: String,
    concrete_type: String,
    lifetime: Option<String>,
    framework: &str,
    line: usize,
) -> DIFact {
    DIFact {
        id: format!(
            "di:{framework}:{file}:{line}:{abstract_type}:{concrete_type}:{}",
            lifetime.as_deref().unwrap_or("unknown")
        ),
        file: file.to_string(),
        language: language.to_string(),
        abstract_type,
        concrete_type,
        lifetime,
        line,
        framework: framework.to_string(),
    }
}

fn source_fragment<'a>(source: &'a str, node: Node<'_>) -> &'a str {
    source.get(node.byte_range()).unwrap_or_default()
}

fn line_number(source: &str, byte_offset: usize) -> usize {
    source
        .get(..byte_offset)
        .unwrap_or_default()
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character == '$' || character.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_aspnet_generic_lifetimes_and_deduplicates_registrations() {
        let source = r#"
builder.Services.AddSingleton<IClock, SystemClock>();
builder.Services.AddScoped<IRepository, SqlRepository>();
builder.Services.AddTransient<EmailSender>();
builder.Services.AddSingleton<IClock, SystemClock>();
"#;

        let facts = extract_di_bindings("src/Program.cs", source);

        assert_eq!(facts.len(), 3);
        assert_eq!(
            facts
                .iter()
                .map(|fact| {
                    (
                        fact.abstract_type.as_str(),
                        fact.concrete_type.as_str(),
                        fact.lifetime.as_deref(),
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                ("IClock", "SystemClock", Some("singleton")),
                ("IRepository", "SqlRepository", Some("scoped")),
                ("EmailSender", "EmailSender", Some("transient")),
            ]
        );
        assert!(facts.iter().all(|fact| {
            fact.file == "src/Program.cs" && fact.language == "csharp" && fact.framework == "aspnet"
        }));
    }

    #[test]
    fn extracts_nestjs_injectables_and_explicit_use_class_providers() {
        let source = r#"
import { Injectable, Module, Scope } from "@nestjs/common";

@Injectable()
export class CacheService {}

@Injectable({ scope: Scope.REQUEST })
export class RequestContext {}

@Module({
  providers: [
    { provide: "CACHE", useClass: CacheService },
    { provide: AuditLog, useClass: RequestContext, scope: Scope.TRANSIENT },
  ],
})
export class AppModule {}
"#;

        let facts = extract_di_bindings("src/app.module.ts", source);

        assert_eq!(facts.len(), 4);
        assert_eq!(
            facts
                .iter()
                .map(|fact| {
                    (
                        fact.abstract_type.as_str(),
                        fact.concrete_type.as_str(),
                        fact.lifetime.as_deref(),
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                ("CacheService", "CacheService", Some("singleton")),
                ("RequestContext", "RequestContext", Some("scoped")),
                ("CACHE", "CacheService", Some("singleton")),
                ("AuditLog", "RequestContext", Some("transient")),
            ]
        );
        assert!(facts.iter().all(|fact| {
            fact.file == "src/app.module.ts"
                && fact.language == "typescript"
                && fact.framework == "nestjs"
        }));
    }

    #[test]
    fn ignores_malformed_or_incomplete_registrations() {
        let source = r#"
const malformed = { provide: Token, useClass: };
@Injectable({ scope: getScope() })
export class DynamicScope {}
@Module({ providers: [{ provide: Token, useClass: forwardRef(() => Service) }] })
"#;

        let facts = extract_di_bindings("src/app.module.ts", source);

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].abstract_type, "DynamicScope");
        assert_eq!(facts[0].concrete_type, "DynamicScope");
        assert_eq!(facts[0].lifetime, None);
    }
}
