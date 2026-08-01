use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use graxus_codemap::CodemapBuilder;
use graxus_core::{FileKind, Language, ParserBackend, ScannedFile};

fn scanned(path: &Path, relative_path: &str, hash: &str) -> ScannedFile {
    ScannedFile {
        path: path.to_path_buf(),
        relative_path: relative_path.to_string(),
        kind: FileKind::Code,
        language: Language::Rust,
        hash: hash.to_string(),
        size: std::fs::metadata(path).unwrap().len(),
        modified: chrono::Utc::now(),
    }
}

fn temp_project() -> PathBuf {
    std::env::temp_dir().join(format!(
        "graxus-codemap-merge-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn merge_reresolves_changed_importer_against_unchanged_target() {
    let root = temp_project();
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let target_path = src.join("handler.rs");
    let caller_path = src.join("main.rs");
    std::fs::write(&target_path, "pub fn run() {}\n").unwrap();
    std::fs::write(
        &caller_path,
        "use crate::handler::run;\nfn main() { run(); }\n",
    )
    .unwrap();

    let mut graph = CodemapBuilder::new(vec![
        scanned(&target_path, "src/handler.rs", "target-v1"),
        scanned(&caller_path, "src/main.rs", "caller-v1"),
    ])
    .with_backend(ParserBackend::TreeSitter)
    .build()
    .unwrap();
    assert_eq!(
        graph.imports[0].resolved_file.as_deref(),
        Some("src/handler.rs")
    );
    assert_eq!(
        graph.calls[0].resolved_symbol.as_deref(),
        Some("src/handler.rs::run")
    );

    // Rebuild only the changed caller. Its import and call cannot resolve from
    // this partial graph until merge reuses the unchanged target facts.
    std::fs::write(
        &caller_path,
        "use crate::handler::run;\nfn main() { run(); run(); }\n",
    )
    .unwrap();
    let changed = CodemapBuilder::new(vec![scanned(&caller_path, "src/main.rs", "caller-v2")])
        .with_backend(ParserBackend::TreeSitter)
        .build()
        .unwrap();
    assert_eq!(changed.calls.len(), 2);
    assert!(changed.imports[0].resolved_file.is_none());
    assert!(changed.calls[0].resolved_symbol.is_none());

    graph.merge(changed);

    let import = graph
        .imports
        .iter()
        .find(|import| import.file == "src/main.rs")
        .unwrap();
    assert_eq!(import.resolved_file.as_deref(), Some("src/handler.rs"));
    assert!(graph
        .calls
        .iter()
        .filter(|call| call.file == "src/main.rs")
        .all(|call| call.resolved_symbol.as_deref() == Some("src/handler.rs::run")));
    let call = graph
        .calls
        .iter()
        .find(|call| call.file == "src/main.rs")
        .unwrap();
    assert_eq!(call.resolved_symbol.as_deref(), Some("src/handler.rs::run"));
    assert_eq!(
        graph
            .edges
            .iter()
            .filter(|edge| {
                edge.from == "src/main.rs"
                    && edge.to == "src/handler.rs"
                    && edge.edge_type == graxus_codemap::CodeEdgeType::Imports
            })
            .count(),
        1
    );
    assert_eq!(
        graph
            .edges
            .iter()
            .filter(|edge| {
                edge.from == "src/main.rs"
                    && edge.to == "src/handler.rs::run"
                    && edge.edge_type == graxus_codemap::CodeEdgeType::Calls
            })
            .count(),
        1
    );

    std::fs::remove_dir_all(root).unwrap();
}
