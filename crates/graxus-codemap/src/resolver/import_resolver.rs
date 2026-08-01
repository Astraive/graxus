use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::{ConfidenceScore, FileNode, ImportFact};

/// Resolve import source paths to actual file paths.
/// Modifies imports in-place, setting `resolved_file` and `confidence`.
pub fn resolve_imports(imports: &mut [ImportFact], file_nodes: &[FileNode]) {
    // Build lookup sets
    let file_set: HashSet<&str> = file_nodes.iter().map(|f| f.path.as_str()).collect();
    let mut by_stem: HashMap<String, Vec<&str>> = HashMap::new();
    for f in file_nodes {
        let stem = Path::new(&f.path)
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        by_stem.entry(stem).or_default().push(&f.path);
    }

    for imp in imports.iter_mut() {
        // Reuse a previous resolution only while its target file is still in
        // the graph. Removed files otherwise leave stale import edges after
        // an incremental merge.
        if let Some(resolved) = imp.resolved_file.as_deref() {
            if file_set.contains(resolved) {
                continue;
            }
            imp.resolved_file = None;
            imp.confidence = ConfidenceScore::unresolved();
        }

        let result = match imp.language.as_str() {
            "rust" => resolve_rust(&imp.source, &imp.file, &file_set, &by_stem),
            "typescript" | "javascript" => resolve_ts(&imp.source, &imp.file, &file_set, &by_stem),
            "python" => resolve_python(&imp.source, &imp.file, &file_set, &by_stem),
            "go" => resolve_go(&imp.source, &imp.file, &file_set, &by_stem),
            "c" | "cpp" => resolve_c_cpp(&imp.source, &imp.file, &file_set, &by_stem),
            _ => None,
        };

        if let Some((path, confidence)) = result {
            imp.resolved_file = Some(path);
            imp.confidence = confidence;
        }
    }
}

fn resolve_rust(
    source: &str,
    file: &str,
    file_set: &HashSet<&str>,
    _by_stem: &HashMap<String, Vec<&str>>,
) -> Option<(String, ConfidenceScore)> {
    let path_str = source.trim();

    // crate::foo::bar → src/foo/bar.rs or src/foo/bar/mod.rs
    if let Some(rest) = path_str.strip_prefix("crate::") {
        let segments: Vec<&str> = rest.split("::").collect();
        if let Some(resolved) = resolve_rust_segments(&segments, "src", file_set) {
            return Some((resolved, ConfidenceScore::named_import_exact()));
        }
    }

    // super::bar → parent dir
    if let Some(rest) = path_str.strip_prefix("super::") {
        let file_dir = Path::new(file).parent().unwrap_or(Path::new("."));
        let segments: Vec<&str> = rest.split("::").collect();
        if let Some(resolved) =
            resolve_rust_segments(&segments, &file_dir.to_string_lossy(), file_set)
        {
            return Some((resolved, ConfidenceScore::named_import_exact()));
        }
    }

    // self::bar → current dir
    if let Some(rest) = path_str.strip_prefix("self::") {
        let file_dir = Path::new(file).parent().unwrap_or(Path::new("."));
        let segments: Vec<&str> = rest.split("::").collect();
        if let Some(resolved) =
            resolve_rust_segments(&segments, &file_dir.to_string_lossy(), file_set)
        {
            return Some((resolved, ConfidenceScore::named_import_exact()));
        }
    }

    // Bare path: foo::bar — try src/foo/bar.rs
    let segments: Vec<&str> = path_str.split("::").collect();
    if segments.len() > 1 {
        if let Some(resolved) = resolve_rust_segments(&segments, "src", file_set) {
            return Some((resolved, ConfidenceScore::path_match_only()));
        }
    }

    // Single segment: just a crate name — skip external crates
    None
}

