use graxus_codemap::{frameworks, CodemapBuilder};
use graxus_core::{FileKind, Language, ParserBackend, ScannedFile};

#[test]
fn exposes_framework_scaffolding_for_big_five_ecosystems() {
    let names: Vec<_> = frameworks::supported_frameworks()
        .iter()
        .map(|item| item.name)
        .collect();
    assert!(names.contains(&"fastapi"));
    assert!(names.contains(&"axum"));
    assert!(names.contains(&"gin"));
    assert!(names.contains(&"nestjs"));
    assert!(names.contains(&"aspnet"));
    assert!(names.contains(&"drogon"));
}

fn scanned(path: std::path::PathBuf, relative_path: &str, language: Language) -> ScannedFile {
    let size = std::fs::metadata(&path).unwrap().len();
    ScannedFile {
        path,
        relative_path: relative_path.to_string(),
        kind: FileKind::Code,
        language,
        hash: "test".to_string(),
        size,
        modified: chrono::Utc::now(),
    }
}

#[cfg(feature = "ripex")]
#[test]
fn ripex_is_primary_and_tree_sitter_is_the_per_file_fallback() {
    let root = std::env::temp_dir().join(format!(
        "graxus-ripex-e2e-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let tsx_path = root.join("view.tsx");
    let java_path = root.join("Service.java");
    std::fs::write(
        &tsx_path,
        "export const view = <section><span>Hello</span></section>;",
    )
    .unwrap();
    std::fs::write(&java_path, "public class Service { void run() {} }").unwrap();

    let graph = CodemapBuilder::new(vec![
        scanned(tsx_path, "view.tsx", Language::TypeScript),
        scanned(java_path, "Service.java", Language::Java),
    ])
    .with_backend(ParserBackend::Ripex)
    .build()
    .unwrap();
    let _ = std::fs::remove_dir_all(&root);

    let tsx = graph.parser_result_for_file("view.tsx").unwrap();
    assert_eq!(tsx.requested_backend, ParserBackend::Ripex);
    assert_eq!(tsx.used_backend, ParserBackend::Ripex);
    assert!(tsx.fallback_reason.is_none());
    assert!(!tsx.facts.is_empty());
    assert!(graph.parser_fact(&tsx.facts[0].id).is_some());

    let java = graph.parser_result_for_file("Service.java").unwrap();
    assert_eq!(java.requested_backend, ParserBackend::Ripex);
    assert_eq!(java.used_backend, ParserBackend::TreeSitter);
    assert!(java
        .fallback_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("no parser")));

    let json = serde_json::to_string(&graph).unwrap();
    let restored: graxus_codemap::CodeGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(
        restored
            .parser_result_for_file("view.tsx")
            .unwrap()
            .used_backend,
        ParserBackend::Ripex
    );
}

#[cfg(feature = "ripex")]
#[test]
fn ripex_assigns_unique_ids_to_duplicate_symbols_and_parser_facts() {
    let root = std::env::temp_dir().join(format!(
        "graxus-ripex-duplicate-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("service.ts");
    std::fs::write(
        &path,
        "class Service { run(value: string): string { return value; } run(value: number): number { return value.toString(); } }",
    )
    .unwrap();

    let graph = CodemapBuilder::new(vec![scanned(path, "service.ts", Language::TypeScript)])
        .with_backend(ParserBackend::Ripex)
        .build()
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);

    let symbols = graph
        .symbols
        .iter()
        .filter(|symbol| symbol.name == "Service.run")
        .collect::<Vec<_>>();
    assert!(symbols.len() >= 2);
    assert_ne!(symbols[0].id, symbols[1].id);
    assert!(symbols
        .iter()
        .all(|symbol| graph.parser_fact(&symbol.id).is_some()));
}

#[test]
fn confidence_scores_normalize_non_finite_and_out_of_range_values() {
    let high =
        graxus_codemap::ConfidenceScore::new(150.0, graxus_codemap::ResolutionMethod::SyntaxOnly);
    assert_eq!(high.score, 100.0);
    assert_eq!(high.label, graxus_codemap::ConfidenceLabel::Exact);

    let invalid = graxus_codemap::ConfidenceScore::new(
        f64::NAN,
        graxus_codemap::ResolutionMethod::SyntaxOnly,
    );
    assert_eq!(invalid.score, 0.0);
    assert_eq!(invalid.label, graxus_codemap::ConfidenceLabel::Unresolved);
}

#[test]
fn removing_a_file_removes_edges_to_its_symbols_and_path() {
    let mut graph = graxus_codemap::CodeGraph {
        edges: vec![
            graxus_codemap::CodeEdge {
                from: "src/changed.rs".into(),
                to: "src/other.rs".into(),
                edge_type: graxus_codemap::CodeEdgeType::Imports,
            },
            graxus_codemap::CodeEdge {
                from: "src/other.rs".into(),
                to: "src/changed.rs::run".into(),
                edge_type: graxus_codemap::CodeEdgeType::Calls,
            },
            graxus_codemap::CodeEdge {
                from: "src/other.rs::keep".into(),
                to: "src/other.rs".into(),
                edge_type: graxus_codemap::CodeEdgeType::DefinedIn,
            },
        ],
        ..Default::default()
    };

    graph.remove_file("src/changed.rs");

    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].from, "src/other.rs::keep");
}

#[test]
fn calls_to_symbol_accepts_canonical_symbol_ids() {
    let symbol = graxus_codemap::SymbolFact {
        id: "symbol:src/lib.rs:run:1:0".into(),
        file: "src/lib.rs".into(),
        name: "run".into(),
        ..Default::default()
    };
    let call = graxus_codemap::CallFact {
        id: "call:src/main.rs:4:0".into(),
        file: "src/main.rs".into(),
        language: "rust".into(),
        kind: graxus_codemap::CallKind::FunctionCall,
        caller_symbol: None,
        callee_text: "run".into(),
        object: None,
        resolved_symbol: Some("src/lib.rs::run".into()),
        line: 4,
        column: 0,
        confidence: graxus_codemap::ConfidenceScore::local_definition(),
    };
    let mut graph = graxus_codemap::CodeGraph::default();
    graph.symbols.push(symbol);
    graph.calls.push(call);

    assert_eq!(graph.calls_to_symbol("symbol:src/lib.rs:run:1:0").len(), 1);
}
