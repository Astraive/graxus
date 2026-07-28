//! Integration tests for the graxus pipeline.
//!
//! Each test creates a temporary project, exercises graxus library functions,
//! and verifies the results. Tests cover: init, scanning, codemap extraction,
//! docgraph building, search, replace with rollback, SQLite storage, and
//! the full end-to-end pipeline.

use std::fs;
use std::path::Path;
use tempfile::TempDir;

use graxus_codemap::CodemapBuilder;
use graxus_core::config::GraxusConfig;
use graxus_core::file_types::{FileKind, Language};
use graxus_core::scanner;
use graxus_core::workspace;
use graxus_core::ScannedFile;
use graxus_edit::find::SearchMode;
use graxus_edit::replace::ReplaceMode;
use graxus_edit::EditEngine;
use graxus_index::{IndexStore, SqliteStore};

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Config tuned for test directories (no gitignore, narrow include set).
fn test_config() -> GraxusConfig {
    let mut config = GraxusConfig::default();
    config.scan.respect_gitignore = false;
    config.scan.include = vec!["**/*.rs".into(), "**/*.md".into(), "**/*.yaml".into()];
    config.scan.exclude = vec![".graxus/**".into()];
    config
}

/// Create a realistic test project with Rust source and Markdown docs.
fn create_test_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // graxus.yaml
    fs::write(
        root.join("graxus.yaml"),
        r#"project:
  name: test-project
  root: .
scan:
  include: ["**/*.rs", "**/*.md", "**/*.yaml"]
  exclude: ["target/**", ".graxus/**"]
  respect_gitignore: false
docs:
  enabled: true
code:
  enabled: true
"#,
    )
    .unwrap();

    fs::create_dir_all(root.join(".graxus")).unwrap();

    // Rust source with functions and a test module
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/main.rs"),
        r#"fn main() {
    println!("Hello, world!");
}

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}
"#,
    )
    .unwrap();

    // Markdown with frontmatter, wiki links, and tags
    fs::write(
        root.join("README.md"),
        r#"---
title: Test Project
tags: [test, demo]
---

# Test Project

This is a [[test]] project.

See [[src/main.rs]] for details.
"#,
    )
    .unwrap();

    fs::write(
        root.join("ARCHITECTURE.md"),
        r#"---
title: Architecture
---

# Architecture

The project uses [[main]] for entry.
"#,
    )
    .unwrap();

    dir
}

/// Build a ScannedFile for a real file on disk.
///
/// Sets `relative_path` to the absolute path so that downstream functions
/// (replace apply, snapshot) can locate the file without relying on CWD.
fn make_scanned(path: &Path, kind: FileKind, lang: Language) -> ScannedFile {
    let content = fs::read(path).unwrap();
    // Simple hash using std::hash (no external dep needed for tests)
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    ScannedFile {
        path: path.to_path_buf(),
        relative_path: path.to_string_lossy().to_string(),
        kind,
        language: lang,
        hash: format!("{:x}", hasher.finish()),
        size: content.len() as u64,
        modified: chrono::Utc::now(),
    }
}

// ── Init ────────────────────────────────────────────────────────────────────

#[test]
fn test_init_creates_structure() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    workspace::init_project(root).unwrap();

    // Directory structure
    assert!(root.join(".graxus").exists());
    for subdir in &["docs", "code", "snapshots", "logs", "reports"] {
        assert!(
            root.join(".graxus").join(subdir).exists(),
            ".graxus/{} should exist",
            subdir
        );
    }

    // Config roundtrip
    assert!(root.join("graxus.yaml").exists());
    let loaded = GraxusConfig::load(root).unwrap();
    assert!(!loaded.project.name.is_empty());
}

// ── Scanning ────────────────────────────────────────────────────────────────

