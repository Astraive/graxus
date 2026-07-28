use std::cmp::Ordering;

use tree_sitter::{Language, Node, Parser};

use crate::facts::{ImplKind, TypeImplFact};

/// Extract direct type relationships from one source file.
///
/// This intentionally works from syntax, not from the graph's symbol table:
/// an implemented trait or base type may be external to the indexed project.
/// The resulting facts therefore describe only relationships explicitly
/// declared in the source and never speculate about a target declaration.
pub fn extract_type_impls(file: &str, source: &str, language: &str) -> Vec<TypeImplFact> {
    let language = language.to_ascii_lowercase();
    let (grammar, declaration_kind) = match language.as_str() {
        "rust" => (tree_sitter_rust::LANGUAGE.into(), "impl_item"),
        "typescript" => (
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "class_declaration",
        ),
        "csharp" => (tree_sitter_c_sharp::LANGUAGE.into(), "class_declaration"),
        "java" => (tree_sitter_java::LANGUAGE.into(), "class_declaration"),
        "kotlin" => (tree_sitter_kotlin_ng::LANGUAGE.into(), "class_declaration"),
        _ => return Vec::new(),
    };

    let Some(tree) = parse_source(grammar, source) else {
        return Vec::new();
    };

    let mut declarations = Vec::new();
    collect_declarations(tree.root_node(), declaration_kind, &mut declarations);

    let mut facts = Vec::new();
    for declaration in declarations {
        let Ok(declaration_source) = declaration.utf8_text(source.as_bytes()) else {
            continue;
        };
        let line = declaration.start_position().row + 1;
        match language.as_str() {
            "rust" => extract_rust_impls(file, &language, declaration_source, line, &mut facts),
            "typescript" => {
                extract_typescript_class(file, &language, declaration_source, line, &mut facts)
            }
            "csharp" => extract_csharp_class(file, &language, declaration_source, line, &mut facts),
            "java" => extract_java_class(file, &language, declaration_source, line, &mut facts),
            "kotlin" => extract_kotlin_class(file, &language, declaration_source, line, &mut facts),
            _ => {}
        }
    }

    resolve_type_impls(facts)
}

/// Normalize and deduplicate explicit type relationships.
///
/// Resolution does not attempt to bind names to symbols: a trait, interface,
/// or base class can legitimately live outside the scanned project.  It only
/// chooses one representation for a repeated source relationship.  In
/// particular, this lets syntax extraction correct Ripex's generic
/// `Extends` classification for TypeScript `implements` clauses.
pub fn resolve_type_impls(mut type_impls: Vec<TypeImplFact>) -> Vec<TypeImplFact> {
    type_impls.retain(|fact| {
        !fact.file.is_empty()
            && !fact.language.is_empty()
            && !fact.implementing_type.is_empty()
            && !fact.trait_or_interface.is_empty()
    });

    type_impls.sort_by(compare_facts);
    type_impls.dedup_by(|left, right| same_relationship(left, right));
    type_impls
}

fn parse_source(language: Language, source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    if parser.set_language(&language).is_err() {
        return None;
    }
    parser.parse(source, None)
}

fn collect_declarations<'tree>(
    node: Node<'tree>,
    declaration_kind: &str,
    declarations: &mut Vec<Node<'tree>>,
) {
    if node.kind() == declaration_kind {
        declarations.push(node);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_declarations(child, declaration_kind, declarations);
    }
}

