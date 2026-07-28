use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Information about a detected workspace (Cargo, npm, or Go).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub name: String,
    pub root: PathBuf,
    pub kind: WorkspaceKind,
}

/// The type of workspace detected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkspaceKind {
    CargoWorkspace,
    NpmWorkspace,
    GoModule,
    Unknown,
}

/// Information about a graxus workspace (monorepo or single project).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraxusWorkspace {
    /// Whether this is a monorepo (has workspace markers).
    pub is_monorepo: bool,
    /// Root directory of the workspace.
    pub root: PathBuf,
    /// Sub-project paths (relative to root).
    pub sub_projects: Vec<PathBuf>,
    /// Languages detected across all sub-projects.
    pub languages: Vec<String>,
    /// The type of workspace (Cargo, npm, Go, etc.).
    pub kind: WorkspaceKind,
}

/// Detect workspace structure from config files in the project root.
pub fn detect_workspaces(root: &Path) -> Vec<WorkspaceInfo> {
    let mut workspaces = Vec::new();

    // Check for Cargo workspace
    if root.join("Cargo.toml").exists() {
        if let Ok(content) = std::fs::read_to_string(root.join("Cargo.toml")) {
            if has_toml_section(&content, "workspace") {
                workspaces.push(WorkspaceInfo {
                    name: root
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    root: root.to_path_buf(),
                    kind: WorkspaceKind::CargoWorkspace,
                });
            }
        }
    }

    // Check for npm workspace
    if root.join("package.json").exists() {
        if let Ok(content) = std::fs::read_to_string(root.join("package.json")) {
            if content.contains("\"workspaces\"") {
                workspaces.push(WorkspaceInfo {
                    name: root
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    root: root.to_path_buf(),
                    kind: WorkspaceKind::NpmWorkspace,
                });
            }
        }
    }

    // Check for Go module
    if root.join("go.mod").exists() {
        workspaces.push(WorkspaceInfo {
            name: root
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            root: root.to_path_buf(),
            kind: WorkspaceKind::GoModule,
        });
    }

    // Recursively check subdirectories (max depth 2)
    detect_sub_workspaces(root, &mut workspaces, 0);

    workspaces
}

fn detect_sub_workspaces(dir: &Path, workspaces: &mut Vec<WorkspaceInfo>, depth: usize) {
    if depth >= 2 {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }

        // Check for nested Cargo workspace
        if path.join("Cargo.toml").exists() {
            if let Ok(content) = std::fs::read_to_string(path.join("Cargo.toml")) {
                if has_toml_section(&content, "workspace") {
                    workspaces.push(WorkspaceInfo {
                        name: name.clone(),
                        root: path.clone(),
                        kind: WorkspaceKind::CargoWorkspace,
                    });
                    continue;
                }
            }
        }

        // Check for nested npm workspace
        if path.join("package.json").exists() {
            if let Ok(content) = std::fs::read_to_string(path.join("package.json")) {
                if content.contains("\"workspaces\"") {
                    workspaces.push(WorkspaceInfo {
                        name: name.clone(),
                        root: path.clone(),
                        kind: WorkspaceKind::NpmWorkspace,
                    });
                    continue;
                }
            }
        }

        // Check for nested Go module
        if path.join("go.mod").exists() {
            workspaces.push(WorkspaceInfo {
                name: name.clone(),
                root: path.clone(),
                kind: WorkspaceKind::GoModule,
            });
            continue;
        }

        detect_sub_workspaces(&path, workspaces, depth + 1);
    }
}

/// Get which workspace a file belongs to, if any.
pub fn file_workspace<'a>(
    file_path: &str,
    workspaces: &'a [WorkspaceInfo],
) -> Option<&'a WorkspaceInfo> {
    workspaces
        .iter()
        .filter(|w| file_path.starts_with(w.root.to_string_lossy().as_ref()))
        .max_by_key(|w| w.root.to_string_lossy().len())
}

