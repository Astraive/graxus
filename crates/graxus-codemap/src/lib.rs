//! Graxus codemap — Source-code structure, symbols, imports, relationships.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use tracing::instrument;

use crate::facts::{DIFact, RouteFact, TypeImplFact};
use graxus_core::ScannedFile;

pub mod extractor;
pub mod facts;
pub mod frameworks;
pub mod graph;
pub mod languages;
pub mod queries;
pub mod resolver;

/// Bridge to the sibling `ripex` parser crate (only compiled when the
/// `ripex` feature is enabled; it is on by default).
#[cfg(feature = "ripex")]
pub mod ripex_bridge;

// ── Enums ──────────────────────────────────────────────────────────────────

/// Numeric confidence score (0.0 to 100.0).
///
/// Custom deserializer handles both old string format ("high", "low") and new object format.
///
/// # Limitations
/// Contains an `f64` field, which prevents deriving `Eq` — this type cannot be used
/// as a key in `HashSet` or `BTreeSet`. The `new()` constructor clamps the score to
/// `[0.0, 100.0]`, preventing `NaN` values that would break `PartialEq` reflexivity.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConfidenceScore {
    pub score: f64,
    pub label: ConfidenceLabel,
    pub method: ResolutionMethod,
}

impl<'de> Deserialize<'de> for ConfidenceScore {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de;

        struct ConfidenceVisitor;

        impl<'de> de::Visitor<'de> for ConfidenceVisitor {
            type Value = ConfidenceScore;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string (\"high\", \"medium\", \"low\", \"unknown\") or object {score, label, method}")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<ConfidenceScore, E> {
                Ok(match v.to_lowercase().as_str() {
                    "high" => ConfidenceScore::new(85.0, ResolutionMethod::PathMatchOnly),
                    "medium" => ConfidenceScore::new(65.0, ResolutionMethod::PathMatchOnly),
                    "low" => ConfidenceScore::new(40.0, ResolutionMethod::FuzzySymbolMatch),
                    _ => ConfidenceScore::unresolved(),
                })
            }

            fn visit_map<M: de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<ConfidenceScore, M::Error> {
                let mut score: Option<f64> = None;
                let mut label: Option<ConfidenceLabel> = None;
                let mut method: Option<ResolutionMethod> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "score" => score = Some(map.next_value()?),
                        "label" => label = Some(map.next_value()?),
                        "method" => method = Some(map.next_value()?),
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }
                let s = score.unwrap_or(0.0);
                match (label, method) {
                    (Some(l), Some(m)) => Ok(ConfidenceScore {
                        score: s.clamp(0.0, 100.0),
                        label: l,
                        method: m,
                    }),
                    (Some(l), None) => Ok(ConfidenceScore {
                        score: s.clamp(0.0, 100.0),
                        label: l,
                        method: ResolutionMethod::Unresolved,
                    }),
                    (None, Some(m)) => {
                        let label = match s as u32 {
                            95..=100 => ConfidenceLabel::Exact,
                            80..=94 => ConfidenceLabel::High,
                            60..=79 => ConfidenceLabel::Medium,
                            35..=59 => ConfidenceLabel::Low,
                            1..=34 => ConfidenceLabel::Weak,
                            _ => ConfidenceLabel::Unresolved,
                        };
                        Ok(ConfidenceScore {
                            score: s.clamp(0.0, 100.0),
                            label,
                            method: m,
                        })
                    }
                    (None, None) => Ok(ConfidenceScore::new(s, ResolutionMethod::Unresolved)),
                }
            }
        }

        deserializer.deserialize_any(ConfidenceVisitor)
    }
}

/// Label for a confidence score range.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfidenceLabel {
    /// 95-100% confidence.
    Exact,
    /// 80-94% confidence.
    High,
    /// 60-79% confidence.
    Medium,
    /// 35-59% confidence.
    Low,
    /// 1-34% confidence.
    Weak,
    /// 0% — unresolved.
    Unresolved,
}

/// Method used to resolve a symbol or import reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResolutionMethod {
    /// Symbol defined in the same file.
    LocalDefinition,
    /// Named import matched to an exact export.
    NamedImportExactExport,
    /// Default import matched to an exact export.
    DefaultImportExactExport,
    /// Namespace import member access.
    NamespaceImportMember,
    /// Rust `use` path resolved exactly.
    RustUseExactPath,
    /// Go package selector resolved.
    GoPackageSelector,
    /// Python `from X import Y` resolved.
    PythonFromImport,
    /// Python module alias resolved.
    PythonModuleAlias,
    /// Symbol found in the same file.
    SameFileSymbol,
    /// Symbol found in a module's public API.
    ModulePublicSymbol,
    /// Matched by file path only.
    PathMatchOnly,
    /// Matched by name within the same project.
    NameMatchSameProject,
    /// Fuzzy symbol name match.
    FuzzySymbolMatch,
    /// Resolved to an external dependency.
    ExternalDependency,
    /// Derived from syntax only (no resolution).
    SyntaxOnly,
    /// Could not resolve.
    Unresolved,
}

impl ConfidenceScore {
    /// Create a new confidence score with the given score and resolution method.
    pub fn new(score: f64, method: ResolutionMethod) -> Self {
        let label = match score as u32 {
            95..=100 => ConfidenceLabel::Exact,
            80..=94 => ConfidenceLabel::High,
            60..=79 => ConfidenceLabel::Medium,
            35..=59 => ConfidenceLabel::Low,
            1..=34 => ConfidenceLabel::Weak,
            _ => ConfidenceLabel::Unresolved,
        };
        Self {
            score: score.clamp(0.0, 100.0),
            label,
            method,
        }
    }

    /// Create an unresolved confidence score (0%).
    pub fn unresolved() -> Self {
        Self {
            score: 0.0,
            label: ConfidenceLabel::Unresolved,
            method: ResolutionMethod::Unresolved,
        }
    }