fn resolve_rust_segments(
    segments: &[&str],
    base: &str,
    file_set: &HashSet<&str>,
) -> Option<String> {
    if segments.is_empty() {
        return None;
    }

    // Handle wildcard imports (use foo::*)
    let segments: Vec<&str> = if segments.last() == Some(&"*") {
        segments[..segments.len() - 1].to_vec()
    } else {
        segments.to_vec()
    };

    let path = segments.join("/");

    // Try direct file: base/path.rs
    let candidate = format!("{}/{}.rs", base, path);
    if file_set.contains(candidate.as_str()) {
        return Some(normalize_path(&candidate));
    }

    // Try module file: base/path/mod.rs
    let candidate = format!("{}/{}/mod.rs", base, path);
    if file_set.contains(candidate.as_str()) {
        return Some(normalize_path(&candidate));
    }

    // A `use` path usually ends with an item name rather than a module name
    // (`crate::handler::run` resolves to `src/handler.rs`). Prefer the module
    // containing the final item after trying the full path above.
    if segments.len() > 1 {
        let module_path = segments[..segments.len() - 1].join("/");
        let candidate = format!("{}/{}.rs", base, module_path);
        if file_set.contains(candidate.as_str()) {
            return Some(normalize_path(&candidate));
        }
        let candidate = format!("{}/{}/mod.rs", base, module_path);
        if file_set.contains(candidate.as_str()) {
            return Some(normalize_path(&candidate));
        }
    }

    // Try just the last segment as a file
    if let Some(last) = segments.last() {
        let candidate = format!("{}/{}.rs", base, last);
        if file_set.contains(candidate.as_str()) {
            return Some(normalize_path(&candidate));
        }
    }

    None
}

fn resolve_ts(
    source: &str,
    file: &str,
    file_set: &HashSet<&str>,
    _by_stem: &HashMap<String, Vec<&str>>,
) -> Option<(String, ConfidenceScore)> {
    let src = source.trim().trim_matches('"').trim_matches('\'');

    // @/ alias → src/ prefix (common in Next.js, Vite, etc.)
    if let Some(rest) = src.strip_prefix("@/") {
        let ts_extensions = [".ts", ".tsx", ".js", ".jsx", ".mts", ".cts"];
        // Try src/rest with extensions
        for ext in &ts_extensions {
            let candidate = format!("src/{}{}", rest, ext);
            if file_set.contains(candidate.as_str()) {
                return Some((candidate, ConfidenceScore::named_import_exact()));
            }
        }
        // Try src/rest/index with extensions
        for ext in &ts_extensions {
            let candidate = format!("src/{}/index{}", rest, ext);
            if file_set.contains(candidate.as_str()) {
                return Some((candidate, ConfidenceScore::named_import_exact()));
            }
        }
    }

    // Relative imports: ./foo or ../foo
    if src.starts_with('.') {
        let file_dir = Path::new(file).parent().unwrap_or(Path::new("."));
        let base = normalize_path(&file_dir.join(src).to_string_lossy());

        // Try exact with various extensions
        let ts_extensions = [".ts", ".tsx", ".js", ".jsx", ".mts", ".cts"];
        for ext in &ts_extensions {
            let candidate = format!("{}{}", base, ext);
            if file_set.contains(candidate.as_str()) {
                return Some((candidate, ConfidenceScore::named_import_exact()));
            }
        }

        // Try index files
        for ext in &ts_extensions {
            let candidate = format!("{}/index{}", base, ext);
            if file_set.contains(candidate.as_str()) {
                return Some((candidate, ConfidenceScore::named_import_exact()));
            }
        }

        return None;
    }

    // Absolute/bare module — try to find by stem
    let stem = src.split('/').next_back().unwrap_or(src);
    let ts_extensions = [".ts", ".tsx", ".js", ".jsx"];
    for ext in &ts_extensions {
        let candidate = format!("{}.{}", stem, ext);
        // Try in common directories
        for prefix in &["src/", "lib/", ""] {
            let full = format!("{}{}", prefix, candidate);
            if file_set.contains(full.as_str()) {
                return Some((full, ConfidenceScore::path_match_only()));
            }
        }
    }

    // Try as path directly
    for ext in &ts_extensions {
        let candidate = format!("{}{}", src, ext);
        if file_set.contains(candidate.as_str()) {
            return Some((candidate, ConfidenceScore::path_match_only()));
        }
    }

    None
}

