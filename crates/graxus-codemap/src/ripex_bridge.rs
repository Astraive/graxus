//! Bridge between the sibling `ripex` parser crate and graxus's fact model.
//!
//! `ripex` is a hand-written parser for 8 languages
//! (js/ts, python, go, rust, c, cpp, csharp). Its public facts
//! (`ParsedSymbol`, `ParsedImport`, `ParsedCall`, `ParsedVariable`) are
//! map onto graxus's normalized facts. This module performs that conversion
//! and retains each complete Ripex fact as a lossless JSON payload.
//!
//! Robustness contract: [`try_extract`] must **never panic** and must
//! **never crash** the surrounding index. Any failure (missing parser,
//! unsupported parser or unexpected panic is caught and reported as `Err` so the
//! caller (`CodemapBuilder::build`) can fall back to the tree-sitter
//! extractor.
//!
//! Parser-native payloads use the same ids as their normalized facts.

#![cfg(feature = "ripex")]

use anyhow::{anyhow, Context, Result};
use graxus_core::ScannedFile;

use crate::{
    CallFact, ConfidenceScore, ImportFact, ParserDiagnostic, ParserFact, ParserFactKind,
    ResolutionMethod, SymbolFact, VariableFact,
};
use crate::facts::{ImplKind, TypeImplFact};

/// Ripex output converted for the Graxus graph without discarding native data.
pub struct RipexExtraction {
    pub symbols: Vec<SymbolFact>,
    pub imports: Vec<ImportFact>,
    pub calls: Vec<CallFact>,
    pub variables: Vec<VariableFact>,
    pub type_impls: Vec<TypeImplFact>,
    pub parser_facts: Vec<ParserFact>,
    pub diagnostics: Vec<ParserDiagnostic>,
}

/// Whether the ripex crate can parse `graxus_lang` (the lowercase
/// [`graxus_core::Language`] string form).
///
/// Returns `false` for languages ripex does not cover (java, kotlin,
/// swift, markdown, etc.) so the caller keeps using tree-sitter.
pub fn ripex_supports(graxus_lang: &str) -> bool {
    matches!(
        graxus_lang,
        "rust" | "typescript" | "javascript" | "go" | "python" | "c" | "cpp" | "csharp"
    )
}
/// Run external compiler validation checks for a source file using Ripex's compiler runner.
pub fn run_compiler_check(
    path: &std::path::Path,
    trusted_project: bool,
) -> Result<ripex::compiler::CompilerCheckReport> {
    let options = ripex::compiler::CompilerCheckOptions {
        project: true,
        trusted_project,
        ..Default::default()
    };
    let lang = ripex::detect_language(path);
    ripex::compiler::check_with_compiler(path, lang, &options)
        .map_err(|e| anyhow!("compiler check failed: {e}"))
}

