use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub source: DependencySource,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencySource {
    Cargo,
    Npm,
    Go,
    Python,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyKind {
    Runtime,
    Dev,
    Build,
}

/// Detect dependencies from manifest files. Checks root and immediate subdirectories.
pub fn detect_dependencies(root: &Path) -> Vec<Dependency> {
    let mut deps = Vec::new();

    // Check root
    deps.extend(detect_in_dir(root));

    // Check immediate subdirectories (for monorepo / workspace layouts)
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(true) {
                deps.extend(detect_in_dir(&path));
            }
        }
    }

    deps
}

fn detect_in_dir(dir: &Path) -> Vec<Dependency> {
    let mut deps = Vec::new();
    if dir.join("Cargo.toml").exists() {
        deps.extend(parse_cargo_deps(dir));
    }
    if dir.join("package.json").exists() {
        deps.extend(parse_npm_deps(dir));
    }
    if dir.join("go.mod").exists() {
        deps.extend(parse_go_deps(dir));
    }
    if dir.join("requirements.txt").exists() || dir.join("pyproject.toml").exists() {
        deps.extend(parse_python_deps(dir));
    }
    deps
}

fn parse_cargo_deps(root: &Path) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let Ok(content) = std::fs::read_to_string(root.join("Cargo.toml")) else { return deps };

    let mut in_deps_section = false;
    let section_re = regex::Regex::new(r"^\[(.+)\]$").unwrap();
    // Simple: name = "version"
    let simple_re = regex::Regex::new(r#"^(\w[\w-]*)\s*=\s*"([^"]*)""#).unwrap();
    // Table: name = { version = "version", ... }
    let table_re = regex::Regex::new(r#"^(\w[\w-]*)\s*=\s*\{"#).unwrap();
    let version_re = regex::Regex::new(r#"version\s*=\s*"([^"]*)""#).unwrap();

    for line in content.lines() {
        let line = line.trim();
        if let Some(cap) = section_re.captures(line) {
            let section = cap.get(1).unwrap().as_str();
            in_deps_section = section == "dependencies" || section == "workspace.dependencies"
                || section == "dev-dependencies" || section == "build-dependencies";
            continue;
        }
        if !in_deps_section { continue; }
        if line.starts_with('#') || line.is_empty() { continue; }

        // Try simple format: name = "version"
        if let Some(cap) = simple_re.captures(line) {
            let name = cap.get(1).unwrap().as_str().to_string();
            let version = cap.get(2).map(|m| m.as_str().to_string());
            deps.push(Dependency {
                name,
                version,
                source: DependencySource::Cargo,
                kind: DependencyKind::Runtime,
            });
            continue;
        }

        // Try table format: name = { version = "ver", ... }
        if let Some(cap) = table_re.captures(line) {
            let name = cap.get(1).unwrap().as_str().to_string();
            let version = version_re.captures(line).map(|c| c.get(1).unwrap().as_str().to_string());
            deps.push(Dependency {
                name,
                version,
                source: DependencySource::Cargo,
                kind: DependencyKind::Runtime,
            });
        }
    }
    deps
}

fn parse_npm_deps(root: &Path) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let Ok(content) = std::fs::read_to_string(root.join("package.json")) else { return deps };

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
        if let Some(deps_obj) = json.get("dependencies").and_then(|d| d.as_object()) {
            for (name, version) in deps_obj {
                deps.push(Dependency {
                    name: name.clone(),
                    version: version.as_str().map(|s| s.to_string()),
                    source: DependencySource::Npm,
                    kind: DependencyKind::Runtime,
                });
            }
        }
        if let Some(dev_deps) = json.get("devDependencies").and_then(|d| d.as_object()) {
            for (name, version) in dev_deps {
                deps.push(Dependency {
                    name: name.clone(),
                    version: version.as_str().map(|s| s.to_string()),
                    source: DependencySource::Npm,
                    kind: DependencyKind::Dev,
                });
            }
        }
    }
    deps
}

fn parse_go_deps(root: &Path) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let Ok(content) = std::fs::read_to_string(root.join("go.mod")) else { return deps };

    let re = regex::Regex::new(r"(\S+)\s+(v[\d.]+)").unwrap();
    for cap in re.captures_iter(&content) {
        deps.push(Dependency {
            name: cap[1].to_string(),
            version: Some(cap[2].to_string()),
            source: DependencySource::Go,
            kind: DependencyKind::Runtime,
        });
    }
    deps
}

fn parse_python_deps(root: &Path) -> Vec<Dependency> {
    let mut deps = Vec::new();

    if let Ok(content) = std::fs::read_to_string(root.join("requirements.txt")) {
        let re = regex::Regex::new(r"^([\w-]+)\s*(?:[>=<~!]+\s*([\d.]+))?").unwrap();
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() { continue; }
            if let Some(cap) = re.captures(line) {
                deps.push(Dependency {
                    name: cap[1].to_string(),
                    version: cap.get(2).map(|v| v.as_str().to_string()),
                    source: DependencySource::Python,
                    kind: DependencyKind::Runtime,
                });
            }
        }
    }
    deps
}