fn extract_rust_impls(
    file: &str,
    language: &str,
    declaration: &str,
    line: usize,
    facts: &mut Vec<TypeImplFact>,
) {
    let masked = mask_non_code(declaration);
    let Some(impl_start) = find_keyword(&masked, "impl", 0) else {
        return;
    };
    let after_impl = skip_whitespace(&masked, impl_start + "impl".len());
    let rest_start = skip_leading_generic_parameters(&masked, after_impl);
    let header_end = find_body_start(&masked, rest_start).unwrap_or(masked.len());
    let header = &declaration[..header_end];
    let header_masked = &masked[..header_end];

    if let Some(for_start) = find_top_level_keyword(header_masked, "for", rest_start) {
        let trait_name = canonical_type_name(&header[rest_start..for_start]);
        let type_start = skip_whitespace(header_masked, for_start + "for".len());
        let implementing_type = canonical_type_name(
            &header[type_start..trim_at_top_level_keyword(header_masked, "where", type_start)],
        );

        let (Some(trait_name), Some(implementing_type)) = (trait_name, implementing_type) else {
            return;
        };
        // Negative implementations (`impl !Send for Type`) do not declare an
        // implementation relationship.
        if trait_name.starts_with('!') {
            return;
        }
        push_fact(
            facts,
            file,
            language,
            implementing_type,
            trait_name,
            line,
            ImplKind::TraitImpl,
        );
    }

    // Inherent `impl Type { ... }` blocks are intentionally omitted.  They
    // have no second participant, while TypeImplFact represents only directed
    // trait/interface and inheritance relationships.
}

fn extract_typescript_class(
    file: &str,
    language: &str,
    declaration: &str,
    line: usize,
    facts: &mut Vec<TypeImplFact>,
) {
    let Some(class) = class_header(declaration) else {
        return;
    };

    if let Some(extends_start) = find_top_level_keyword(&class.masked, "extends", class.name_end) {
        let end = first_clause_boundary(
            &class.masked,
            extends_start + "extends".len(),
            &["implements"],
        );
        push_clause_relations(
            facts,
            file,
            language,
            &class.name,
            &class.header[skip_whitespace(&class.masked, extends_start + "extends".len())..end],
            line,
            ImplKind::Extends,
        );
    }

    if let Some(implements_start) =
        find_top_level_keyword(&class.masked, "implements", class.name_end)
    {
        let start = skip_whitespace(&class.masked, implements_start + "implements".len());
        push_clause_relations(
            facts,
            file,
            language,
            &class.name,
            &class.header[start..],
            line,
            ImplKind::Implements,
        );
    }
}

fn extract_csharp_class(
    file: &str,
    language: &str,
    declaration: &str,
    line: usize,
    facts: &mut Vec<TypeImplFact>,
) {
    let Some(class) = class_header(declaration) else {
        return;
    };
    let Some(colon) = find_top_level_char(&class.masked, b':', class.name_end) else {
        return;
    };
    let end = trim_at_top_level_keyword(&class.masked, "where", colon + 1);
    push_clause_relations(
        facts,
        file,
        language,
        &class.name,
        &class.header[colon + 1..end],
        line,
        ImplKind::CSharpInheritance,
    );
}

fn extract_java_class(
    file: &str,
    language: &str,
    declaration: &str,
    line: usize,
    facts: &mut Vec<TypeImplFact>,
) {
    let Some(class) = class_header(declaration) else {
        return;
    };

    if let Some(extends_start) = find_top_level_keyword(&class.masked, "extends", class.name_end) {
        let end = first_clause_boundary(
            &class.masked,
            extends_start + "extends".len(),
            &["implements"],
        );
        push_clause_relations(
            facts,
            file,
            language,
            &class.name,
            &class.header[skip_whitespace(&class.masked, extends_start + "extends".len())..end],
            line,
            ImplKind::Extends,
        );
    }

    if let Some(implements_start) =
        find_top_level_keyword(&class.masked, "implements", class.name_end)
    {
        let start = skip_whitespace(&class.masked, implements_start + "implements".len());
        push_clause_relations(
            facts,
            file,
            language,
            &class.name,
            &class.header[start..],
            line,
            ImplKind::Implements,
        );
    }
}

fn extract_kotlin_class(
    file: &str,
    language: &str,
    declaration: &str,
    line: usize,
    facts: &mut Vec<TypeImplFact>,
) {
    let Some(class) = class_header(declaration) else {
        return;
    };
    let Some(colon) = find_top_level_char(&class.masked, b':', class.name_end) else {
        return;
    };

    // Kotlin uses one colon-separated supertype list.  Its grammar does not
    // distinguish an interface-only first entry without semantic information,
    // so record the first relation as `Extends` and the rest as `Implements`.
    let supertypes = split_top_level(&class.header[colon + 1..]);
    for (index, supertype) in supertypes.into_iter().enumerate() {
        let Some(supertype) = canonical_type_name(supertype) else {
            continue;
        };
        let kind = if index == 0 {
            ImplKind::Extends
        } else {
            ImplKind::Implements
        };
        push_fact(
            facts,
            file,
            language,
            class.name.clone(),
            supertype,
            line,
            kind,
        );
    }
}