    /// High confidence for a local definition match.
    pub fn local_definition() -> Self {
        Self::new(98.0, ResolutionMethod::LocalDefinition)
    }
    /// High confidence for a named import exact export match.
    pub fn named_import_exact() -> Self {
        Self::new(95.0, ResolutionMethod::NamedImportExactExport)
    }
    /// Good confidence for a default import exact export match.
    pub fn default_import_exact() -> Self {
        Self::new(88.0, ResolutionMethod::DefaultImportExactExport)
    }
    /// High confidence for a namespace import member match.
    pub fn namespace_import() -> Self {
        Self::new(93.0, ResolutionMethod::NamespaceImportMember)
    }
    /// High confidence for a Rust `use` path match.
    pub fn rust_use_path() -> Self {
        Self::new(95.0, ResolutionMethod::RustUseExactPath)
    }
    /// Good confidence for a Go package selector match.
    pub fn go_package() -> Self {
        Self::new(88.0, ResolutionMethod::GoPackageSelector)
    }
    /// Good confidence for a Python `from` import match.
    pub fn python_from() -> Self {
        Self::new(85.0, ResolutionMethod::PythonFromImport)
    }
    /// Low confidence for a same-project name match.
    pub fn same_project() -> Self {
        Self::new(40.0, ResolutionMethod::NameMatchSameProject)
    }
    /// Medium confidence for a path-only match.
    pub fn path_match_only() -> Self {
        Self::new(60.0, ResolutionMethod::PathMatchOnly)
    }
    /// Minimal confidence from syntax-only analysis.
    pub fn syntax_only() -> Self {
        Self::new(10.0, ResolutionMethod::SyntaxOnly)
    }

    /// Returns true if this score represents a resolved reference.
    pub fn is_resolved(&self) -> bool {
        self.score > 0.0
    }
}

impl Default for ConfidenceScore {
    fn default() -> Self {
        Self::unresolved()
    }
}

impl std::fmt::Display for ConfidenceScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label_str = match self.label {
            ConfidenceLabel::Exact => "exact",
            ConfidenceLabel::High => "high",
            ConfidenceLabel::Medium => "medium",
            ConfidenceLabel::Low => "low",
            ConfidenceLabel::Weak => "weak",
            ConfidenceLabel::Unresolved => "unresolved",
        };
        write!(f, "{}% ({})", self.score, label_str)
    }
}

/// Backward-compatible confidence enum. Maps to ConfidenceScore internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}

impl From<Confidence> for ConfidenceScore {
    fn from(c: Confidence) -> Self {
        match c {
            Confidence::High => ConfidenceScore::new(85.0, ResolutionMethod::PathMatchOnly),
            Confidence::Medium => ConfidenceScore::new(65.0, ResolutionMethod::PathMatchOnly),
            Confidence::Low => ConfidenceScore::new(40.0, ResolutionMethod::FuzzySymbolMatch),
            Confidence::Unknown => ConfidenceScore::unresolved(),
        }
    }
}

/// Classification of import statement types across languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    /// `import { name } from "source"`.
    NamedImport,
    /// `import * as ns from "source"`.
    NamespaceImport,
    /// `import name from "source"`.
    DefaultImport,
    /// `import "source"` (no bindings).
    SideEffectImport,
    /// `use crate::module::name`.
    RustUse,
    /// `import "package"`.
    GoImport,
    /// `from module import name`.
    FromImport,
    /// `import module`.
    PythonImport,
    /// `import com.app.models.User`.
    JavaImport,
    /// `import com.app.models.User`.
    KotlinImport,
    /// `import Foundation`.
    SwiftImport,
    /// Re-export from another module.
    ReExport,
    /// Type-only import.
    TypeImport,
    /// Type-only re-export.
    TypeReExport,
}

/// Classification of code symbols (functions, types, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    /// A function or procedure.
    Function,
    /// A class definition.
    Class,
    /// A struct definition.
    Struct,
    /// A trait definition.
    Trait,
    /// An interface definition.
    Interface,
    /// A method on a class/struct.
    Method,
    /// A module or namespace.
    Module,
    /// A constant value.
    Constant,
    /// An enum definition.
    Enum,
    /// A type alias.
    Type,
    /// A variable binding.
    Variable,
    /// A constructor.
    Constructor,
    /// A destructor.
    Destructor,
    /// A property getter.
    Getter,
    /// A property setter.
    Setter,
    /// A class or object property.
    Property,
    /// An event declaration.
    Event,
    /// A delegate declaration.
    Delegate,
    /// A namespace declaration.
    Namespace,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Function => write!(f, "function"),
            SymbolKind::Class => write!(f, "class"),
            SymbolKind::Struct => write!(f, "struct"),
            SymbolKind::Trait => write!(f, "trait"),
            SymbolKind::Interface => write!(f, "interface"),
            SymbolKind::Method => write!(f, "method"),
            SymbolKind::Module => write!(f, "module"),
            SymbolKind::Constant => write!(f, "constant"),
            SymbolKind::Enum => write!(f, "enum"),
            SymbolKind::Type => write!(f, "type"),
            SymbolKind::Variable => write!(f, "variable"),
            SymbolKind::Constructor => write!(f, "constructor"),
            SymbolKind::Destructor => write!(f, "destructor"),
            SymbolKind::Getter => write!(f, "getter"),
            SymbolKind::Setter => write!(f, "setter"),
            SymbolKind::Property => write!(f, "property"),
            SymbolKind::Event => write!(f, "event"),
            SymbolKind::Delegate => write!(f, "delegate"),
            SymbolKind::Namespace => write!(f, "namespace"),
        }
    }
}

/// Visibility level of a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Publicly accessible.
    Public,
    /// Private to the enclosing scope.
    Private,
    /// Accessible to subclasses/derived types.
    Protected,
    /// Accessible within the same package/crate.
    Internal,
    /// Visibility could not be determined.
    Unknown,
}

/// Classification of function/method call types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallKind {
    /// A direct function call: `foo()`.
    FunctionCall,
    /// A method call on an object: `obj.method()`.
    MethodCall,
    /// A constructor call: `new Foo()`.
    ConstructorCall,
    /// A path-qualified call: `module::func()`.
    PathCall,
    /// A selector call (Go): `obj.Method()`.
    SelectorCall,
    /// An explicit destructor call.
    DestructorCall,
    /// A call to a base/super implementation.
    SuperCall,
    /// A delegate invocation.
    DelegateCall,
}

/// Type of relationship edge in the code graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CodeEdgeType {
    /// A file contains a symbol.
    Contains,
    /// A file imports from another file.
    Imports,
    /// A file exports to another file.
    Exports,
    /// A symbol calls another symbol.
    Calls,
    /// A type implements an interface/trait.
    Implements,
    /// A type extends another type.
    Extends,
    /// A general reference between nodes.
    References,
    /// A symbol is defined in a file.
    DefinedIn,
}

// ── Data types ─────────────────────────────────────────────────────────────