fn resolve_python(
    source: &str,
    file: &str,
    file_set: &HashSet<&str>,
    _by_stem: &HashMap<String, Vec<&str>>,
) -> Option<(String, ConfidenceScore)> {
    let src = source.trim();

    // Relative import: .foo or ..foo
    if src.starts_with('.') {
        let file_dir = Path::new(file).parent().unwrap_or(Path::new("."));
        let dots = src.chars().take_while(|&c| c == '.').count();
        let module = src[dots..].replace('.', "/");

        // Walk up parent directories based on dot count
        let mut base = file_dir.to_path_buf();
        for _ in 1..dots {
            base = base.parent().unwrap_or(Path::new(".")).to_path_buf();
        }

        // Try module.py
        let candidate = normalize_path(&base.join(format!("{}.py", module)).to_string_lossy());
        if file_set.contains(candidate.as_str()) {
            return Some((candidate, ConfidenceScore::named_import_exact()));
        }

        // Try module/__init__.py
        let candidate = normalize_path(
            &base
                .join(format!("{}/__init__.py", module))
                .to_string_lossy(),
        );
        if file_set.contains(candidate.as_str()) {
            return Some((candidate, ConfidenceScore::named_import_exact()));
        }

        return None;
    }

    // Absolute import: foo.bar → foo/bar.py
    let module_path = src.replace('.', "/");
    let candidates = vec![
        format!("{}.py", module_path),
        format!("{}/__init__.py", module_path),
        format!("src/{}.py", module_path),
        format!("src/{}/__init__.py", module_path),
    ];
    for candidate in candidates {
        if file_set.contains(candidate.as_str()) {
            return Some((candidate, ConfidenceScore::path_match_only()));
        }
    }

    None
}

fn resolve_go(
    source: &str,
    file: &str,
    file_set: &HashSet<&str>,
    _by_stem: &HashMap<String, Vec<&str>>,
) -> Option<(String, ConfidenceScore)> {
    let src = source.trim().trim_matches('"');

    // Relative import: ./pkg
    if src.starts_with('.') {
        let file_dir = Path::new(file).parent().unwrap_or(Path::new("."));
        let pkg = src.trim_start_matches('.').trim_start_matches('/');
        let dir = file_dir.join(pkg);

        // Look for any .go file in that directory
        for f in file_set.iter() {
            let f_path = Path::new(f);
            if f_path
                .parent()
                .map(|p| {
                    normalize_path(&p.to_string_lossy()) == normalize_path(&dir.to_string_lossy())
                })
                .unwrap_or(false)
                && f.ends_with(".go")
            {
                return Some((f.to_string(), ConfidenceScore::named_import_exact()));
            }
        }

        return None;
    }

    // Module import: github.com/org/pkg → match by package name
    if let Some(last) = src.split('/').next_back() {
        // Try to find a directory with that name containing .go files
        for f in file_set.iter() {
            if f.ends_with(".go") {
                let parent = Path::new(f)
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if parent == last {
                    return Some((f.to_string(), ConfidenceScore::same_project()));
                }
            }
        }
    }

    None
}