/// Extract facts for a single file using the ripex parser.
///
/// # Robustness
/// - Wrapped in [`std::panic::catch_unwind`] so a parser panic becomes a
///   clean `Err` instead of taking the whole index down.
/// - Parse errors collected by ripex are logged at `warn` level but do
///   **not** fail extraction (a partial parse is still useful).
/// - The returned `Err` signals the caller to fall back to tree-sitter.
pub fn try_extract(
    graxus_lang: &str,
    ext: &str,
    source: &str,
    scanned: &ScannedFile,
) -> Result<RipexExtraction> {
    if !ripex_supports(graxus_lang) {
        return Err(anyhow!("ripex has no parser for language {graxus_lang:?}"));
    }

    let rel = &scanned.relative_path;

    // A parser panic must not propagate into a repository-wide index.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Extension-aware selection is required for JSX/TSX and TS module modes.
        let parser = ripex::parser_for_ext(graxus_lang, ext)
            .ok_or_else(|| anyhow!("ripex returned no parser for {graxus_lang:?} ({ext})"))?;
        let parsed = parser.parse(source);
        let diagnostics = parsed
            .errors
            .iter()
            .map(|err| ParserDiagnostic {
                code: format!("{:?}", err.code),
                message: err.message.clone(),
                line: err.span.start.line,
                column: err.span.start.column,
            })
            .collect::<Vec<_>>();
        for err in &parsed.errors {
            tracing::warn!(
                "ripex parse warning in {rel}:{line}: {msg}",
                line = err.span.start.line,
                msg = err.message
            );
        }
        let facts = parser
            .extract_best_effort(&parsed)
            .unwrap_or_else(|_| parser.extract_unchecked(&parsed));
        Ok::<_, anyhow::Error>((facts, diagnostics))
    }));

    let (facts, diagnostics) = match outcome {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => return Err(e).context("ripex extraction failed"),
        Err(_) => return Err(anyhow!("ripex parser panicked on {rel:?}")),
    };

    let mut symbols = facts
        .symbols
        .into_iter()
        .map(|s| {
            let raw = serde_json::to_value(&s).context("serialize ripex symbol")?;
            let type_impls = convert_type_impls(&s, rel, graxus_lang);
            Ok((convert_symbol(s, rel, graxus_lang), raw, type_impls))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut imports = facts
        .imports
        .into_iter()
        .map(|im| {
            let raw = serde_json::to_value(&im).context("serialize ripex import")?;
            Ok((convert_import(im, rel, graxus_lang), raw))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut calls = facts
        .calls
        .into_iter()
        .map(|c| {
            let raw = serde_json::to_value(&c).context("serialize ripex call")?;
            Ok((convert_call(c, rel, graxus_lang), raw))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut variables = facts
        .variables
        .into_iter()
        .map(|v| {
            let raw = serde_json::to_value(&v).context("serialize ripex variable")?;
            Ok((convert_variable(v, rel, graxus_lang), raw))
        })
        .collect::<Result<Vec<_>>>()?;

    // Keep raw and normalized facts paired while establishing deterministic ids.
    symbols.sort_by(|a, b| {
        a.0.line_start
            .cmp(&b.0.line_start)
            .then(a.0.name.cmp(&b.0.name))
    });
    imports.sort_by(|a, b| a.0.line.cmp(&b.0.line));
    calls.sort_by(|a, b| a.0.line.cmp(&b.0.line).then(a.0.column.cmp(&b.0.column)));
    variables.sort_by(|a, b| a.0.line_def.cmp(&b.0.line_def));

    let mut parser_facts = Vec::with_capacity(
        symbols.len() + imports.len() + calls.len() + variables.len(),
    );
    let mut type_impls = Vec::new();
    let symbols = symbols
        .into_iter()
        .map(|(mut fact, data, symbol_type_impls)| {
            fact.id = format!("symbol:{rel}:{}", fact.name);
            type_impls.extend(symbol_type_impls);
            parser_facts.push(ParserFact {
                id: fact.id.clone(),
                kind: ParserFactKind::Symbol,
                data,
            });
            fact
        })
        .collect();
    let imports = imports
        .into_iter()
        .enumerate()
        .map(|(i, (mut fact, data))| {
            fact.id = format!("import:{rel}:{i}");
            parser_facts.push(ParserFact {
                id: fact.id.clone(),
                kind: ParserFactKind::Import,
                data,
            });
            fact
        })
        .collect();
    let calls = calls
        .into_iter()
        .enumerate()
        .map(|(i, (mut fact, data))| {
            fact.id = format!("call:{rel}:{}:{i}", fact.line);
            parser_facts.push(ParserFact {
                id: fact.id.clone(),
                kind: ParserFactKind::Call,
                data,
            });
            fact
        })
        .collect();
    let variables = variables
        .into_iter()
        .enumerate()
        .map(|(i, (mut fact, data))| {
            fact.id = format!("var:{rel}:{i}");
            parser_facts.push(ParserFact {
                id: fact.id.clone(),
                kind: ParserFactKind::Variable,
                data,
            });
            fact
        })
        .collect();

    Ok(RipexExtraction {
        symbols,
        imports,
        calls,
        variables,
        type_impls,
        parser_facts,
        diagnostics,
    })
}

fn convert_type_impls(
    symbol: &ripex::ParsedSymbol,
    rel: &str,
    lang: &str,
) -> Vec<TypeImplFact> {
    let kind = match lang {
        "cpp" => ImplKind::CppInheritance,
        "csharp" => ImplKind::CSharpInheritance,
        _ => ImplKind::Extends,
    };
    symbol
        .base_classes
        .iter()
        .enumerate()
        .map(|(i, base)| TypeImplFact {
            id: format!("type-impl:{rel}:{}:{base}:{i}", symbol.name),
            file: rel.to_string(),
            language: lang.to_string(),
            implementing_type: symbol.name.clone(),
            trait_or_interface: base.clone(),
            line: symbol.line_start,
            kind,
        })
        .collect()
}

fn convert_symbol(s: ripex::ParsedSymbol, rel: &str, lang: &str) -> SymbolFact {
    SymbolFact {
        id: String::new(),
        file: rel.to_string(),
        language: lang.to_string(),
        kind: to_graxus_symbol_kind(&s.kind),
        name: s.name,
        exported: s.exported,
        line_start: s.line_start,
        line_end: s.line_end,
        visibility: to_graxus_visibility(s.visibility),
        signature: s.signature,
        is_test: s.is_test,
        usage_count: 0,
        doc_string: s.doc_string,
        return_type: s.return_type,
        is_async: s.is_async,
        is_static: s.is_static,
        attributes: s.attributes,
    }
}

fn convert_import(i: ripex::ParsedImport, rel: &str, lang: &str) -> ImportFact {
    ImportFact {
        id: String::new(),
        file: rel.to_string(),
        language: lang.to_string(),
        kind: to_graxus_import_kind(&i.kind),
        source: i.source,
        local_name: i.local_name,
        imported_name: i.imported_name,
        resolved_file: None,
        line: i.line,
        confidence: ConfidenceScore::new(80.0, ResolutionMethod::SyntaxOnly),
    }
}

fn convert_call(c: ripex::ParsedCall, rel: &str, lang: &str) -> CallFact {
    CallFact {
        id: String::new(),
        file: rel.to_string(),
        language: lang.to_string(),
        kind: to_graxus_call_kind(&c.kind),
        caller_symbol: None,
        callee_text: c.callee_text,
        object: c.object,
        resolved_symbol: None,
        line: c.line,
        column: c.column,
        confidence: ConfidenceScore::new(80.0, ResolutionMethod::SyntaxOnly),
    }
}

fn convert_variable(v: ripex::ParsedVariable, rel: &str, lang: &str) -> VariableFact {
    VariableFact {
        id: String::new(),
        file: rel.to_string(),
        language: lang.to_string(),
        name: v.name,
        kind: to_graxus_var_kind(&v.kind),
        type_annotation: v.type_annotation,
        is_mutable: v.is_mutable,
        line_def: v.line_def,
        scope_symbol: v.scope_symbol,
        scope_start: v.scope_start,
        scope_end: v.scope_end,
        usage_sites: v
            .usage_sites
            .into_iter()
            .map(|u| crate::UsageSite {
                line: u.line,
                column: u.column,
                usage_kind: to_graxus_usage_kind(&u.usage_kind),
            })
            .collect(),
    }
}

// ── Enum adapters ──────────────────────────────────────────────────────
//
// ripex and graxus both serialize these enums with `rename_all = "snake_case"`
// (graxus `Visibility` uses `lowercase`, which is identical for these tokens).
// graxus's variants are a subset of ripex's. We round-trip through a JSON
// string; unknown ripex variants fall back to a sensible default so a future
// ripex addition can never break graxus extraction.

fn json_roundtrip<T: serde::de::DeserializeOwned>(value: &impl serde::Serialize, default: T) -> T {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => {
            serde_json::from_value(serde_json::Value::String(s)).unwrap_or(default)
        }
        _ => default,
    }
}

fn to_graxus_symbol_kind(v: &ripex::SymbolKind) -> crate::SymbolKind {
    json_roundtrip(v, crate::SymbolKind::Variable)
}

fn to_graxus_import_kind(v: &ripex::ImportKind) -> crate::ImportKind {
    json_roundtrip(v, crate::ImportKind::NamedImport)
}

fn to_graxus_call_kind(v: &ripex::CallKind) -> crate::CallKind {
    json_roundtrip(v, crate::CallKind::FunctionCall)
}

fn to_graxus_var_kind(v: &ripex::VarKind) -> crate::VarKind {
    json_roundtrip(v, crate::VarKind::Let)
}

fn to_graxus_usage_kind(v: &ripex::UsageKind) -> crate::UsageKind {
    json_roundtrip(v, crate::UsageKind::Read)
}

fn to_graxus_visibility(v: ripex::Visibility) -> crate::Visibility {
    json_roundtrip(&v, crate::Visibility::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graxus_core::{FileKind, Language, ScannedFile};

    fn sample(lang: Language, rel: &str) -> ScannedFile {
        ScannedFile {
            path: std::path::PathBuf::from(rel),
            relative_path: rel.to_string(),
            kind: FileKind::Code,
            language: lang,
            hash: String::new(),
            size: 0,
            modified: chrono::Utc::now(),
        }
    }

    #[test]
    fn rust_symbols_extract() {
        let f = sample(Language::Rust, "a.rs");
        let extraction =
            try_extract("rust", "rs", "pub fn main() {}\nstruct Foo { x: i32 }\n", &f).unwrap();
        assert!(!extraction.symbols.is_empty(), "expected at least one symbol");
        assert_eq!(extraction.symbols[0].language, "rust");
        assert!(
            !extraction.symbols[0].id.is_empty(),
            "ids are assigned by the bridge"
        );
    }

    #[test]
    fn python_imports_extract() {
        let f = sample(Language::Python, "b.py");
        let extraction =
            try_extract("python", "py", "from os import path\nimport sys\n", &f).unwrap();
        assert!(!extraction.imports.is_empty());
    }

    #[test]
    fn typescript_syntax_parsed_in_ts_mode() {
        let f = sample(Language::TypeScript, "c.ts");
        // `with_typescript()` parses TS declarations (function/const with type
        // annotations) that the plain-JS parser would reject.
        let src = "export const x: number = 1;\nfunction foo(a: string): void {}\n";
        let extraction = try_extract("typescript", "ts", src, &f).unwrap();
        assert!(
            !extraction.symbols.is_empty(),
            "TS const/function should parse in TS mode"
        );
    }

    #[test]
    fn preserves_rich_ripex_facts_and_kinds() {
        let f = sample(Language::TypeScript, "rich.ts");
        let src = r#"
export { User as Person } from "./types";
export async function load() { return fetchUser(); }
class Admin extends User {
    constructor() {}
    get name() { return "admin"; }
}
"#;
        let extraction = try_extract("typescript", "ts", src, &f).unwrap();

        assert!(extraction
            .imports
            .iter()
            .any(|fact| fact.kind == crate::ImportKind::ReExport));
        assert!(extraction
            .symbols
            .iter()
            .any(|fact| fact.kind == crate::SymbolKind::Constructor));
        assert_eq!(
            to_graxus_symbol_kind(&ripex::SymbolKind::Getter),
            crate::SymbolKind::Getter
        );

        let load = extraction
            .parser_facts
            .iter()
            .find(|fact| fact.data["name"] == "load")
            .expect("raw load symbol");
        assert_eq!(load.data["is_async"], true);

        let admin = extraction
            .parser_facts
            .iter()
            .find(|fact| fact.data["name"] == "Admin")
            .expect("raw Admin symbol");
        assert_eq!(admin.data["base_classes"], serde_json::json!(["User"]));
        assert!(extraction.type_impls.iter().any(|fact| {
            fact.implementing_type == "Admin" && fact.trait_or_interface == "User"
        }));
    }

    #[test]
    fn tsx_uses_extension_aware_ripex_parser() {
        let f = sample(Language::TypeScript, "view.tsx");
        let extraction = try_extract(
            "typescript",
            "tsx",
            "export const view = <section><span>Hello</span></section>;",
            &f,
        )
        .unwrap();

        assert!(extraction.diagnostics.is_empty());
        assert!(!extraction.parser_facts.is_empty());
    }

    #[test]
    fn unsupported_language_errors_for_fallback() {
        let f = sample(Language::Java, "d.java");
        assert!(try_extract("java", "java", "class A {}", &f).is_err());
    }

    #[test]
    fn empty_source_is_ok() {
        let f = sample(Language::Rust, "e.rs");
        // empty/whitespace-only files yield no facts but should not error
        // (we only error on non-empty-with-zero-facts to trigger fallback).
        assert!(try_extract("rust", "rs", "   \n", &f).is_ok());
    }
}