/// A single import statement extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportFact {
    /// Unique identifier for this import.
    pub id: String,
    /// Relative file path where this import appears.
    pub file: String,
    /// Source language identifier (e.g. "rust", "typescript").
    pub language: String,
    /// The kind of import statement.
    pub kind: ImportKind,
    /// The import source path string (e.g. "std::collections::HashMap").
    pub source: String,
    /// Local binding name (e.g. "HashMap" for `use std::collections::HashMap`).
    pub local_name: Option<String>,
    /// The specific name that was imported (for named imports).
    pub imported_name: Option<String>,
    /// Resolved file path after import resolution.
    pub resolved_file: Option<String>,
    /// Line number where the import appears (1-based).
    pub line: usize,
    /// Confidence score for the resolution.
    pub confidence: ConfidenceScore,
}

/// A code symbol (function, class, struct, etc.) extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolFact {
    /// Unique identifier for this symbol.
    pub id: String,
    /// Relative file path where this symbol is defined.
    pub file: String,
    /// Source language identifier.
    pub language: String,
    /// The kind of symbol.
    pub kind: SymbolKind,
    /// The symbol's name.
    pub name: String,
    /// Whether this symbol is exported (public API).
    pub exported: bool,
    /// Starting line number (1-based).
    pub line_start: usize,
    /// Ending line number (1-based).
    pub line_end: usize,
    /// Visibility level.
    pub visibility: Visibility,
    /// Full function/method signature text (params + return type), empty if not applicable.
    #[serde(default)]
    pub signature: String,
    /// Whether this symbol is a test function.
    #[serde(default)]
    pub is_test: bool,
    /// How many times this symbol is called by other code (computed post-extraction).
    #[serde(default)]
    pub usage_count: usize,
    /// Doc comment text associated with this symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_string: Option<String>,
    /// Explicit return type annotation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
    /// Whether this symbol is an async function or method.
    #[serde(default)]
    pub is_async: bool,
    /// Whether this symbol is static.
    #[serde(default)]
    pub is_static: bool,
    /// Annotations/decorators attached to this symbol.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<String>,
}
impl Default for SymbolFact {
    fn default() -> Self {
        Self {
            id: String::new(),
            file: String::new(),
            language: String::new(),
            kind: SymbolKind::Function,
            name: String::new(),
            exported: false,
            line_start: 0,
            line_end: 0,
            visibility: Visibility::Unknown,
            signature: String::new(),
            is_test: false,
            usage_count: 0,
            doc_string: None,
            return_type: None,
            is_async: false,
            is_static: false,
            attributes: Vec::new(),
        }
    }
}

/// A function/method call site extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallFact {
    /// Unique identifier for this call.
    pub id: String,
    /// Relative file path where this call appears.
    pub file: String,
    /// Source language identifier.
    pub language: String,
    /// The kind of call.
    pub kind: CallKind,
    /// The enclosing function/method that contains this call.
    pub caller_symbol: Option<String>,
    /// The raw text of the callee (function name or path).
    pub callee_text: String,
    /// For method calls, the object being called on.
    pub object: Option<String>,
    /// Resolved symbol key in "file::name" format.
    pub resolved_symbol: Option<String>,
    /// Line number of the call (1-based).
    pub line: usize,
    /// Column number of the call (0-based).
    pub column: usize,
    /// Confidence score for the resolution.
    pub confidence: ConfidenceScore,
}

/// A directed edge in the code graph representing a relationship between nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeEdge {
    /// Source node identifier.
    pub from: String,
    /// Target node identifier.
    pub to: String,
    /// The type of relationship.
    pub edge_type: CodeEdgeType,
}

/// A file node in the code graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    /// Relative file path.
    pub path: String,
    /// Source language identifier.
    pub language: String,
    /// Content hash for change detection.
    pub hash: String,
    /// File size in bytes.
    pub size: u64,
}

/// Parser-level fact category for the lossless source payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserFactKind {
    Symbol,
    Import,
    Call,
    Variable,
}

/// A complete parser-native fact associated with its normalized Graxus fact id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserFact {
    pub id: String,
    pub kind: ParserFactKind,
    pub data: serde_json::Value,
}

/// A parser diagnostic retained for code-intelligence clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserDiagnostic {
    pub code: String,
    pub message: String,
    pub line: usize,
    pub column: usize,
}

/// Backend provenance and parser-native facts for one source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileParserResult {
    pub file: String,
    pub requested_backend: graxus_core::ParserBackend,
    pub used_backend: graxus_core::ParserBackend,
    #[serde(default)]
    pub fallback_reason: Option<String>,
    #[serde(default)]
    pub diagnostics: Vec<ParserDiagnostic>,
    /// Lossless Ripex facts. Tree-sitter fallback facts remain available through
    /// the normalized graph collections.
    #[serde(default)]
    pub facts: Vec<ParserFact>,
}

impl Default for FileParserResult {
    fn default() -> Self {
        Self {
            file: String::new(),
            requested_backend: graxus_core::ParserBackend::Ripex,
            used_backend: graxus_core::ParserBackend::TreeSitter,
            fallback_reason: Some("parser provenance unavailable in legacy data".to_string()),
            diagnostics: Vec::new(),
            facts: Vec::new(),
        }
    }
}

/// A single file's analysis (raw facts before resolution).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysis {
    /// Relative file path.
    pub file: String,
    /// Source language identifier.
    pub language: String,
    /// Extracted imports.
    pub imports: Vec<ImportFact>,
    /// Extracted symbols.
    pub symbols: Vec<SymbolFact>,
    /// Extracted calls.
    pub calls: Vec<CallFact>,
    /// Extracted framework route facts.
    #[serde(default)]
    pub routes: Vec<RouteFact>,
    /// Extracted type implementation facts.
    #[serde(default)]
    pub type_impls: Vec<TypeImplFact>,
    /// Extracted dependency injection bindings.
    #[serde(default)]
    pub di_bindings: Vec<DIFact>,
    /// Extracted variable bindings.
    #[serde(default)]
    pub variables: Vec<VariableFact>,
    /// Parser backend and native facts used for this file.
    #[serde(default)]
    pub parser: FileParserResult,
}

/// Type hint from constructor calls (e.g., `const x = new Foo()` means `x: Foo`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeHint {
    /// File where the hint was found.
    pub file: String,
    /// Variable name.
    pub variable: String,
    /// Inferred type name.
    pub type_name: String,
    /// Line number (1-based).
    pub line: usize,
}