#[test]
fn test_scan_finds_rust_and_markdown_files() {
    let dir = create_test_project();
    let root = dir.path();
    let config = test_config();

    let files = scanner::scan(root, &config).unwrap();

    assert!(
        files.len() >= 3,
        "Expected at least 3 files, got {}",
        files.len()
    );

    let rs: Vec<_> = files
        .iter()
        .filter(|f| f.language == Language::Rust)
        .collect();
    let md: Vec<_> = files
        .iter()
        .filter(|f| f.language == Language::Markdown)
        .collect();

    assert!(!rs.is_empty(), "Should find Rust files");
    assert!(!md.is_empty(), "Should find Markdown files");
    assert_eq!(rs[0].kind, FileKind::Code);
    assert_eq!(md[0].kind, FileKind::Doc);
}

#[test]
fn test_scan_categorized_separates_docs_and_code() {
    let dir = create_test_project();
    let root = dir.path();
    let config = test_config();

    let (docs, code, configs) = scanner::scan_categorized(root, &config).unwrap();

    assert!(!docs.is_empty(), "Should have doc files");
    assert!(!code.is_empty(), "Should have code files");

    for d in &docs {
        assert_eq!(d.kind, FileKind::Doc);
    }
    for c in &code {
        assert_eq!(c.kind, FileKind::Code);
    }
    // graxus.yaml should appear in configs
    assert!(
        configs
            .iter()
            .any(|c| c.relative_path.contains("graxus.yaml")),
        "Should find graxus.yaml in config files"
    );
}

// ── Codemap ─────────────────────────────────────────────────────────────────

#[test]
fn test_codemap_extracts_rust_symbols() {
    let dir = create_test_project();
    let root = dir.path();
    let config = test_config();

    let (_docs, code, _) = scanner::scan_categorized(root, &config).unwrap();
    assert!(!code.is_empty());

    let graph = CodemapBuilder::new(code).build().unwrap();

    assert!(!graph.files.is_empty(), "Codemap should have files");

    let names: Vec<&str> = graph.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"main"), "Should find main, got {:?}", names);
    assert!(names.contains(&"add"), "Should find add, got {:?}", names);
    assert!(
        names.contains(&"test_add"),
        "Should find test_add, got {:?}",
        names
    );

    let main = graph.find_symbol("main").unwrap();
    assert_eq!(main.kind, graxus_codemap::SymbolKind::Function);

    let test_fn = graph.find_symbol("test_add").unwrap();
    assert!(test_fn.is_test, "test_add should be marked as test");
}

#[test]
fn test_codemap_builds_indexes() {
    let dir = create_test_project();
    let root = dir.path();
    let config = test_config();

    let (_docs, code, _) = scanner::scan_categorized(root, &config).unwrap();
    let graph = CodemapBuilder::new(code).build().unwrap();
    let indexes = graph.build_indexes();

    assert!(
        indexes.find_symbol(&graph, "main").is_some(),
        "Index should find main"
    );
    assert!(
        indexes.find_symbol(&graph, "add").is_some(),
        "Index should find add"
    );
}

#[test]
fn test_codemap_save_and_load() {
    let dir = create_test_project();
    let root = dir.path();
    let config = test_config();

    let (_docs, code, _) = scanner::scan_categorized(root, &config).unwrap();
    let graph = CodemapBuilder::new(code).build().unwrap();

    let out = root.join(".graxus/code");
    CodemapBuilder::save(&graph, &out).unwrap();

    assert!(out.join("codemap.json").exists());
    assert!(out.join("symbols.json").exists());
    assert!(out.join("imports.json").exists());

    // Verify JSON is valid
    let raw = fs::read_to_string(out.join("codemap.json")).unwrap();
    let loaded: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(loaded.get("files").is_some());
    assert!(loaded.get("symbols").is_some());
}

// ── Docgraph ────────────────────────────────────────────────────────────────

