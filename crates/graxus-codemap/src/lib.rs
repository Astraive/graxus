//! Graxus codemap — Source-code structure, symbols, imports, relationships.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use graxus_core::ScannedFile;

pub mod extractor;
pub mod graph;
pub mod languages;
pub mod resolver;

// ── Enums ──────────────────────────────────────────────────────────────────

/// Numeric confidence score (0.0 to 100.0).
/// Custom deserializer handles both old string format ("high", "low") and new object format.
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

            fn visit_map<M: de::MapAccess<'de>>(self, mut map: M) -> Result<ConfidenceScore, M::Error> {
                let mut score: Option<f64> = None;
                let mut label: Option<ConfidenceLabel> = None;
                let mut method: Option<ResolutionMethod> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "score" => score = Some(map.next_value()?),
                        "label" => { let _: serde_json::Value = map.next_value()?; }
                        "method" => { let _: serde_json::Value = map.next_value()?; }
                        _ => { let _: serde_json::Value = map.next_value()?; }
                    }
                }
                let s = score.unwrap_or(0.0);
                Ok(ConfidenceScore::new(s, ResolutionMethod::Unresolved))
            }
        }

        deserializer.deserialize_any(ConfidenceVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfidenceLabel {
    Exact,      // 95-100
    High,       // 80-94
    Medium,     // 60-79
    Low,        // 35-59
    Weak,       // 1-34
    Unresolved, // 0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResolutionMethod {
    LocalDefinition,
    NamedImportExactExport,
    DefaultImportExactExport,
    NamespaceImportMember,
    RustUseExactPath,
    GoPackageSelector,
    PythonFromImport,
    PythonModuleAlias,
    SameFileSymbol,
    ModulePublicSymbol,
    PathMatchOnly,
    NameMatchSameProject,
    FuzzySymbolMatch,
    ExternalDependency,
    SyntaxOnly,
    Unresolved,
}

impl ConfidenceScore {
    pub fn new(score: f64, method: ResolutionMethod) -> Self {
        let label = match score as u32 {
            95..=100 => ConfidenceLabel::Exact,
            80..=94 => ConfidenceLabel::High,
            60..=79 => ConfidenceLabel::Medium,
            35..=59 => ConfidenceLabel::Low,
            1..=34 => ConfidenceLabel::Weak,
            _ => ConfidenceLabel::Unresolved,
        };
        Self { score: score.clamp(0.0, 100.0), label, method }
    }

    pub fn unresolved() -> Self {
        Self { score: 0.0, label: ConfidenceLabel::Unresolved, method: ResolutionMethod::Unresolved }
    }

    pub fn local_definition() -> Self { Self::new(98.0, ResolutionMethod::LocalDefinition) }
    pub fn named_import_exact() -> Self { Self::new(95.0, ResolutionMethod::NamedImportExactExport) }
    pub fn default_import_exact() -> Self { Self::new(88.0, ResolutionMethod::DefaultImportExactExport) }
    pub fn namespace_import() -> Self { Self::new(93.0, ResolutionMethod::NamespaceImportMember) }
    pub fn rust_use_path() -> Self { Self::new(95.0, ResolutionMethod::RustUseExactPath) }
    pub fn go_package() -> Self { Self::new(88.0, ResolutionMethod::GoPackageSelector) }
    pub fn python_from() -> Self { Self::new(85.0, ResolutionMethod::PythonFromImport) }
    pub fn same_project() -> Self { Self::new(40.0, ResolutionMethod::NameMatchSameProject) }
    pub fn path_match_only() -> Self { Self::new(60.0, ResolutionMethod::PathMatchOnly) }
    pub fn syntax_only() -> Self { Self::new(10.0, ResolutionMethod::SyntaxOnly) }

    pub fn is_resolved(&self) -> bool { self.score > 0.0 }
}

impl Default for ConfidenceScore {
    fn default() -> Self { Self::unresolved() }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    NamedImport,
    NamespaceImport,
    DefaultImport,
    SideEffectImport,
    RustUse,
    GoImport,
    FromImport,
    PythonImport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Class,
    Struct,
    Trait,
    Interface,
    Method,
    Module,
    Constant,
    Enum,
    Type,
    Variable,
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallKind {
    FunctionCall,
    MethodCall,
    ConstructorCall,
    PathCall,
    SelectorCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CodeEdgeType {
    Contains,
    Imports,
    Exports,
    Calls,
    Implements,
    Extends,
    References,
    DefinedIn,
}

// ── Data types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportFact {
    pub id: String,
    pub file: String,
    pub language: String,
    pub kind: ImportKind,
    pub source: String,
    pub local_name: Option<String>,
    pub imported_name: Option<String>,
    pub resolved_file: Option<String>,
    pub line: usize,
    pub confidence: ConfidenceScore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolFact {
    pub id: String,
    pub file: String,
    pub language: String,
    pub kind: SymbolKind,
    pub name: String,
    pub exported: bool,
    pub line_start: usize,
    pub line_end: usize,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallFact {
    pub id: String,
    pub file: String,
    pub language: String,
    pub kind: CallKind,
    pub caller_symbol: Option<String>,
    pub callee_text: String,
    pub object: Option<String>,
    pub resolved_symbol: Option<String>,
    pub line: usize,
    pub column: usize,
    pub confidence: ConfidenceScore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeEdge {
    pub from: String,
    pub to: String,
    pub edge_type: CodeEdgeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub path: String,
    pub language: String,
    pub hash: String,
    pub size: u64,
}

/// A single file's analysis (raw facts before resolution).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysis {
    pub file: String,
    pub language: String,
    pub imports: Vec<ImportFact>,
    pub symbols: Vec<SymbolFact>,
    pub calls: Vec<CallFact>,
}

/// Type hint from constructor calls (e.g., `const x = new Foo()` → x: Foo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeHint {
    pub file: String,
    pub variable: String,
    pub type_name: String,
    pub line: usize,
}

/// Complete code graph containing all extracted and resolved facts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodeGraph {
    pub files: Vec<FileNode>,
    pub symbols: Vec<SymbolFact>,
    pub imports: Vec<ImportFact>,
    pub calls: Vec<CallFact>,
    pub edges: Vec<CodeEdge>,
    #[serde(default)]
    pub type_hints: Vec<TypeHint>,
}

impl CodeGraph {
    pub fn new() -> Self {
        Self::default()
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
        CodeGraphIndexes { by_name, symbols_by_file, imports_by_file, calls_by_file, file_set }
    }

    pub fn find_symbol(&self, name: &str) -> Option<&SymbolFact> {
        self.symbols.iter().find(|s| s.name == name)
    }

    pub fn find_symbols(&self, name: &str) -> Vec<&SymbolFact> {
        self.symbols.iter().filter(|s| s.name == name).collect()
    }

    pub fn symbols_in_file(&self, path: &str) -> Vec<&SymbolFact> {
        self.symbols.iter().filter(|s| s.file == path).collect()
    }

    pub fn imports_in_file(&self, path: &str) -> Vec<&ImportFact> {
        self.imports.iter().filter(|i| i.file == path).collect()
    }

    pub fn calls_in_file(&self, path: &str) -> Vec<&CallFact> {
        self.calls.iter().filter(|c| c.file == path).collect()
    }

    pub fn calls_to_symbol(&self, symbol_id: &str) -> Vec<&CallFact> {
        self.calls
            .iter()
            .filter(|c| c.resolved_symbol.as_deref() == Some(symbol_id))
            .collect()
    }

    pub fn has_file(&self, path: &str) -> bool {
        self.files.iter().any(|f| f.path == path)
    }

    pub fn file_paths(&self) -> Vec<&str> {
        self.files.iter().map(|f| f.path.as_str()).collect()
    }
}

/// Pre-built HashMap indexes for O(1) lookups on CodeGraph.
pub struct CodeGraphIndexes {
    pub by_name: HashMap<String, Vec<usize>>,
    pub symbols_by_file: HashMap<String, Vec<usize>>,
    pub imports_by_file: HashMap<String, Vec<usize>>,
    pub calls_by_file: HashMap<String, Vec<usize>>,
    pub file_set: std::collections::HashSet<String>,
}

impl CodeGraphIndexes {
    pub fn find_symbol<'a>(&self, graph: &'a CodeGraph, name: &str) -> Option<&'a SymbolFact> {
        self.by_name.get(name).and_then(|idxs| idxs.first()).map(|&i| &graph.symbols[i])
    }

    pub fn symbols_in_file<'a>(&self, graph: &'a CodeGraph, path: &str) -> Vec<&'a SymbolFact> {
        self.symbols_by_file.get(path).map(|idxs| idxs.iter().map(|&i| &graph.symbols[i]).collect()).unwrap_or_default()
    }

    pub fn has_file(&self, path: &str) -> bool {
        self.file_set.contains(path)
    }
}