/// Classification of variable binding kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VarKind {
    /// `let` binding (Rust/JS/TS).
    Let,
    /// `const` binding.
    Const,
    /// `var` declaration (JS/TS).
    Var,
    /// Function/method parameter.
    Parameter,
    /// `for` loop iteration variable.
    ForLoop,
    /// Destructured binding.
    Pattern,
    Static,
    Global,
    ThreadLocal,
    Extern,
    Register,
    Auto,
    Field,
    Property,
    EnumMember,
}

/// How a variable is used at a specific site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageKind {
    Read,
    Write,
    Move,
    Borrow,
    BorrowMut,
    PassedAsArg,
}

/// A single usage site of a variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSite {
    pub line: usize,
    pub column: usize,
    pub usage_kind: UsageKind,
}

/// A variable binding extracted from source code, with scope and usage tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableFact {
    pub id: String,
    pub file: String,
    pub language: String,
    pub name: String,
    pub kind: VarKind,
    #[serde(default)]
    pub type_annotation: Option<String>,
    pub is_mutable: bool,
    pub line_def: usize,
    /// Name of the enclosing function/method, if any.
    #[serde(default)]
    pub scope_symbol: Option<String>,
    pub scope_start: usize,
    pub scope_end: usize,
    #[serde(default)]
    pub usage_sites: Vec<UsageSite>,
}

/// Complete code graph containing all extracted and resolved facts.
///
/// Provides both linear-scan methods and cached index-based methods for lookups.
/// The cached indexes are built lazily on first use.
pub struct CodeGraph {
    /// All files in the codebase.
    pub files: Vec<FileNode>,
    /// All extracted symbols.
    pub symbols: Vec<SymbolFact>,
    /// All extracted imports.
    pub imports: Vec<ImportFact>,
    /// All extracted call sites.
    pub calls: Vec<CallFact>,
    /// All extracted HTTP route facts.
    pub routes: Vec<RouteFact>,
    /// All extracted trait/interface implementation facts.
    pub type_impls: Vec<TypeImplFact>,
    /// All extracted dependency injection bindings.
    pub di_bindings: Vec<DIFact>,
    /// All relationship edges.
    pub edges: Vec<CodeEdge>,
    /// Type hints from constructor calls.
    pub type_hints: Vec<TypeHint>,
    /// All extracted variable bindings with scope and usage information.
    pub variables: Vec<VariableFact>,
    /// All extracted decorators/attributes/annotations.
    pub decorators: Vec<crate::extractor::decorators::DecoratorFact>,
    /// All extracted macro definitions.
    pub macros: Vec<crate::extractor::macros::MacroFact>,
    /// Per-file parser provenance and lossless Ripex fact payloads.
    pub parser_results: Vec<FileParserResult>,
    /// Lazily-built indexes for O(1) lookups. Skipped during serialization.
    pub indexes: OnceLock<CodeGraphIndexes>,
}

impl Clone for CodeGraph {
    fn clone(&self) -> Self {
        Self {
            files: self.files.clone(),
            symbols: self.symbols.clone(),
            imports: self.imports.clone(),
            calls: self.calls.clone(),
            routes: self.routes.clone(),
            type_impls: self.type_impls.clone(),
            di_bindings: self.di_bindings.clone(),
            edges: self.edges.clone(),
            type_hints: self.type_hints.clone(),
            variables: self.variables.clone(),
            decorators: self.decorators.clone(),
            macros: self.macros.clone(),
            parser_results: self.parser_results.clone(),
            indexes: OnceLock::new(),
        }
    }
}

impl std::fmt::Debug for CodeGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeGraph")
            .field("files", &self.files.len())
            .field("symbols", &self.symbols.len())
            .field("imports", &self.imports.len())
            .field("calls", &self.calls.len())
            .field("routes", &self.routes.len())
            .field("type_impls", &self.type_impls.len())
            .field("di_bindings", &self.di_bindings.len())
            .field("edges", &self.edges.len())
            .field("type_hints", &self.type_hints.len())
            .field("variables", &self.variables.len())
            .field("decorators", &self.decorators.len())
            .field("macros", &self.macros.len())
            .field("parser_results", &self.parser_results.len())
            .finish()
    }
}

impl Default for CodeGraph {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            symbols: Vec::new(),
            imports: Vec::new(),
            calls: Vec::new(),
            routes: Vec::new(),
            type_impls: Vec::new(),
            di_bindings: Vec::new(),
            edges: Vec::new(),
            type_hints: Vec::new(),
            variables: Vec::new(),
            decorators: Vec::new(),
            macros: Vec::new(),
            parser_results: Vec::new(),
            indexes: OnceLock::new(),
        }
    }
}

impl Serialize for CodeGraph {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("CodeGraph", 13)?;
        state.serialize_field("files", &self.files)?;
        state.serialize_field("symbols", &self.symbols)?;
        state.serialize_field("imports", &self.imports)?;
        state.serialize_field("calls", &self.calls)?;
        state.serialize_field("routes", &self.routes)?;
        state.serialize_field("type_impls", &self.type_impls)?;
        state.serialize_field("di_bindings", &self.di_bindings)?;
        state.serialize_field("edges", &self.edges)?;
        state.serialize_field("type_hints", &self.type_hints)?;
        state.serialize_field("variables", &self.variables)?;
        state.serialize_field("decorators", &self.decorators)?;
        state.serialize_field("macros", &self.macros)?;
        state.serialize_field("parser_results", &self.parser_results)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for CodeGraph {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct CodeGraphHelper {
            files: Vec<FileNode>,
            symbols: Vec<SymbolFact>,
            imports: Vec<ImportFact>,
            calls: Vec<CallFact>,
            #[serde(default)]
            routes: Vec<RouteFact>,
            #[serde(default)]
            type_impls: Vec<TypeImplFact>,
            #[serde(default)]
            di_bindings: Vec<DIFact>,
            edges: Vec<CodeEdge>,
            #[serde(default)]
            type_hints: Vec<TypeHint>,
            #[serde(default)]
            variables: Vec<VariableFact>,
            #[serde(default)]
            decorators: Vec<crate::extractor::decorators::DecoratorFact>,
            #[serde(default)]
            macros: Vec<crate::extractor::macros::MacroFact>,
            #[serde(default)]
            parser_results: Vec<FileParserResult>,
        }
        let helper = CodeGraphHelper::deserialize(deserializer)?;
        Ok(CodeGraph {
            files: helper.files,
            symbols: helper.symbols,
            imports: helper.imports,
            calls: helper.calls,
            routes: helper.routes,
            type_impls: helper.type_impls,
            di_bindings: helper.di_bindings,
            edges: helper.edges,
            type_hints: helper.type_hints,
            variables: helper.variables,
            decorators: helper.decorators,
            macros: helper.macros,
            parser_results: helper.parser_results,
            indexes: OnceLock::new(),
        })
    }
}