/// Detect if the given root is a monorepo and gather workspace information.
///
/// Checks for:
/// - Cargo workspaces (`[workspace]` in Cargo.toml)
/// - npm workspaces (`"workspaces"` in package.json)
/// - Go workspaces (go.work)
/// - Nested graxus.yaml files (sub-projects with their own graxus config)
pub fn detect_workspace(root: &Path) -> GraxusWorkspace {
    let mut kind = WorkspaceKind::Unknown;

    // Check Cargo workspace
    if root.join("Cargo.toml").exists() {
        if let Ok(content) = std::fs::read_to_string(root.join("Cargo.toml")) {
            if has_toml_section(&content, "workspace") {
                kind = WorkspaceKind::CargoWorkspace;
            }
        }
    }

    // Check npm workspace
    if kind == WorkspaceKind::Unknown && root.join("package.json").exists() {
        if let Ok(content) = std::fs::read_to_string(root.join("package.json")) {
            if content.contains("\"workspaces\"") {
                kind = WorkspaceKind::NpmWorkspace;
            }
        }
    }

    // Check Go workspace
    if kind == WorkspaceKind::Unknown && root.join("go.work").exists() {
        kind = WorkspaceKind::GoModule;
    }

    let sub_projects = list_subprojects(root);
    let languages = detect_languages(root, &sub_projects);

    GraxusWorkspace {
        is_monorepo: !sub_projects.is_empty(),
        root: root.to_path_buf(),
        sub_projects,
        languages,
        kind,
    }
}

/// List all sub-projects in the workspace.
///
/// A sub-project is a directory that contains:
/// - A `graxus.yaml` file (graxus sub-project)
/// - A `Cargo.toml` with a `[package]` section (Rust sub-project)
/// - A `package.json` with a `"name"` field (JS/TS sub-project)
/// - A `go.mod` file (Go sub-project)
///
/// Scans up to 3 levels deep, skipping common non-project directories.
pub fn list_subprojects(root: &Path) -> Vec<PathBuf> {
    let mut projects = Vec::new();
    scan_subprojects(root, &mut projects, 0);
    projects.sort();
    projects.dedup();
    projects
}

fn scan_subprojects(dir: &Path, projects: &mut Vec<PathBuf>, depth: usize) {
    if depth >= 3 {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.starts_with('.')
            || name == "target"
            || name == "node_modules"
            || name == "dist"
            || name == "build"
            || name == "vendor"
        {
            continue;
        }

        // Check for graxus.yaml (strongest signal — this is a graxus sub-project)
        if path.join("graxus.yaml").exists() && path != dir {
            projects.push(path.clone());
            continue;
        }

        // Check for Cargo.toml with [package] (not a workspace root)
        if path.join("Cargo.toml").exists() {
            if let Ok(content) = std::fs::read_to_string(path.join("Cargo.toml")) {
                let has_package = content.lines().any(|l| {
                    let stripped = l.split('#').next().unwrap_or(l);
                    stripped.contains("[package]")
                });
                if has_package && !has_toml_section(&content, "workspace") {
                    projects.push(path.clone());
                    continue;
                }
            }
        }

        // Check for package.json with "name" (not just a workspace root)
        if path.join("package.json").exists() {
            if let Ok(content) = std::fs::read_to_string(path.join("package.json")) {
                if content.contains("\"name\"") {
                    projects.push(path.clone());
                    continue;
                }
            }
        }

        // Check for go.mod
        if path.join("go.mod").exists() {
            projects.push(path.clone());
            continue;
        }

        // Recurse into subdirectories
        scan_subprojects(&path, projects, depth + 1);
    }
}

/// Detect languages used across the workspace by scanning file extensions.
fn detect_languages(root: &Path, sub_projects: &[PathBuf]) -> Vec<String> {
    use std::collections::BTreeSet;
    let mut languages = BTreeSet::new();

    // Check root-level files
    if root.join("Cargo.toml").exists() {
        languages.insert("rust".to_string());
    }
    if root.join("package.json").exists() {
        languages.insert("javascript".to_string());
    }
    if root.join("go.mod").exists() {
        languages.insert("go".to_string());
    }
    if root.join("requirements.txt").exists() || root.join("pyproject.toml").exists() {
        languages.insert("python".to_string());
    }

    // Check sub-project files
    for project in sub_projects {
        if project.join("Cargo.toml").exists() {
            languages.insert("rust".to_string());
        }
        if project.join("package.json").exists() {
            languages.insert("javascript".to_string());
            if project.join("tsconfig.json").exists() {
                languages.insert("typescript".to_string());
            }
        }
        if project.join("go.mod").exists() {
            languages.insert("go".to_string());
        }
        if project.join("requirements.txt").exists() || project.join("pyproject.toml").exists() {
            languages.insert("python".to_string());
        }
    }

    languages.into_iter().collect()
}

