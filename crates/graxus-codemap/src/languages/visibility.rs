//! Shared visibility-detection helpers for language indexers.
//!
//! Each language's tree-sitter grammar exposes visibility differently (or not
//! at all as a dedicated node). Rather than depend on the exact child-node
//! name per grammar, these helpers inspect the source text at/just-before the
//! definition node's start position for the language's visibility keywords.
//!
//! Every helper returns `(exported, visibility)` where `exported` is reserved
//! for symbols reachable from outside their defining unit (module/crate/file):
//! the dead-code scan treats exported symbols as presumed-public-API and
//! excludes them unless `--include-exported` is passed.

use crate::Visibility;

/// Look backwards from the definition's start line, collecting non-blank text,
/// to read any modifier keywords that precede the declaration. Returns the
/// joined leading text (lowercased) for keyword scanning. Stops at the first
/// blank line or after `max_lines` lines.
///
/// `def_node` is the `@def` capture node for the declaration.
fn leading_text(def_node: Option<tree_sitter::Node>, source: &str, max_lines: usize) -> String {
    let Some(node) = def_node else {
        return String::new();
    };
    let start_row = node.start_position().row;
    let lines: Vec<&str> = source.lines().collect();
    let mut acc: Vec<&str> = Vec::new();
    // Walk upward from start_row, collecting modifier lines (attributes,
    // annotations, `export`, `pub`, access specifiers). Stop at a blank line.
    for i in (0..=start_row).rev() {
        if i >= lines.len() {
            break;
        }
        let line = lines[i].trim();
        if i == start_row {
            acc.push(line);
            continue;
        }
        if line.is_empty() {
            break;
        }
        acc.push(line);
        if acc.len() >= max_lines {
            break;
        }
    }
    acc.iter()
        .rev()
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

// ── Rust ───────────────────────────────────────────────────────────────────

/// Rust visibility from a definition node: walks the immediate children for a
/// `visibility_modifier` node (the tree-sitter-rust grammar exposes `pub`,
/// `pub(crate)`, etc. this way).
pub fn rust_visibility(def_node: Option<tree_sitter::Node>, source: &str) -> (bool, Visibility) {
    let Some(node) = def_node else {
        return (false, Visibility::Unknown);
    };
    let mut i = 0;
    while let Some(child) = node.child(i) {
        if child.kind() == "visibility_modifier" {
            let text = source[child.byte_range()].trim();
            if text == "pub" {
                return (true, Visibility::Public);
            }
            if text.starts_with("pub(crate)") || text.starts_with("pub (crate)") {
                return (false, Visibility::Internal);
            }
            // pub(super), pub(in path), or other restricted pub.
            return (false, Visibility::Protected);
        }
        i += 1;
    }
    (false, Visibility::Private)
}

// ── C ──────────────────────────────────────────────────────────────────────

/// C visibility/linkage. In C, a function/variable has external linkage unless
/// declared `static` (file-local). Types (struct/enum/typedef) have no linkage;
/// treat them as public (visible across translation units via headers).
pub fn c_visibility(
    def_node: Option<tree_sitter::Node>,
    source: &str,
    kind_is_type: bool,
) -> (bool, Visibility) {
    if kind_is_type {
        return (true, Visibility::Public);
    }
    let lead = leading_text(def_node, source, 2);
    if lead.contains("static") {
        (false, Visibility::Private)
    } else {
        // External linkage — visible to other translation units.
        (true, Visibility::Public)
    }
}

// ── C++ ────────────────────────────────────────────────────────────────────

/// C++ visibility. Free functions/types default to external linkage unless
/// `static` or inside an anonymous namespace. Class members follow the most
/// recent `public:`/`private:`/`protected:` access specifier in scope — we
/// approximate by scanning the leading text for the nearest specifier.
pub fn cpp_visibility(
    def_node: Option<tree_sitter::Node>,
    source: &str,
    is_member: bool,
) -> (bool, Visibility) {
    if is_member {
        // Scan a wider window to find the enclosing access specifier.
        let lead = leading_text(def_node, source, 40);
        if lead.contains("private:") {
            return (false, Visibility::Private);
        }
        if lead.contains("protected:") {
            return (false, Visibility::Protected);
        }
        // public: or none — default to public for members.
        return (true, Visibility::Public);
    }
    // Free function / type.
    let lead = leading_text(def_node, source, 3);
    if lead.contains("static") {
        (false, Visibility::Private)
    } else {
        (true, Visibility::Public)
    }
}

// ── C# ─────────────────────────────────────────────────────────────────────

/// C# visibility. Read modifier keywords on the declaration line.
pub fn csharp_visibility(def_node: Option<tree_sitter::Node>, source: &str) -> (bool, Visibility) {
    let lead = leading_text(def_node, source, 2);
    // The declaration line is the last element collected; check modifiers.
    if lead.contains("public") {
        (true, Visibility::Public)
    } else if lead.contains("protected internal") || lead.contains("internal protected") {
        (false, Visibility::Protected)
    } else if lead.contains("internal") {
        (false, Visibility::Internal)
    } else if lead.contains("protected") {
        (false, Visibility::Protected)
    } else if lead.contains("private") {
        (false, Visibility::Private)
    } else {
        // C# default is private for members, internal for top-level types.
        (false, Visibility::Private)
    }
}

// ── TypeScript / JavaScript ────────────────────────────────────────────────

/// TypeScript visibility. `export` makes a declaration reachable from other
/// modules. Class/interface/type members use `public`/`private`/`protected`,
/// but for dead-code purposes the `export` keyword on the top-level declaration
/// is what matters.
pub fn typescript_visibility(
    def_node: Option<tree_sitter::Node>,
    source: &str,
) -> (bool, Visibility) {
    let lead = leading_text(def_node, source, 3);
    let exported =
        lead.contains("export ") || lead.contains("export\t") || lead.contains("export(");
    let visibility = if lead.contains("private ") {
        Visibility::Private
    } else if lead.contains("protected ") {
        Visibility::Protected
    } else if lead.contains("public ") {
        Visibility::Public
    } else {
        Visibility::Unknown
    };
    (exported, visibility)
}

// ── Java / Kotlin / Swift ──────────────────────────────────────────────────

/// Java-family visibility (Java, Kotlin, Swift). All three use modifier
/// keywords (`public`/`private`/`protected`/`internal`) on the declaration
/// line. `public` = exported; everything else is not. Package-private (Java,
/// no modifier) defaults to non-exported Internal.
pub fn java_family_visibility(
    def_node: Option<tree_sitter::Node>,
    source: &str,
) -> (bool, Visibility) {
    let lead = leading_text(def_node, source, 2);
    if lead.contains("public ") {
        (true, Visibility::Public)
    } else if lead.contains("private ") {
        (false, Visibility::Private)
    } else if lead.contains("protected ") {
        (false, Visibility::Protected)
    } else if lead.contains("internal ") {
        (false, Visibility::Internal)
    } else if lead.contains("fileprivate ") || lead.contains("file ") {
        (false, Visibility::Private)
    } else {
        // No modifier: Java = package-private, Kotlin = public, Swift = internal.
        // Treat as non-exported (safe default for dead-code detection).
        (false, Visibility::Internal)
    }
}