impl CodeGraph {
    /// Create an empty code graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a code graph from individual parts. Indexes are built lazily on first use.
    pub fn from_parts(
        files: Vec<FileNode>,
        symbols: Vec<SymbolFact>,
        imports: Vec<ImportFact>,
        calls: Vec<CallFact>,
        routes: Vec<RouteFact>,
        type_impls: Vec<TypeImplFact>,
        di_bindings: Vec<DIFact>,
        edges: Vec<CodeEdge>,
        type_hints: Vec<TypeHint>,
        variables: Vec<VariableFact>,
    ) -> Self {
        Self {
            files,
            symbols,
            imports,
            calls,
            routes,
            type_impls,
            di_bindings,
            edges,
            type_hints,
            variables,
            decorators: Vec::new(),
            macros: Vec::new(),
            parser_results: Vec::new(),
            indexes: OnceLock::new(),
        }
    }

    /// Build HashMap indexes for O(1) lookups. Call after constructing CodeGraph.
    pub fn build_indexes(&self) -> CodeGraphIndexes {
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, s) in self.symbols.iter().enumerate() {
            by_name.entry(s.name.clone()).or_default().push(i);
        }
        let mut symbols_by_file: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, s) in self.symbols.iter().enumerate() {
            symbols_by_file.entry(s.file.clone()).or_default().push(i);
        }
        let mut imports_by_file: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, imp) in self.imports.iter().enumerate() {
            imports_by_file.entry(imp.file.clone()).or_default().push(i);
        }
        let mut calls_by_file: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, c) in self.calls.iter().enumerate() {
            calls_by_file.entry(c.file.clone()).or_default().push(i);
        }
        let file_set: std::collections::HashSet<String> =
            self.files.iter().map(|f| f.path.clone()).collect();
        CodeGraphIndexes {
            by_name,
            symbols_by_file,
            imports_by_file,
            calls_by_file,
            file_set,
        }
    }

    /// Get or lazily build the cached indexes.
    fn get_indexes(&self) -> &CodeGraphIndexes {
        self.indexes.get_or_init(|| self.build_indexes())
    }

    /// Find the first symbol with the given name. Uses cached index for O(1) lookup.
    pub fn find_symbol(&self, name: &str) -> Option<&SymbolFact> {
        self.get_indexes().find_symbol(self, name)
    }

    /// Find all symbols with the given name. Uses cached index for O(1) lookup.
    pub fn find_symbols(&self, name: &str) -> Vec<&SymbolFact> {
        self.get_indexes().symbols_by_name(self, name)
    }

    /// Get all symbols defined in the given file. Uses cached index for O(1) lookup.
    pub fn symbols_in_file(&self, path: &str) -> Vec<&SymbolFact> {
        self.get_indexes().symbols_in_file(self, path)
    }

    /// Get all imports in the given file. Uses cached index for O(1) lookup.
    pub fn imports_in_file(&self, path: &str) -> Vec<&ImportFact> {
        self.get_indexes().imports_in_file(self, path)
    }

    /// Get all call sites in the given file. Uses cached index for O(1) lookup.
    pub fn calls_in_file(&self, path: &str) -> Vec<&CallFact> {
        self.get_indexes().calls_in_file(self, path)
    }

    /// Return parser provenance and native facts for a file.
    pub fn parser_result_for_file(&self, path: &str) -> Option<&FileParserResult> {
        self.parser_results
            .iter()
            .find(|result| result.file == path)
    }

    /// Find a lossless parser-native fact by its normalized fact id.
    pub fn parser_fact(&self, fact_id: &str) -> Option<&ParserFact> {
        self.parser_results
            .iter()
            .flat_map(|result| &result.facts)
            .find(|fact| fact.id == fact_id)
    }

    /// Find all calls that target the given symbol ID.
    pub fn calls_to_symbol(&self, symbol_id: &str) -> Vec<&CallFact> {
        self.calls
            .iter()
            .filter(|c| c.resolved_symbol.as_deref() == Some(symbol_id))
            .collect()
    }

    /// Check if the graph contains the given file path.
    pub fn has_file(&self, path: &str) -> bool {
        self.get_indexes().has_file(path)
    }

    /// Get all file paths in the graph.
    pub fn file_paths(&self) -> Vec<&str> {
        self.files.iter().map(|f| f.path.as_str()).collect()
    }

    /// Remove all data (file node, symbols, imports, calls) for the given file path.
    pub fn remove_file(&mut self, path: &str) {
        self.files.retain(|f| f.path != path);
        self.symbols.retain(|s| s.file != path);
        self.imports.retain(|i| i.file != path);
        self.calls.retain(|c| c.file != path);
        self.routes.retain(|r| r.file != path);
        self.type_impls.retain(|t| t.file != path);
        self.di_bindings.retain(|d| d.file != path);
        self.type_hints.retain(|h| h.file != path);
        self.variables.retain(|v| v.file != path);
        self.decorators.retain(|d| d.file != path);
        self.macros.retain(|m| m.file != path);
        self.parser_results.retain(|p| p.file != path);
        // Rebuild indexes after removal
        self.indexes = OnceLock::new();
    }

    /// Merge another CodeGraph into this one.
    ///
    /// Files in `other` that already exist in `self` will have their data
    /// replaced (effectively an update). New files are appended.
    pub fn merge(&mut self, other: CodeGraph) {
        // Remove existing data for files that are being updated
        let other_file_paths: std::collections::HashSet<&str> =
            other.files.iter().map(|f| f.path.as_str()).collect();
        for path in &other_file_paths {
            self.remove_file(path);
        }

        // Append all data from other
        self.files.extend(other.files);
        self.symbols.extend(other.symbols);
        self.imports.extend(other.imports);
        self.calls.extend(other.calls);
        self.routes.extend(other.routes);
        self.type_impls.extend(other.type_impls);
        self.di_bindings.extend(other.di_bindings);
        self.edges.extend(other.edges);
        self.type_hints.extend(other.type_hints);
        self.variables.extend(other.variables);
        self.decorators.extend(other.decorators);
        self.macros.extend(other.macros);
        self.parser_results.extend(other.parser_results);

        // Rebuild indexes
        self.indexes = OnceLock::new();
    }
}