struct ClassHeader<'a> {
    header: &'a str,
    masked: String,
    name: String,
    name_end: usize,
}

fn class_header(declaration: &str) -> Option<ClassHeader<'_>> {
    let masked_declaration = mask_non_code(declaration);
    let class_start = find_keyword(&masked_declaration, "class", 0)?;
    let header_end = find_body_start(&masked_declaration, class_start).unwrap_or(declaration.len());
    let header = &declaration[..header_end];
    let masked = masked_declaration[..header_end].to_string();
    let mut name_start = skip_whitespace(&masked, class_start + "class".len());
    if masked.as_bytes().get(name_start) == Some(&b'@') {
        name_start += 1;
    }
    let name_end = identifier_end(&masked, name_start);
    if name_start == name_end {
        return None;
    }
    let name = canonical_type_name(&header[name_start..name_end])?;

    Some(ClassHeader {
        header,
        masked,
        name,
        name_end,
    })
}

fn push_clause_relations(
    facts: &mut Vec<TypeImplFact>,
    file: &str,
    language: &str,
    implementing_type: &str,
    clause: &str,
    line: usize,
    kind: ImplKind,
) {
    for relation in split_top_level(clause) {
        let Some(relation) = canonical_type_name(relation) else {
            continue;
        };
        push_fact(
            facts,
            file,
            language,
            implementing_type.to_string(),
            relation,
            line,
            kind,
        );
    }
}

fn push_fact(
    facts: &mut Vec<TypeImplFact>,
    file: &str,
    language: &str,
    implementing_type: String,
    trait_or_interface: String,
    line: usize,
    kind: ImplKind,
) {
    let id = format!(
        "type-impl:{file}:{implementing_type}:{trait_or_interface}:{line}:{}",
        impl_kind_name(kind)
    );
    facts.push(TypeImplFact {
        id,
        file: file.to_string(),
        language: language.to_string(),
        implementing_type,
        trait_or_interface,
        line,
        kind,
    });
}

fn canonical_type_name(raw: &str) -> Option<String> {
    let masked = mask_non_code(raw);
    let end = first_type_suffix(&masked);
    let mut name = raw[..end].trim();
    if let Some(by_start) = find_top_level_keyword(&masked, "by", 0) {
        name = raw[..by_start].trim();
    }
    name = name
        .strip_prefix("dyn ")
        .or_else(|| name.strip_prefix("impl "))
        .unwrap_or(name)
        .trim();
    name = name.trim_end_matches(['?', '!']).trim();
    name = name.trim_matches('`');

    if name.is_empty() {
        return None;
    }

    let normalized: String = name
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let normalized = normalized.trim_start_matches('@');
    if normalized.is_empty()
        || !normalized.chars().all(|character| {
            character.is_alphanumeric() || matches!(character, '_' | ':' | '.' | '$' | '#')
        })
    {
        return None;
    }
    Some(normalized.to_string())
}

fn first_type_suffix(masked: &str) -> usize {
    let bytes = masked.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(byte, b'<' | b'(' | b'[' | b'{' | b';') {
            return index;
        }
    }
    masked.len()
}

fn first_clause_boundary(masked: &str, start: usize, keywords: &[&str]) -> usize {
    keywords
        .iter()
        .filter_map(|keyword| find_top_level_keyword(masked, keyword, start))
        .min()
        .unwrap_or(masked.len())
}

fn trim_at_top_level_keyword(masked: &str, keyword: &str, start: usize) -> usize {
    find_top_level_keyword(masked, keyword, start).unwrap_or(masked.len())
}