#[test]
fn test_docgraph_builds_with_wiki_links_and_tags() {
    let dir = create_test_project();
    let root = dir.path();
    let config = test_config();

    let graph = graxus_docgraph::build(root, &config).unwrap();

    assert_eq!(graph.nodes.len(), 2, "Expected 2 doc nodes");

    let readme = graph.find_by_path("README.md").expect("README.md missing");
    assert_eq!(readme.title, "Test Project");

    let arch = graph
        .find_by_path("ARCHITECTURE.md")
        .expect("ARCHITECTURE.md missing");
    assert_eq!(arch.title, "Architecture");

    // Tags from frontmatter
    let tags = graph.get_all_tags();
    assert!(
        tags.contains(&"test".to_string()),
        "Should have 'test' tag, got {:?}",
        tags
    );
    assert!(tags.contains(&"demo".to_string()), "Should have 'demo' tag");

    // Wiki links
    assert!(!readme.wiki_links.is_empty());
    let targets: Vec<&str> = readme
        .wiki_links
        .iter()
        .map(|l| l.target.as_str())
        .collect();
    assert!(targets.contains(&"test"), "Should link to 'test'");
    assert!(
        targets.contains(&"src/main.rs"),
        "Should link to 'src/main.rs'"
    );

    // Edges: LinksTo + BacklinksTo + HasTag + HasHeading
    use graxus_docgraph::graph::DocEdgeType;
    let links_to: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| matches!(e.edge_type, DocEdgeType::LinksTo))
        .collect();
    let backlinks: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| matches!(e.edge_type, DocEdgeType::BacklinksTo))
        .collect();
    assert!(!links_to.is_empty(), "Should have LinksTo edges");
    assert!(!backlinks.is_empty(), "Should have BacklinksTo edges");
}

#[test]
fn test_docgraph_save_and_load_roundtrip() {
    let dir = create_test_project();
    let root = dir.path();
    let config = test_config();

    let graph = graxus_docgraph::build(root, &config).unwrap();

    let docs_dir = workspace::docs_dir(root);
    let loaded = graxus_docgraph::graph::DocGraph::load(&docs_dir).unwrap();

    assert_eq!(loaded.nodes.len(), graph.nodes.len());
    assert_eq!(loaded.edges.len(), graph.edges.len());
}

// ── Search (find) ───────────────────────────────────────────────────────────

#[test]
fn test_find_literal_search() {
    let dir = create_test_project();
    let root = dir.path();
    let config = test_config();

    let files = scanner::scan(root, &config).unwrap();
    let hits = graxus_edit::find::search("println", &files, &SearchMode::Literal).unwrap();

    assert!(!hits.is_empty(), "Should find 'println'");
    assert!(hits.iter().any(|h| h.match_text == "println"));
    assert!(hits.iter().any(|h| h.file.contains("main.rs")));
}

#[test]
fn test_find_regex_search() {
    let dir = create_test_project();
    let root = dir.path();
    let config = test_config();

    let files = scanner::scan(root, &config).unwrap();
    let hits = graxus_edit::find::search(r"fn \w+", &files, &SearchMode::Regex).unwrap();

    assert!(
        hits.len() >= 3,
        "Should find at least 3 fn definitions, got {}",
        hits.len()
    );
}

#[test]
fn test_find_search_across_docs_and_code() {
    let dir = create_test_project();
    let root = dir.path();
    let config = test_config();

    let files = scanner::scan(root, &config).unwrap();
    let hits = graxus_edit::find::search("project", &files, &SearchMode::Literal).unwrap();

    let doc_hits: Vec<_> = hits.iter().filter(|h| h.file.ends_with(".md")).collect();
    assert!(!doc_hits.is_empty(), "Should find 'project' in docs");
}

// ── Replace ─────────────────────────────────────────────────────────────────

#[test]
fn test_replace_preview_shows_changes() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    let test_file = root.join("example.rs");
    fs::write(&test_file, "fn goodbye() {\n    println!(\"goodbye\");\n}").unwrap();

    let scanned = make_scanned(&test_file, FileKind::Code, Language::Rust);

    let preview = graxus_edit::replace::preview_replace(
        "goodbye",
        "greet",
        &[scanned],
        &ReplaceMode::Literal,
        100,
    )
    .unwrap();

    assert_eq!(preview.total_replacements, 2);
    assert_eq!(preview.affected_files.len(), 1);
    assert_eq!(preview.old, "goodbye");
    assert_eq!(preview.new, "greet");
    assert_eq!(preview.mode, "literal");
}