/// Pre-built HashMap indexes for O(1) lookups on [`CodeGraph`].
pub struct CodeGraphIndexes {
    /// Symbol name to symbol indices.
    pub by_name: HashMap<String, Vec<usize>>,
    /// File path to symbol indices in that file.
    pub symbols_by_file: HashMap<String, Vec<usize>>,
    /// File path to import indices in that file.
    pub imports_by_file: HashMap<String, Vec<usize>>,
    /// File path to call indices in that file.
    pub calls_by_file: HashMap<String, Vec<usize>>,
    /// Set of all file paths for fast membership checks.
    pub file_set: std::collections::HashSet<String>,
}

impl CodeGraphIndexes {
    /// Find the first symbol with the given name using the index.
    pub fn find_symbol<'a>(&self, graph: &'a CodeGraph, name: &str) -> Option<&'a SymbolFact> {
        self.by_name
            .get(name)
            .and_then(|idxs| idxs.first())
            .map(|&i| &graph.symbols[i])
    }

    /// Find all symbols with the given name using the index.
    pub fn symbols_by_name<'a>(&self, graph: &'a CodeGraph, name: &str) -> Vec<&'a SymbolFact> {
        self.by_name
            .get(name)
            .map(|idxs| idxs.iter().map(|&i| &graph.symbols[i]).collect())
            .unwrap_or_default()
    }

    /// Get all symbols in the given file using the index.
    pub fn symbols_in_file<'a>(&self, graph: &'a CodeGraph, path: &str) -> Vec<&'a SymbolFact> {
        self.symbols_by_file
            .get(path)
            .map(|idxs| idxs.iter().map(|&i| &graph.symbols[i]).collect())
            .unwrap_or_default()
    }

    /// Get all imports in the given file using the index.
    pub fn imports_in_file<'a>(&self, graph: &'a CodeGraph, path: &str) -> Vec<&'a ImportFact> {
        self.imports_by_file
            .get(path)
            .map(|idxs| idxs.iter().map(|&i| &graph.imports[i]).collect())
            .unwrap_or_default()
    }

    /// Get all calls in the given file using the index.
    pub fn calls_in_file<'a>(&self, graph: &'a CodeGraph, path: &str) -> Vec<&'a CallFact> {
        self.calls_by_file
            .get(path)
            .map(|idxs| idxs.iter().map(|&i| &graph.calls[i]).collect())
            .unwrap_or_default()
    }

    /// Check if the graph contains the given file path.
    pub fn has_file(&self, path: &str) -> bool {
        self.file_set.contains(path)
    }
}

// ── LanguageIndexer trait ──────────────────────────────────────────────────