fn skip_leading_generic_parameters(masked: &str, start: usize) -> usize {
    if masked.as_bytes().get(start) != Some(&b'<') {
        return start;
    }

    let mut depth = 0usize;
    for (index, byte) in masked.as_bytes().iter().enumerate().skip(start) {
        match byte {
            b'<' => depth += 1,
            b'>' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    return skip_whitespace(masked, index + 1);
                }
            }
            _ => {}
        }
    }
    masked.len()
}

fn find_body_start(masked: &str, start: usize) -> Option<usize> {
    let mut depths = DelimiterDepths::default();
    for (index, byte) in masked.as_bytes().iter().enumerate().skip(start) {
        if *byte == b'{' && depths.is_top_level() {
            return Some(index);
        }
        depths.consume(*byte);
    }
    None
}

fn find_top_level_char(masked: &str, target: u8, start: usize) -> Option<usize> {
    let mut depths = DelimiterDepths::default();
    for (index, byte) in masked.as_bytes().iter().enumerate().skip(start) {
        if *byte == target && depths.is_top_level() {
            return Some(index);
        }
        depths.consume(*byte);
    }
    None
}

fn find_top_level_keyword(masked: &str, keyword: &str, start: usize) -> Option<usize> {
    let mut depths = DelimiterDepths::default();
    let bytes = masked.as_bytes();
    let keyword = keyword.as_bytes();

    for index in start..bytes.len() {
        if depths.is_top_level()
            && bytes[index..].starts_with(keyword)
            && is_keyword_boundary(bytes, index, keyword.len())
        {
            return Some(index);
        }
        depths.consume(bytes[index]);
    }
    None
}

fn find_keyword(masked: &str, keyword: &str, start: usize) -> Option<usize> {
    let bytes = masked.as_bytes();
    let keyword = keyword.as_bytes();
    (start..bytes.len()).find(|&index| {
        bytes[index..].starts_with(keyword) && is_keyword_boundary(bytes, index, keyword.len())
    })
}

fn is_keyword_boundary(bytes: &[u8], start: usize, len: usize) -> bool {
    let before = start.checked_sub(1).and_then(|index| bytes.get(index));
    let after = bytes.get(start + len);
    !before.is_some_and(|byte| is_identifier_byte(*byte))
        && !after.is_some_and(|byte| is_identifier_byte(*byte))
}

fn identifier_end(masked: &str, start: usize) -> usize {
    let bytes = masked.as_bytes();
    let mut end = start;
    while bytes.get(end).is_some_and(|byte| is_identifier_byte(*byte)) {
        end += 1;
    }
    end
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$') || byte >= 0x80
}

fn skip_whitespace(text: &str, mut start: usize) -> usize {
    while text
        .as_bytes()
        .get(start)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        start += 1;
    }
    start
}

fn split_top_level(text: &str) -> Vec<&str> {
    let masked = mask_non_code(text);
    let mut depths = DelimiterDepths::default();
    let mut start = 0;
    let mut parts = Vec::new();

    for (index, byte) in masked.as_bytes().iter().enumerate() {
        if *byte == b',' && depths.is_top_level() {
            parts.push(&text[start..index]);
            start = index + 1;
        } else {
            depths.consume(*byte);
        }
    }
    parts.push(&text[start..]);
    parts
}

#[derive(Default)]
struct DelimiterDepths {
    angle: usize,
    paren: usize,
    bracket: usize,
}

impl DelimiterDepths {
    fn is_top_level(&self) -> bool {
        self.angle == 0 && self.paren == 0 && self.bracket == 0
    }

    fn consume(&mut self, byte: u8) {
        match byte {
            b'<' => self.angle += 1,
            b'>' if self.angle > 0 => self.angle -= 1,
            b'(' => self.paren += 1,
            b')' if self.paren > 0 => self.paren -= 1,
            b'[' => self.bracket += 1,
            b']' if self.bracket > 0 => self.bracket -= 1,
            _ => {}
        }
    }
}