/// Check if a TOML string contains a `[section_name]` header (not in comments).
///
/// Strips single-line comments (`# ...`) before checking. Handles `#` inside
/// quoted strings by tracking whether we're inside a string context.
fn has_toml_section(content: &str, section_name: &str) -> bool {
    let pattern = format!("[{}]", section_name);
    for line in content.lines() {
        let stripped = strip_toml_comment(line);
        if stripped.contains(&pattern) {
            return true;
        }
    }
    false
}

/// Strip a TOML inline comment from a line, respecting quoted strings.
///
/// A `#` character starts a comment unless it's inside a basic (`"..."`) or
/// literal (`'...'`) string. This is a best-effort parser — it doesn't handle
/// multi-line strings, but those don't appear in simple Cargo.toml/package.json
/// sections we care about.
fn strip_toml_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut string_char = b'"';
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if b == b'\\' {
                i += 2; // skip escaped char
                continue;
            }
            if b == string_char {
                in_string = false;
            }
        } else if b == b'"' || b == b'\'' {
            in_string = true;
            string_char = b;
        } else if b == b'#' {
            return &line[..i];
        }
        i += 1;
    }
    line
}

/// Check if a .graxus directory exists and has been indexed for a given path.
pub fn is_indexed(project_path: &Path) -> bool {
    let graxus_dir = project_path.join(".graxus");
    if !graxus_dir.is_dir() {
        return false;
    }
    // Check for any index artifacts
    graxus_dir.join("files.json").exists()
        || graxus_dir.join("code").join("codemap.json").exists()
        || graxus_dir.join("index.db").exists()
}