/// Each language implements this trait to extract facts from parsed trees.
///
/// Implementations should pre-compile tree-sitter queries for performance.
pub trait LanguageIndexer: Send + Sync {
    /// Returns the language identifier (e.g. "rust", "typescript").
    fn language_id(&self) -> &'static str;
    /// Returns file extensions this indexer handles (e.g. `&["rs"]`).
    fn extensions(&self) -> &'static [&'static str];
    /// Returns the tree-sitter language for parsing.
    fn tree_sitter_language(&self) -> tree_sitter::Language;
    /// Extract import statements from a parsed tree.
    fn extract_imports(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<ImportFact>;
    /// Extract symbol definitions from a parsed tree.
    fn extract_symbols(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<SymbolFact>;
    /// Extract call sites from a parsed tree.
    fn extract_calls(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<CallFact>;

    /// Extract variable bindings with scope and usage information.
    /// Default implementation returns empty (opt-in per language).
    fn extract_variables(
        &self,
        tree: &tree_sitter::Tree,
        source: &str,
        file_path: &str,
    ) -> Vec<VariableFact> {
        let _ = (tree, source, file_path);
        Vec::new()
    }
}

// ── Builder ────────────────────────────────────────────────────────────────

/// Builder for constructing a [`CodeGraph`] from scanned source files.
pub struct CodemapBuilder {
    files: Vec<ScannedFile>,
    backend: graxus_core::ParserBackend,
}

struct FileExtraction {
    imports: Vec<ImportFact>,
    symbols: Vec<SymbolFact>,
    calls: Vec<CallFact>,
    variables: Vec<VariableFact>,
    type_impls: Vec<TypeImplFact>,
    parser: FileParserResult,
}

#[cfg(feature = "ripex")]
fn extract_with_ripex(
    requested_backend: graxus_core::ParserBackend,
    language: &str,
    extension: &str,
    source: &str,
    scanned: &ScannedFile,
) -> anyhow::Result<FileExtraction> {
    let extracted = ripex_bridge::try_extract(language, extension, source, scanned)?;
    Ok(FileExtraction {
        imports: extracted.imports,
        symbols: extracted.symbols,
        calls: extracted.calls,
        variables: extracted.variables,
        type_impls: extracted.type_impls,
        parser: FileParserResult {
            file: scanned.relative_path.clone(),
            requested_backend,
            used_backend: graxus_core::ParserBackend::Ripex,
            fallback_reason: None,
            diagnostics: extracted.diagnostics,
            facts: extracted.parser_facts,
        },
    })
}

#[cfg(not(feature = "ripex"))]
fn extract_with_ripex(
    _requested_backend: graxus_core::ParserBackend,
    _language: &str,
    _extension: &str,
    _source: &str,
    _scanned: &ScannedFile,
) -> anyhow::Result<FileExtraction> {
    anyhow::bail!("ripex support was disabled at compile time")
}

impl CodemapBuilder {
    /// Create a new builder from the given scanned files.
    ///
    /// Defaults to the `ripex` backend (prefer the sibling `ripex` parser, falling back
    /// to tree-sitter per-file on failure or unsupported language).
    pub fn new(files: Vec<ScannedFile>) -> Self {
        Self {
            files,
            backend: graxus_core::ParserBackend::Ripex,
        }
    }

    /// Override the parser backend used during [`build`](Self::build).
    pub fn with_backend(mut self, backend: graxus_core::ParserBackend) -> Self {
        self.backend = backend;
        self
    }

    /// Extract facts for a single file using the tree-sitter `indexer`.
    ///
    /// This is the original code path and is used directly when the backend is
    /// `TreeSitter`, and as the fallback for the `Ripex` backend when ripex
    /// fails or does not support the language.
    fn tree_sitter_extract(
        &self,
        indexer: &dyn crate::LanguageIndexer,
        source: &str,
        rel: &str,
    ) -> (
        Vec<ImportFact>,
        Vec<SymbolFact>,
        Vec<CallFact>,
        Vec<VariableFact>,
    ) {
        let mut parser = tree_sitter::Parser::new();
        if let Err(e) = parser.set_language(&indexer.tree_sitter_language()) {
            tracing::warn!("Failed to set tree-sitter language for {rel}: {e}");
            return (vec![], vec![], vec![], vec![]);
        }
        let Some(tree) = parser.parse(source, None) else {
            tracing::warn!("Failed to parse {rel} with tree-sitter");
            return (vec![], vec![], vec![], vec![]);
        };
        let imports = indexer.extract_imports(&tree, source, rel);
        let symbols = indexer.extract_symbols(&tree, source, rel);
        let calls = indexer.extract_calls(&tree, source, rel);
        let vars = indexer.extract_variables(&tree, source, rel);
        (imports, symbols, calls, vars)
    }

    fn extract_file(
        &self,
        indexer: &dyn crate::LanguageIndexer,
        source: &str,
        scanned: &ScannedFile,
        ext: &str,
    ) -> FileExtraction {
        let rel = &scanned.relative_path;
        let lang = scanned.language.as_str();

        if self.backend.effective() == graxus_core::ParserBackend::Ripex {
            match extract_with_ripex(self.backend, lang, ext, source, scanned) {
                Ok(extraction) => return extraction,
                Err(error) => {
                    tracing::warn!(
                        "ripex backend failed for {} ({}); falling back to tree-sitter",
                        rel,
                        error
                    );
                    let (imports, symbols, calls, variables) =
                        self.tree_sitter_extract(indexer, source, rel);
                    return FileExtraction {
                        imports,
                        symbols,
                        calls,
                        variables,
                        type_impls: Vec::new(),
                        parser: FileParserResult {
                            file: rel.clone(),
                            requested_backend: self.backend,
                            used_backend: graxus_core::ParserBackend::TreeSitter,
                            fallback_reason: Some(error.to_string()),
                            diagnostics: Vec::new(),
                            facts: Vec::new(),
                        },
                    };
                }
            }
        }

        let (imports, symbols, calls, variables) = self.tree_sitter_extract(indexer, source, rel);
        FileExtraction {
            imports,
            symbols,
            calls,
            variables,
            type_impls: Vec::new(),
            parser: FileParserResult {
                file: rel.clone(),
                requested_backend: self.backend,
                used_backend: graxus_core::ParserBackend::TreeSitter,
                fallback_reason: None,
                diagnostics: Vec::new(),
                facts: Vec::new(),
            },
        }
    }

    /// Build the complete code graph for all scanned code files.
    #[instrument(skip_all, fields(files = self.files.len()))]
    pub fn build(&self) -> anyhow::Result<CodeGraph> {
        let registry = languages::registry();
        let mut all_imports = Vec::new();
        let mut all_symbols = Vec::new();
        let mut all_calls = Vec::new();
        let mut all_variables = Vec::new();
        let mut all_type_impls = Vec::new();
        let mut all_routes = Vec::new();
        let mut all_di_bindings = Vec::new();
        let mut all_decorators = Vec::new();
        let mut all_macros = Vec::new();
        let mut parser_results = Vec::new();
        let mut file_nodes = Vec::new();

        let lang_files = self.group_by_language();

        for (lang_id, files) in &lang_files {
            let indexer = match registry.get(lang_id.as_str()) {
                Some(idx) => idx,
                None => continue,
            };

            for scanned in files {
                let source = match std::fs::read_to_string(&scanned.path) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("Failed to read {}: {}", scanned.path.display(), e);
                        continue;
                    }
                };

                let rel = &scanned.relative_path;
                let ext = scanned
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let FileExtraction {
                    mut imports,
                    mut symbols,
                    mut calls,
                    variables: mut vars,
                    type_impls,
                    parser,
                } = self.extract_file(&**indexer, &source, scanned, &ext);

                // Assign IDs
                for (i, imp) in imports.iter_mut().enumerate() {
                    imp.id = format!("import:{}:{}", rel, i);
                }
                for sym in symbols.iter_mut() {
                    sym.id = format!("symbol:{}:{}", rel, sym.name);
                }
                // Calls are assigned a deterministic id keyed on (file, line,
                // ordinal) so they don't collide on the SQLite `calls.id` PK.
                // Without this, every call shared an empty id and `INSERT OR
                // REPLACE` collapsed them into a single row.
                for (i, call) in calls.iter_mut().enumerate() {
                    call.id = format!("call:{}:{}:{}", rel, call.line, i);
                }
                for (i, v) in vars.iter_mut().enumerate() {
                    v.id = format!("var:{}:{}", rel, i);
                }

                all_imports.extend(imports);
                all_symbols.extend(symbols);
                all_calls.extend(calls);
                all_variables.extend(vars);
                all_type_impls.extend(type_impls);
                all_routes.extend(frameworks::extract_routes(rel, &source, lang_id));
                all_type_impls.extend(resolver::type_resolver::extract_type_impls(
                    rel, &source, lang_id,
                ));
                all_di_bindings.extend(resolver::di_resolver::extract_di_bindings(rel, &source));
                parser_results.push(parser);

                file_nodes.push(FileNode {
                    path: rel.clone(),
                    language: scanned.language.as_str().to_string(),
                    hash: scanned.hash.clone(),
                    size: scanned.size,
                });

                // Extract decorators/attributes and macro definitions via the
                // text-scan extractors. These are language-agnostic heuristics
                // that complement the AST-based symbol extraction above.
                all_decorators.extend(extractor::decorators::extract_decorators(
                    &source,
                    rel,
                    lang_id.as_str(),
                ));
                all_macros.extend(extractor::macros::extract_macros(
                    &source,
                    rel,
                    lang_id.as_str(),
                ));
            }
        }

        // Resolve imports to files
        resolver::import_resolver::resolve_imports(&mut all_imports, &file_nodes);

        // Resolve calls to symbols
        resolver::symbol_resolver::resolve_calls(&mut all_calls, &all_symbols, &all_imports);

        // Build file -> symbols index for O(1) file lookup in caller population
        let mut symbols_by_file: HashMap<&str, Vec<&SymbolFact>> = HashMap::new();
        for sym in &all_symbols {
            symbols_by_file
                .entry(sym.file.as_str())
                .or_default()
                .push(sym);
        }

        // Populate caller_symbol by matching each call to the enclosing symbol
        for call in all_calls.iter_mut() {
            let call_line = call.line;
            if let Some(syms) = symbols_by_file.get(call.file.as_str()) {
                if let Some(enclosing) = syms
                    .iter()
                    .find(|s| s.line_start <= call_line && call_line <= s.line_end)
                {
                    call.caller_symbol = Some(enclosing.name.clone());
                }
            }
        }

        let mut routes = resolver::route_resolver::resolve_routes(all_routes, &all_symbols);
        for (i, route) in routes.iter_mut().enumerate() {
            route.id = format!("route:{}:{}:{}:{}", route.file, route.line, route.method, i);
        }
        let mut type_impls = resolver::type_resolver::resolve_type_impls(all_type_impls);
        for (i, type_impl) in type_impls.iter_mut().enumerate() {
            type_impl.id = format!(
                "type_impl:{}:{}:{}:{}",
                type_impl.file, type_impl.line, type_impl.implementing_type, i
            );
        }
        let mut di_bindings = resolver::di_resolver::resolve_di_bindings(all_di_bindings);
        for (i, binding) in di_bindings.iter_mut().enumerate() {
            binding.id = format!(
                "di:{}:{}:{}:{}",
                binding.file, binding.line, binding.abstract_type, i
            );
        }

        // Build edges
        let mut edges = Vec::new();
        for imp in &all_imports {
            if let Some(ref resolved) = imp.resolved_file {
                edges.push(CodeEdge {
                    from: imp.file.clone(),
                    to: resolved.clone(),
                    edge_type: CodeEdgeType::Imports,
                });
            }
        }
        for sym in &all_symbols {
            edges.push(CodeEdge {
                from: format!("{}::{}", sym.file, sym.name),
                to: sym.file.clone(),
                edge_type: CodeEdgeType::DefinedIn,
            });
        }
        for call in &all_calls {
            if let Some(ref resolved) = call.resolved_symbol {
                edges.push(CodeEdge {
                    from: call.file.clone(),
                    to: resolved.clone(),
                    edge_type: CodeEdgeType::Calls,
                });
            }
        }
        for type_impl in &type_impls {
            edges.push(CodeEdge {
                from: format!("{}::{}", type_impl.file, type_impl.implementing_type),
                to: type_impl.trait_or_interface.clone(),
                edge_type: match type_impl.kind {
                    crate::facts::ImplKind::TraitImpl
                    | crate::facts::ImplKind::Implements
                    | crate::facts::ImplKind::Derive => CodeEdgeType::Implements,
                    crate::facts::ImplKind::Extends
                    | crate::facts::ImplKind::CSharpInheritance
                    | crate::facts::ImplKind::CppInheritance => CodeEdgeType::Extends,
                },
            });
        }

        Ok(CodeGraph {
            files: file_nodes,
            symbols: all_symbols,
            imports: all_imports,
            calls: all_calls,
            routes,
            type_impls,
            di_bindings,
            edges,
            type_hints: Vec::new(),
            variables: all_variables,
            decorators: all_decorators,
            macros: all_macros,
            parser_results,
            indexes: OnceLock::new(),
        })
    }

    /// Build analysis for a single file.
    pub fn build_for_file(&self, path: &Path) -> anyhow::Result<FileAnalysis> {
        let scanned = self
            .files
            .iter()
            .find(|f| f.path == path)
            .ok_or_else(|| anyhow::anyhow!("File not found in scan: {}", path.display()))?;

        let registry = languages::registry();
        let lang_id = scanned.language.as_str();
        let indexer = registry
            .get(lang_id)
            .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", lang_id))?;

        let source = std::fs::read_to_string(&scanned.path)?;
        let rel = &scanned.relative_path;
        let ext = scanned
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let FileExtraction {
            mut imports,
            mut symbols,
            mut calls,
            variables: mut vars,
            type_impls,
            parser,
        } = self.extract_file(&**indexer, &source, scanned, &ext);

        for (i, imp) in imports.iter_mut().enumerate() {
            imp.id = format!("import:{}:{}", rel, i);
        }
        for sym in symbols.iter_mut() {
            sym.id = format!("symbol:{}:{}", rel, sym.name);
        }
        for (i, call) in calls.iter_mut().enumerate() {
            call.id = format!("call:{}:{}:{}", rel, call.line, i);
        }
        for (i, v) in vars.iter_mut().enumerate() {
            v.id = format!("var:{}:{}", rel, i);
        }

        Ok(FileAnalysis {
            file: rel.clone(),
            language: lang_id.to_string(),
            imports,
            symbols,
            calls,
            routes: Vec::new(),
            type_impls,
            di_bindings: Vec::new(),
            variables: vars,
            parser,
        })
    }

    /// Save the code graph to disk as JSON files.
    #[instrument(skip_all)]
    pub fn save(graph: &CodeGraph, output_dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(output_dir)?;

        let codemap_path = output_dir.join("codemap.json");
        let symbols_path = output_dir.join("symbols.json");
        let imports_path = output_dir.join("imports.json");

        std::fs::write(&codemap_path, serde_json::to_string_pretty(graph)?)?;
        std::fs::write(&symbols_path, serde_json::to_string_pretty(&graph.symbols)?)?;
        std::fs::write(&imports_path, serde_json::to_string_pretty(&graph.imports)?)?;

        tracing::info!("Saved codemap to {}", output_dir.display());
        Ok(())
    }

    fn group_by_language(&self) -> HashMap<String, Vec<&ScannedFile>> {
        let mut map: HashMap<String, Vec<&ScannedFile>> = HashMap::new();
        for file in &self.files {
            let lang = file.language.as_str().to_string();
            map.entry(lang).or_default().push(file);
        }
        map
    }
}

/// Build a code graph from scanned files (convenience wrapper).
pub fn build(files: &[ScannedFile]) -> anyhow::Result<CodeGraph> {
    CodemapBuilder::new(files.to_vec()).build()
}

/// Save a code graph to disk (convenience wrapper).
pub fn save(graph: &CodeGraph, output_dir: &Path) -> anyhow::Result<()> {
    CodemapBuilder::save(graph, output_dir)
}