/// Replace comments and literals with spaces while preserving every byte
/// offset.  Header parsing can then use byte offsets safely on the original
/// declaration and cannot mistake quoted or commented keywords for syntax.
fn mask_non_code(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        LineComment,
        BlockComment,
        String { quote: u8, escaped: bool },
        Char { escaped: bool },
    }

    let bytes = source.as_bytes();
    let mut masked = bytes.to_vec();
    let mut state = State::Code;
    let mut index = 0;

    while index < bytes.len() {
        match state {
            State::Code => match bytes[index] {
                b'/' if bytes.get(index + 1) == Some(&b'/') => {
                    masked[index] = b' ';
                    masked[index + 1] = b' ';
                    index += 2;
                    state = State::LineComment;
                }
                b'/' if bytes.get(index + 1) == Some(&b'*') => {
                    masked[index] = b' ';
                    masked[index + 1] = b' ';
                    index += 2;
                    state = State::BlockComment;
                }
                b'"' => {
                    masked[index] = b' ';
                    index += 1;
                    state = State::String {
                        quote: b'"',
                        escaped: false,
                    };
                }
                b'\'' => {
                    masked[index] = b' ';
                    index += 1;
                    state = State::Char { escaped: false };
                }
                _ => index += 1,
            },
            State::LineComment => {
                if bytes[index] == b'\n' {
                    state = State::Code;
                } else {
                    masked[index] = b' ';
                }
                index += 1;
            }
            State::BlockComment => {
                if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    masked[index] = b' ';
                    masked[index + 1] = b' ';
                    index += 2;
                    state = State::Code;
                } else {
                    if bytes[index] != b'\n' {
                        masked[index] = b' ';
                    }
                    index += 1;
                }
            }
            State::String { quote, escaped } => {
                if bytes[index] != b'\n' {
                    masked[index] = b' ';
                }
                let current = bytes[index];
                if !escaped && current == quote {
                    state = State::Code;
                } else {
                    state = State::String {
                        quote,
                        escaped: !escaped && current == b'\\',
                    };
                }
                index += 1;
            }
            State::Char { escaped } => {
                if bytes[index] != b'\n' {
                    masked[index] = b' ';
                }
                let current = bytes[index];
                if !escaped && current == b'\'' {
                    state = State::Code;
                } else {
                    state = State::Char {
                        escaped: !escaped && current == b'\\',
                    };
                }
                index += 1;
            }
        }
    }

    // The mask contains only original UTF-8 bytes and ASCII spaces/newlines.
    String::from_utf8(masked).unwrap_or_default()
}

fn compare_facts(left: &TypeImplFact, right: &TypeImplFact) -> Ordering {
    (
        left.file.as_str(),
        left.language.as_str(),
        left.implementing_type.as_str(),
        left.trait_or_interface.as_str(),
    )
        .cmp(&(
            right.file.as_str(),
            right.language.as_str(),
            right.implementing_type.as_str(),
            right.trait_or_interface.as_str(),
        ))
        .then_with(|| relationship_priority(left.kind).cmp(&relationship_priority(right.kind)))
        .then_with(|| left.line.cmp(&right.line))
        .then_with(|| left.id.cmp(&right.id))
}

fn same_relationship(left: &TypeImplFact, right: &TypeImplFact) -> bool {
    left.file == right.file
        && left.language == right.language
        && left.implementing_type == right.implementing_type
        && left.trait_or_interface == right.trait_or_interface
}

fn relationship_priority(kind: ImplKind) -> u8 {
    match kind {
        ImplKind::Implements => 0,
        ImplKind::TraitImpl => 1,
        ImplKind::Derive => 2,
        ImplKind::Extends => 3,
        ImplKind::CSharpInheritance => 4,
        ImplKind::CppInheritance => 5,
    }
}