#[test]
fn test_replace_apply_and_rollback() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    let test_file = root.join("test.rs");
    fs::write(&test_file, "fn goodbye() {\n    println!(\"goodbye\");\n}").unwrap();

    let scanned = make_scanned(&test_file, FileKind::Code, Language::Rust);

    let store = IndexStore::new(root.join(".graxus"));
    fs::create_dir_all(root.join(".graxus")).unwrap();
    let engine = EditEngine::new(store, 100);

    // Preview
    let preview = engine
        .preview_replace("goodbye", "greet", &[scanned], ReplaceMode::Literal)
        .unwrap();
    assert_eq!(preview.total_replacements, 2);

    // Apply (snapshot + mutate)
    let snapshot = engine.apply_replace(&preview, "test-replace").unwrap();

    let content = fs::read_to_string(&test_file).unwrap();
    assert!(
        content.contains("greet"),
        "Should contain 'greet', got: {}",
        content
    );
    assert!(
        !content.contains("goodbye"),
        "Should not contain 'goodbye' after replace"
    );

    // Rollback
    engine.rollback(&snapshot).unwrap();

    let content = fs::read_to_string(&test_file).unwrap();
    assert!(
        content.contains("goodbye"),
        "Should contain 'goodbye' after rollback, got: {}",
        content
    );
    assert!(
        !content.contains("greet"),
        "Should not contain 'greet' after rollback"
    );
}

// ── SQLite storage ──────────────────────────────────────────────────────────

#[test]
fn test_sqlite_store_insert_and_query() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");

    let store = SqliteStore::new(&db_path).unwrap();

    store
        .insert_symbol(
            "sym:main",
            "src/main.rs",
            "rust",
            "function",
            "main",
            false,
            1,
            5,
            "public",
            "fn main()",
            false,
            0,
        )
        .unwrap();

    store
        .insert_symbol(
            "sym:add",
            "src/main.rs",
            "rust",
            "function",
            "add",
            true,
            8,
            10,
            "public",
            "fn add(a: i32, b: i32) -> i32",
            false,
            0,
        )
        .unwrap();

    assert_eq!(store.symbol_count().unwrap(), 2);

    let syms = store.get_symbols_by_name("main").unwrap();
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].file, "src/main.rs");

    let file_syms = store.get_symbols_by_file("src/main.rs").unwrap();
    assert_eq!(file_syms.len(), 2);

    let results = store.search_symbols("add").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "add");
}

#[test]
fn test_sqlite_store_insert_imports_and_calls() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");

    let store = SqliteStore::new(&db_path).unwrap();

    store
        .insert_import(
            "imp:0",
            "src/main.rs",
            "rust",
            "rust_use",
            "std::collections::HashMap",
            Some("HashMap"),
            None,
            None,
            1,
            "high",
        )
        .unwrap();

    store
        .insert_call(
            "call:0",
            "src/main.rs",
            "rust",
            "function_call",
            Some("main"),
            "println",
            None,
            None,
            3,
            5,
            "high",
        )
        .unwrap();

    assert_eq!(store.import_count().unwrap(), 1);
    assert_eq!(store.call_count().unwrap(), 1);

    let imps = store.get_imports_by_file("src/main.rs").unwrap();
    assert_eq!(imps.len(), 1);
    assert_eq!(imps[0].source, "std::collections::HashMap");
}

// ── IndexStore (JSON + snapshots) ───────────────────────────────────────────

#[test]
fn test_index_store_save_and_load_json() {
    let dir = TempDir::new().unwrap();
    let store = IndexStore::new(dir.path().to_path_buf());

    let data = serde_json::json!({
        "name": "test",
        "version": "1.0",
        "items": [1, 2, 3]
    });
    store.save_json("config.json", &data).unwrap();

    let loaded: serde_json::Value = store.load_json("config.json").unwrap();
    assert_eq!(loaded["name"], "test");
    assert_eq!(loaded["version"], "1.0");

    assert!(store.exists("config.json"));
    assert!(!store.exists("nonexistent.json"));
}

