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
