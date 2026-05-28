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
        if imp.resolved_file.is_some() {
            continue;
        }

        let result = match imp.language.as_str() {
            "rust" => resolve_rust(&imp.source, &imp.file, &file_set, &by_stem),
            "typescript" | "javascript" => resolve_ts(&imp.source, &imp.file, &file_set, &by_stem),
            "python" => resolve_python(&imp.source, &imp.file, &file_set, &by_stem),
            "go" => resolve_go(&imp.source, &imp.file, &file_set, &by_stem),
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
        if let Some(resolved) = resolve_rust_segments(&segments, &file_dir.to_string_lossy(), file_set) {
            return Some((resolved, ConfidenceScore::named_import_exact()));
        }
    }

    // self::bar → current dir
    if let Some(rest) = path_str.strip_prefix("self::") {
        let file_dir = Path::new(file).parent().unwrap_or(Path::new("."));
        let segments: Vec<&str> = rest.split("::").collect();
        if let Some(resolved) = resolve_rust_segments(&segments, &file_dir.to_string_lossy(), file_set) {
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
    let candidate = format!("{}/mod.rs", format!("{}/{}", base, path));
    if file_set.contains(candidate.as_str()) {
        return Some(normalize_path(&candidate));
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

    // Skip alias paths like @/...
    if src.starts_with('@') {
        return None;
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
    let stem = src.split('/').last().unwrap_or(src);
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
        let candidate = normalize_path(&base.join(format!("{}/__init__.py", module)).to_string_lossy());
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
            if f_path.parent().map(|p| normalize_path(&p.to_string_lossy()) == normalize_path(&dir.to_string_lossy())).unwrap_or(false) {
                if f.ends_with(".go") {
                    return Some((f.to_string(), ConfidenceScore::named_import_exact()));
                }
            }
        }

        return None;
    }

    // Module import: github.com/org/pkg → match by package name
    if let Some(last) = src.split('/').last() {
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

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .replace("//", "/")
        .to_string()
}