/// Check if the index for a project is stale (files changed since last index).
///
/// Uses the `ignore` crate's walker to respect `.gitignore` rules, matching
/// the behavior of the main scanner.
pub fn is_stale(project_path: &Path) -> bool {
    let files_json = project_path.join(".graxus").join("files.json");
    if !files_json.exists() {
        return true; // No index at all
    }

    let index_modified = match std::fs::metadata(&files_json) {
        Ok(m) => m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        Err(_) => return true,
    };

    // Use the ignore crate's walker to respect .gitignore rules
    let walker = ignore::WalkBuilder::new(project_path)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .hidden(false)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        // Skip the .graxus directory itself
        if path.starts_with(project_path.join(".graxus")) {
            continue;
        }
        if let Ok(meta) = std::fs::metadata(path) {
            if let Ok(modified) = meta.modified() {
                if modified > index_modified {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_workspace_cargo_monorepo() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create a Cargo workspace
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();

        // Create sub-projects
        let crate_a = root.join("crates").join("a");
        std::fs::create_dir_all(&crate_a).unwrap();
        std::fs::write(crate_a.join("Cargo.toml"), "[package]\nname = \"a\"\n").unwrap();

        let crate_b = root.join("crates").join("b");
        std::fs::create_dir_all(&crate_b).unwrap();
        std::fs::write(crate_b.join("Cargo.toml"), "[package]\nname = \"b\"\n").unwrap();

        let ws = detect_workspace(root);
        assert!(ws.is_monorepo);
        assert_eq!(ws.sub_projects.len(), 2);
        assert!(ws.languages.contains(&"rust".to_string()));
    }

    #[test]
    fn test_detect_workspace_npm_monorepo() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create an npm workspace
        std::fs::write(
            root.join("package.json"),
            r#"{"name": "my-monorepo", "workspaces": ["packages/*"]}"#,
        )
        .unwrap();

        // Create sub-projects
        let pkg_a = root.join("packages").join("a");
        std::fs::create_dir_all(&pkg_a).unwrap();
        std::fs::write(pkg_a.join("package.json"), r#"{"name": "@my/a"}"#).unwrap();

        let ws = detect_workspace(root);
        assert!(ws.is_monorepo);
        assert_eq!(ws.sub_projects.len(), 1);
        assert!(ws.languages.contains(&"javascript".to_string()));
    }

    #[test]
    fn test_detect_workspace_go_monorepo() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create a Go workspace
        std::fs::write(root.join("go.work"), "go 1.21\n").unwrap();

        // Create sub-projects
        let svc = root.join("services").join("api");
        std::fs::create_dir_all(&svc).unwrap();
        std::fs::write(svc.join("go.mod"), "module example.com/api\n").unwrap();

        let ws = detect_workspace(root);
        assert!(ws.is_monorepo);
        assert_eq!(ws.sub_projects.len(), 1);
        assert!(ws.languages.contains(&"go".to_string()));
    }

    #[test]
    fn test_detect_workspace_single_project() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Single project with no sub-projects
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"single\"\n").unwrap();

        let ws = detect_workspace(root);
        assert!(!ws.is_monorepo);
        assert!(ws.sub_projects.is_empty());
    }

    #[test]
    fn test_detect_workspace_graxus_subprojects() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Sub-projects with their own graxus.yaml
        let sub_a = root.join("backend");
        std::fs::create_dir_all(&sub_a).unwrap();
        std::fs::write(sub_a.join("graxus.yaml"), "project:\n  name: backend\n").unwrap();

        let sub_b = root.join("frontend");
        std::fs::create_dir_all(&sub_b).unwrap();
        std::fs::write(sub_b.join("graxus.yaml"), "project:\n  name: frontend\n").unwrap();

        let ws = detect_workspace(root);
        assert!(ws.is_monorepo);
        assert_eq!(ws.sub_projects.len(), 2);
    }

    #[test]
    fn test_list_subprojects_empty() {
        let dir = tempdir().unwrap();
        let projects = list_subprojects(dir.path());
        assert!(projects.is_empty());
    }

    #[test]
    fn test_list_subprojects_skips_hidden() {
        let dir = tempdir().unwrap();

        // Create a hidden directory with Cargo.toml
        let hidden = dir.path().join(".hidden");
        std::fs::create_dir_all(&hidden).unwrap();
        std::fs::write(hidden.join("Cargo.toml"), "[package]\nname = \"hidden\"\n").unwrap();

        // Create a visible sub-project
        let visible = dir.path().join("visible");
        std::fs::create_dir_all(&visible).unwrap();
        std::fs::write(
            visible.join("Cargo.toml"),
            "[package]\nname = \"visible\"\n",
        )
        .unwrap();

        let projects = list_subprojects(dir.path());
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0], visible);
    }

    #[test]
    fn test_list_subprojects_skips_target_node_modules() {
        let dir = tempdir().unwrap();

        let target = dir.path().join("target").join("debug");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(
            target.join("Cargo.toml"),
            "[package]\nname = \"not-real\"\n",
        )
        .unwrap();

        let nm = dir.path().join("node_modules").join("pkg");
        std::fs::create_dir_all(&nm).unwrap();
        std::fs::write(nm.join("package.json"), r#"{"name": "not-real"}"#).unwrap();

        let projects = list_subprojects(dir.path());
        assert!(projects.is_empty());
    }

    #[test]
    fn test_is_indexed_false_no_graxus_dir() {
        let dir = tempdir().unwrap();
        assert!(!is_indexed(dir.path()));
    }

    #[test]
    fn test_is_indexed_true_with_files_json() {
        let dir = tempdir().unwrap();
        let graxus = dir.path().join(".graxus");
        std::fs::create_dir_all(&graxus).unwrap();
        std::fs::write(graxus.join("files.json"), "[]").unwrap();
        assert!(is_indexed(dir.path()));
    }

    #[test]
    fn test_is_stale_true_no_index() {
        let dir = tempdir().unwrap();
        assert!(is_stale(dir.path()));
    }
}