/// Resolve C/C++ `#include` directives.
///
/// Quote includes (`"foo.h"`, `"utils/parser.h"`) are project-local; angle-
/// bracket includes (`<stdio.h>`, `<vector>`) are system/stdlib headers that
/// are never part of the project and return `None`.
///
/// Resolution strategy: the include path (minus quotes) is matched against
/// (a) the exact relative path in the file set, (b) the same path relative to
/// the importing file's directory, and (c) by basename stem (e.g. `"parser.h"`
/// matches `src/utils/parser.h`).
fn resolve_c_cpp(
    source: &str,
    file: &str,
    file_set: &HashSet<&str>,
    by_stem: &HashMap<String, Vec<&str>>,
) -> Option<(String, ConfidenceScore)> {
    let raw = source.trim();

    // Angle-bracket includes are system headers — never resolvable.
    if raw.starts_with('<') && raw.ends_with('>') {
        return None;
    }

    // Strip quotes: `"foo.h"` → `foo.h`.
    let path_str = raw
        .trim_start_matches('"')
        .trim_end_matches('"')
        .trim_start_matches('\'')
        .trim_end_matches('\'');

    if path_str.is_empty() || path_str.starts_with('<') {
        return None;
    }

    let normalized = normalize_path(path_str);

    // (a) Exact relative-path match.
    if file_set.contains(normalized.as_str()) {
        return Some((normalized, ConfidenceScore::named_import_exact()));
    }

    // (b) Relative to the importing file's directory.
    let file_dir = Path::new(file).parent().unwrap_or(Path::new("."));
    let relative = normalize_path(&file_dir.join(&normalized).to_string_lossy());
    if file_set.contains(relative.as_str()) {
        return Some((relative, ConfidenceScore::named_import_exact()));
    }

    // (c) Match by basename stem (e.g. include `"parser.h"` → `src/utils/parser.h`).
    let stem = Path::new(&normalized)
        .file_stem()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if !stem.is_empty() {
        if let Some(matches) = by_stem.get(&stem) {
            // Prefer a match whose extension matches the include's extension.
            let want_header = normalized.ends_with(".h")
                || normalized.ends_with(".hpp")
                || normalized.ends_with(".hxx");
            for &candidate in matches {
                let is_header = candidate.ends_with(".h")
                    || candidate.ends_with(".hpp")
                    || candidate.ends_with(".hxx");
                if want_header == is_header {
                    return Some((candidate.to_string(), ConfidenceScore::path_match_only()));
                }
            }
            // Fall back to the first stem match regardless of extension.
            return Some((matches[0].to_string(), ConfidenceScore::path_match_only()));
        }
    }

    None
}

fn normalize_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    let mut components = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            component => components.push(component),
        }
    }
    components.join("/")
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImportKind, ResolutionMethod};

    fn file(path: &str) -> FileNode {
        FileNode {
            path: path.to_string(),
            language: "rust".to_string(),
            hash: String::new(),
            size: 0,
        }
    }

    fn import(source: &str, resolved_file: Option<&str>) -> ImportFact {
        ImportFact {
            id: "import:test".to_string(),
            file: "src/main.rs".to_string(),
            language: "rust".to_string(),
            kind: ImportKind::RustUse,
            source: source.to_string(),
            local_name: Some("run".to_string()),
            imported_name: None,
            resolved_file: resolved_file.map(str::to_string),
            line: 1,
            confidence: resolved_file
                .map(|_| ConfidenceScore::named_import_exact())
                .unwrap_or_default(),
        }
    }

    #[test]
    fn resolves_rust_item_import_to_unchanged_module_file() {
        let mut imports = vec![import("crate::handler::run", None)];
        resolve_imports(&mut imports, &[file("src/main.rs"), file("src/handler.rs")]);

        assert_eq!(imports[0].resolved_file.as_deref(), Some("src/handler.rs"));
        assert_eq!(
            imports[0].confidence.method,
            ResolutionMethod::NamedImportExactExport
        );
    }

    #[test]
    fn normalizes_relative_typescript_import_paths() {
        let mut imports = vec![ImportFact {
            id: "import:ts".to_string(),
            file: "src/main.ts".to_string(),
            language: "typescript".to_string(),
            kind: ImportKind::NamedImport,
            source: "./handler".to_string(),
            local_name: Some("run".to_string()),
            imported_name: Some("run".to_string()),
            resolved_file: None,
            line: 1,
            confidence: ConfidenceScore::unresolved(),
        }];
        resolve_imports(&mut imports, &[file("src/main.ts"), file("src/handler.ts")]);

        assert_eq!(imports[0].resolved_file.as_deref(), Some("src/handler.ts"));
    }

    #[test]
    fn clears_import_resolution_when_target_file_disappears() {
        let mut imports = vec![import("crate::handler", Some("src/handler.rs"))];
        resolve_imports(&mut imports, &[file("src/main.rs")]);

        assert!(imports[0].resolved_file.is_none());
        assert_eq!(imports[0].confidence, ConfidenceScore::unresolved());
    }
}