fn impl_kind_name(kind: ImplKind) -> &'static str {
    match kind {
        ImplKind::TraitImpl => "trait_impl",
        ImplKind::Derive => "derive",
        ImplKind::Implements => "implements",
        ImplKind::Extends => "extends",
        ImplKind::CSharpInheritance => "csharp_inheritance",
        ImplKind::CppInheritance => "cpp_inheritance",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_fact(
        facts: &[TypeImplFact],
        implementing_type: &str,
        trait_or_interface: &str,
        kind: ImplKind,
    ) -> bool {
        facts.iter().any(|fact| {
            fact.implementing_type == implementing_type
                && fact.trait_or_interface == trait_or_interface
                && fact.kind == kind
        })
    }

    #[test]
    fn extracts_rust_trait_impls_and_skips_inherent_blocks() {
        let facts = extract_type_impls(
            "src/person.rs",
            r#"
                trait Greeter {}
                struct Person;
                impl Greeter for Person {}
                impl Person {}
            "#,
            "rust",
        );

        assert_eq!(facts.len(), 1);
        assert!(has_fact(&facts, "Person", "Greeter", ImplKind::TraitImpl));
        assert!(facts
            .iter()
            .all(|fact| fact.file == "src/person.rs" && fact.language == "rust"));
    }

    #[test]
    fn extracts_typescript_extends_and_implements_without_duplicates() {
        let facts = extract_type_impls(
            "src/admin.ts",
            r#"
                class Admin<T> extends User<T> implements Auditable, Serializable {
                    id = 1;
                }
            "#,
            "typescript",
        );

        assert_eq!(facts.len(), 3);
        assert!(has_fact(&facts, "Admin", "User", ImplKind::Extends));
        assert!(has_fact(&facts, "Admin", "Auditable", ImplKind::Implements));
        assert!(has_fact(
            &facts,
            "Admin",
            "Serializable",
            ImplKind::Implements
        ));
    }

    #[test]
    fn extracts_csharp_base_and_interfaces() {
        let facts = extract_type_impls(
            "Handlers.cs",
            r#"
                public class Handler : BaseHandler, IRequestHandler, IDisposable {}
            "#,
            "csharp",
        );

        assert_eq!(facts.len(), 3);
        assert!(has_fact(
            &facts,
            "Handler",
            "BaseHandler",
            ImplKind::CSharpInheritance
        ));
        assert!(has_fact(
            &facts,
            "Handler",
            "IRequestHandler",
            ImplKind::CSharpInheritance
        ));
        assert!(has_fact(
            &facts,
            "Handler",
            "IDisposable",
            ImplKind::CSharpInheritance
        ));
    }

    #[test]
    fn extracts_java_and_kotlin_supertype_lists() {
        let java = extract_type_impls(
            "Service.java",
            "class Service extends BaseService implements Runnable, Closeable {}",
            "java",
        );
        assert_eq!(java.len(), 3);
        assert!(has_fact(&java, "Service", "BaseService", ImplKind::Extends));
        assert!(has_fact(&java, "Service", "Runnable", ImplKind::Implements));
        assert!(has_fact(
            &java,
            "Service",
            "Closeable",
            ImplKind::Implements
        ));

        let kotlin = extract_type_impls(
            "Service.kt",
            "class Service(value: String) : BaseService(), Runnable, Closeable {}",
            "kotlin",
        );
        assert_eq!(kotlin.len(), 3);
        assert!(has_fact(
            &kotlin,
            "Service",
            "BaseService",
            ImplKind::Extends
        ));
        assert!(has_fact(
            &kotlin,
            "Service",
            "Runnable",
            ImplKind::Implements
        ));
        assert!(has_fact(
            &kotlin,
            "Service",
            "Closeable",
            ImplKind::Implements
        ));
    }

    #[test]
    fn resolution_prefers_explicit_implements_over_generic_extends() {
        let source = extract_type_impls(
            "service.ts",
            "class Service implements Contract {}",
            "typescript",
        );
        let mut duplicate = source[0].clone();
        duplicate.id = "ripex:incorrect-kind".to_string();
        duplicate.kind = ImplKind::Extends;

        let resolved = resolve_type_impls(vec![duplicate, source[0].clone()]);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].kind, ImplKind::Implements);
    }

    #[test]
    fn malformed_source_is_safe_and_does_not_extract_comments() {
        let facts = extract_type_impls(
            "broken.ts",
            r#"
                // class Pretend extends Imaginary implements Never {}
                class Broken<T extends { id: string }> implements Contract
            "#,
            "typescript",
        );

        assert!(facts.iter().all(|fact| fact.implementing_type != "Pretend"));
    }
}