#[test]
fn test_index_store_snapshots() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    let test_file = root.join("test.txt");
    fs::write(&test_file, "original content").unwrap();

    fs::create_dir_all(root.join(".graxus")).unwrap();
    let store = IndexStore::new(root.join(".graxus"));

    // Snapshot
    let snapshot = store
        .create_snapshot("test-snap", &[test_file.clone()])
        .unwrap();
    assert_eq!(snapshot.label, "test-snap");
    assert_eq!(snapshot.files.len(), 1);

    // Modify
    fs::write(&test_file, "modified content").unwrap();
    assert_eq!(fs::read_to_string(&test_file).unwrap(), "modified content");

    // Rollback
    store.rollback_snapshot(&snapshot).unwrap();
    assert_eq!(fs::read_to_string(&test_file).unwrap(), "original content");

    // List
    let metas = store.list_snapshots().unwrap();
    assert_eq!(metas.len(), 1);
    assert_eq!(metas[0].label, "test-snap");
}

// ── Full pipeline ───────────────────────────────────────────────────────────

#[test]
fn test_full_pipeline_init_scan_codemap_docgraph_sqlite() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // 1. Init
    let config = workspace::init_project(root).unwrap();
    assert!(root.join(".graxus").exists());
    assert!(root.join("graxus.yaml").exists());

    // 2. Add test files
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/main.rs"),
        r#"fn main() {
    println!("Hello");
}

pub fn helper() -> i32 {
    42
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("README.md"),
        r#"---
title: My Project
tags: [rust]
---

# My Project

A [[demo]] project.
"#,
    )
    .unwrap();

    // 3. Scan
    let (docs, code, _configs) = scanner::scan_categorized(root, &config).unwrap();
    assert!(!code.is_empty(), "Should find code files");
    assert!(!docs.is_empty(), "Should find doc files");

    // Save file list
    let all: Vec<_> = docs.iter().chain(code.iter()).collect();
    let json = serde_json::to_string_pretty(&all).unwrap();
    fs::write(root.join(".graxus/files.json"), &json).unwrap();

    // 4. Build codemap
    let codemap = CodemapBuilder::new(code).build().unwrap();
    assert!(!codemap.files.is_empty());
    assert!(codemap.find_symbol("main").is_some());
    assert!(codemap.find_symbol("helper").is_some());

    let code_dir = root.join(".graxus/code");
    CodemapBuilder::save(&codemap, &code_dir).unwrap();
    assert!(code_dir.join("codemap.json").exists());

    // 5. Build docgraph
    let docgraph = graxus_docgraph::build(root, &config).unwrap();
    assert!(!docgraph.nodes.is_empty());

    // 6. SQLite
    let db_path = root.join(".graxus/index.db");
    let db = SqliteStore::new(&db_path).unwrap();

    for sym in &codemap.symbols {
        db.insert_symbol(
            &sym.id,
            &sym.file,
            &sym.language,
            &sym.kind.to_string(),
            &sym.name,
            sym.exported,
            sym.line_start,
            sym.line_end,
            &format!("{:?}", sym.visibility).to_lowercase(),
            &sym.signature,
            sym.is_test,
            sym.usage_count,
        )
        .unwrap();
    }

    assert_eq!(db.symbol_count().unwrap(), codemap.symbols.len());

    // 7. Verify all artifacts
    assert!(root.join(".graxus/files.json").exists());
    assert!(root.join(".graxus/code/codemap.json").exists());
    assert!(root.join(".graxus/code/symbols.json").exists());
    assert!(root.join(".graxus/docs/graph.json").exists());
    assert!(root.join(".graxus/docs/nodes.json").exists());
    assert!(root.join(".graxus/docs/edges.json").exists());
    assert!(root.join(".graxus/index.db").exists());
}