// ── LanguageIndexer trait ──────────────────────────────────────────────────

/// Each language implements this trait to extract facts from parsed trees.
pub trait LanguageIndexer: Send + Sync {
    fn language_id(&self) -> &'static str;
    fn extensions(&self) -> &'static [&'static str];

    fn tree_sitter_language(&self) -> tree_sitter::Language;

    fn extract_imports(&self, tree: &tree_sitter::Tree, source: &str, file_path: &str) -> Vec<ImportFact>;
    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &str, file_path: &str) -> Vec<SymbolFact>;
    fn extract_calls(&self, tree: &tree_sitter::Tree, source: &str, file_path: &str) -> Vec<CallFact>;
}

// ── Builder ────────────────────────────────────────────────────────────────

pub struct CodemapBuilder {
    files: Vec<ScannedFile>,
}

impl CodemapBuilder {
    pub fn new(files: Vec<ScannedFile>) -> Self {
        Self { files }
    }

    /// Build the complete code graph for all scanned code files.
    pub fn build(&self) -> anyhow::Result<CodeGraph> {
        let registry = languages::registry();
        let mut all_imports = Vec::new();
        let mut all_symbols = Vec::new();
        let mut all_calls = Vec::new();
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

                let mut parser = tree_sitter::Parser::new();
                if let Err(e) = parser.set_language(&indexer.tree_sitter_language()) {
                    tracing::warn!("Failed to set language {}: {}", lang_id, e);
                    continue;
                }

                let tree = match parser.parse(&source, None) {
                    Some(t) => t,
                    None => {
                        tracing::warn!("Failed to parse {}", scanned.path.display());
                        continue;
                    }
                };

                let rel = &scanned.relative_path;

                let mut imports = indexer.extract_imports(&tree, &source, rel);
                let mut symbols = indexer.extract_symbols(&tree, &source, rel);
                let calls = indexer.extract_calls(&tree, &source, rel);

                // Assign IDs
                for (i, imp) in imports.iter_mut().enumerate() {
                    imp.id = format!("import:{}:{}", rel, i);
                }
                for sym in symbols.iter_mut() {
                    sym.id = format!("symbol:{}:{}", rel, sym.name);
                }

                all_imports.extend(imports);
                all_symbols.extend(symbols);
                all_calls.extend(calls);

                file_nodes.push(FileNode {
                    path: rel.clone(),
                    language: scanned.language.as_str().to_string(),
                    hash: scanned.hash.clone(),
                    size: scanned.size,
                });
            }
        }

        // Resolve imports to files
        resolver::import_resolver::resolve_imports(&mut all_imports, &file_nodes);

        // Resolve calls to symbols
        resolver::symbol_resolver::resolve_calls(&mut all_calls, &all_symbols, &all_imports);

        // Populate caller_symbol by matching each call to the enclosing symbol
        for call in all_calls.iter_mut() {
            let call_line = call.line;
            let file = &call.file;
            // Find the symbol in the same file whose range contains this call line
            if let Some(enclosing) = all_symbols.iter().find(|s| {
                s.file == *file && s.line_start <= call_line && call_line <= s.line_end
            }) {
                call.caller_symbol = Some(enclosing.name.clone());
            }
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

        Ok(CodeGraph {
            files: file_nodes,
            symbols: all_symbols,
            imports: all_imports,
            calls: all_calls,
            edges,
            type_hints: Vec::new(),
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
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&indexer.tree_sitter_language())?;
        let tree = parser
            .parse(&source, None)
            .ok_or_else(|| anyhow::anyhow!("Parse failed"))?;

        let rel = &scanned.relative_path;
        let mut imports = indexer.extract_imports(&tree, &source, rel);
        let mut symbols = indexer.extract_symbols(&tree, &source, rel);
        let calls = indexer.extract_calls(&tree, &source, rel);

        for (i, imp) in imports.iter_mut().enumerate() {
            imp.id = format!("import:{}:{}", rel, i);
        }
        for sym in symbols.iter_mut() {
            sym.id = format!("symbol:{}:{}", rel, sym.name);
        }

        Ok(FileAnalysis {
            file: rel.clone(),
            language: lang_id.to_string(),
            imports,
            symbols,
            calls,
        })
    }

    /// Save the code graph to disk as JSON files.
    pub fn save(graph: &CodeGraph, output_dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(output_dir)?;

        let codemap_path = output_dir.join("codemap.json");
        let symbols_path = output_dir.join("symbols.json");
        let imports_path = output_dir.join("imports.json");

        std::fs::write(&codemap_path, serde_json::to_string_pretty(graph)?)?;
        std::fs::write(
            &symbols_path,
            serde_json::to_string_pretty(&graph.symbols)?,
        )?;
        std::fs::write(
            &imports_path,
            serde_json::to_string_pretty(&graph.imports)?,
        )?;

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
