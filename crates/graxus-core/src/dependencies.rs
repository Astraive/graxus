use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A dependency found in a project manifest file (Cargo.toml, package.json, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub source: DependencySource,
    pub kind: DependencyKind,
}

/// The package manager that manages a dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencySource {
    Cargo,
    Npm,
    Go,
    Python,
}

/// Whether a dependency is used at runtime, development, or build time.
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
    deps.extend(detect_in_dir(root).unwrap_or_default());

    // Check immediate subdirectories (for monorepo / workspace layouts)
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && !path
                    .file_name()
                    .map(|n| n.to_string_lossy().starts_with('.'))
                    .unwrap_or(true)
            {
                deps.extend(detect_in_dir(&path).unwrap_or_default());
            }
        }
    }

    deps
}

fn detect_in_dir(dir: &Path) -> Result<Vec<Dependency>> {
    let mut deps = Vec::new();
    if dir.join("Cargo.toml").exists() {
        deps.extend(parse_cargo_deps(dir).context("Failed to parse Cargo.toml")?);
    }
    if dir.join("package.json").exists() {
        deps.extend(parse_npm_deps(dir).context("Failed to parse package.json")?);
    }
    if dir.join("go.mod").exists() {
        deps.extend(parse_go_deps(dir).context("Failed to parse go.mod")?);
    }
    if dir.join("requirements.txt").exists() || dir.join("pyproject.toml").exists() {
        deps.extend(parse_python_deps(dir).context("Failed to parse Python deps")?);
    }
    Ok(deps)
}

fn parse_cargo_deps(root: &Path) -> Result<Vec<Dependency>> {
    let mut deps = Vec::new();
    let Ok(content) = std::fs::read_to_string(root.join("Cargo.toml")) else {
        return Ok(deps);
    };

    let mut in_deps_section = false;
    let section_re = regex::Regex::new(r"^\[(.+)\]$").context("invalid section regex")?;
    // Simple: name = "version"
    let simple_re =
        regex::Regex::new(r#"^(\w[\w-]*)\s*=\s*"([^"]*)""#).context("invalid simple regex")?;
    // Table: name = { version = "version", ... }
    let table_re = regex::Regex::new(r#"^(\w[\w-]*)\s*=\s*\{"#).context("invalid table regex")?;
    let version_re =
        regex::Regex::new(r#"version\s*=\s*"([^"]*)""#).context("invalid version regex")?;

    for line in content.lines() {
        let line = line.trim();
        if let Some(cap) = section_re.captures(line) {
            let section = cap.get(1).context("missing section capture")?.as_str();
            in_deps_section = section == "dependencies"
                || section == "workspace.dependencies"
                || section == "dev-dependencies"
                || section == "build-dependencies";
            continue;
        }
        if !in_deps_section {
            continue;
        }
        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        // Try simple format: name = "version"
        if let Some(cap) = simple_re.captures(line) {
            let name = cap
                .get(1)
                .context("missing name capture")?
                .as_str()
                .to_string();
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
            let name = cap
                .get(1)
                .context("missing table name capture")?
                .as_str()
                .to_string();
            let version = version_re
                .captures(line)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
            deps.push(Dependency {
                name,
                version,
                source: DependencySource::Cargo,
                kind: DependencyKind::Runtime,
            });
        }
    }
    Ok(deps)
}

fn parse_npm_deps(root: &Path) -> Result<Vec<Dependency>> {
    let mut deps = Vec::new();
    let Ok(content) = std::fs::read_to_string(root.join("package.json")) else {
        return Ok(deps);
    };

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
    Ok(deps)
}

fn parse_go_deps(root: &Path) -> Result<Vec<Dependency>> {
    let mut deps = Vec::new();
    let Ok(content) = std::fs::read_to_string(root.join("go.mod")) else {
        return Ok(deps);
    };

    let re = regex::Regex::new(r"(\S+)\s+(v[\d.]+)").context("invalid go.mod regex")?;
    for cap in re.captures_iter(&content) {
        let name = cap
            .get(1)
            .context("missing go dep name")?
            .as_str()
            .to_string();
        let version = cap
            .get(2)
            .context("missing go dep version")?
            .as_str()
            .to_string();
        deps.push(Dependency {
            name,
            version: Some(version),
            source: DependencySource::Go,
            kind: DependencyKind::Runtime,
        });
    }
    Ok(deps)
}

fn parse_python_deps(root: &Path) -> Result<Vec<Dependency>> {
    let mut deps = Vec::new();

    if let Ok(content) = std::fs::read_to_string(root.join("requirements.txt")) {
        let re = regex::Regex::new(r"^([\w-]+)\s*(?:[>=<~!]+\s*([\d.]+))?")
            .context("invalid python regex")?;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some(cap) = re.captures(line) {
                let name = cap
                    .get(1)
                    .context("missing python dep name")?
                    .as_str()
                    .to_string();
                deps.push(Dependency {
                    name,
                    version: cap.get(2).map(|v| v.as_str().to_string()),
                    source: DependencySource::Python,
                    kind: DependencyKind::Runtime,
                });
            }
        }
    }
    Ok(deps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_parse_cargo_simple_deps() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[dependencies]
serde = "1.0"
anyhow = "1"
tokio = "1.28"
"#,
        )
        .unwrap();

        let deps = parse_cargo_deps(dir.path()).unwrap();
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].name, "serde");
        assert_eq!(deps[0].version.as_deref(), Some("1.0"));
        assert_eq!(deps[1].name, "anyhow");
        assert_eq!(deps[2].name, "tokio");
    }

    #[test]
    fn test_parse_cargo_table_deps() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
"#,
        )
        .unwrap();

        let deps = parse_cargo_deps(dir.path()).unwrap();
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].name, "serde");
        assert_eq!(deps[0].version.as_deref(), Some("1.0"));
        assert_eq!(deps[1].name, "tokio");
        assert_eq!(deps[1].version.as_deref(), Some("1"));
    }

    #[test]
    fn test_parse_cargo_dev_dependencies() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[dependencies]
serde = "1.0"

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
"#,
        )
        .unwrap();

        let deps = parse_cargo_deps(dir.path()).unwrap();
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[1].name, "tempfile");
        assert_eq!(deps[2].name, "assert_cmd");
    }

    #[test]
    fn test_parse_cargo_build_dependencies() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[build-dependencies]
cc = "1.0"
"#,
        )
        .unwrap();

        let deps = parse_cargo_deps(dir.path()).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "cc");
    }

    #[test]
    fn test_parse_cargo_workspace_dependencies() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[workspace.dependencies]
serde = "1.0"
anyhow = "1"
"#,
        )
        .unwrap();

        let deps = parse_cargo_deps(dir.path()).unwrap();
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].name, "serde");
        assert_eq!(deps[1].name, "anyhow");
    }

    #[test]
    fn test_parse_cargo_mixed_simple_and_table() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[dependencies]
serde = "1.0"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
"#,
        )
        .unwrap();

        let deps = parse_cargo_deps(dir.path()).unwrap();
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].name, "serde");
        assert_eq!(deps[0].version.as_deref(), Some("1.0"));
        assert_eq!(deps[1].name, "tokio");
        assert_eq!(deps[1].version.as_deref(), Some("1"));
        assert_eq!(deps[2].name, "anyhow");
    }

    #[test]
    fn test_parse_cargo_empty_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let deps = parse_cargo_deps(dir.path()).unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn test_parse_cargo_comments_ignored() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[dependencies]
# serde = "1.0"
anyhow = "1"
"#,
        )
        .unwrap();

        let deps = parse_cargo_deps(dir.path()).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "anyhow");
    }

    #[test]
    fn test_parse_npm_deps() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
    "dependencies": {
        "react": "^18.0.0",
        "react-dom": "^18.0.0"
    },
    "devDependencies": {
        "typescript": "^5.0.0"
    }
}"#,
        )
        .unwrap();

        let deps = parse_npm_deps(dir.path()).unwrap();
        assert_eq!(deps.len(), 3);
        let runtime: Vec<_> = deps
            .iter()
            .filter(|d| matches!(d.kind, DependencyKind::Runtime))
            .collect();
        let dev: Vec<_> = deps
            .iter()
            .filter(|d| matches!(d.kind, DependencyKind::Dev))
            .collect();
        assert_eq!(runtime.len(), 2);
        assert_eq!(dev.len(), 1);
        assert_eq!(dev[0].name, "typescript");
    }

    #[test]
    fn test_parse_go_deps() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("go.mod"),
            r#"module example.com/myapp

go 1.21

require (
    github.com/gin-gonic/gin v1.9.1
    github.com/go-sql-driver/mysql v1.7.1
)
"#,
        )
        .unwrap();

        let deps = parse_go_deps(dir.path()).unwrap();
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].name, "github.com/gin-gonic/gin");
        assert_eq!(deps[0].version.as_deref(), Some("v1.9.1"));
        assert_eq!(deps[1].name, "github.com/go-sql-driver/mysql");
    }

    #[test]
    fn test_parse_python_deps() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("requirements.txt"),
            r#"# This is a comment
flask>=2.0
requests==2.31.0
pytest
"#,
        )
        .unwrap();

        let deps = parse_python_deps(dir.path()).unwrap();
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].name, "flask");
        assert_eq!(deps[0].version.as_deref(), Some("2.0"));
        assert_eq!(deps[1].name, "requests");
        assert_eq!(deps[1].version.as_deref(), Some("2.31.0"));
        assert_eq!(deps[2].name, "pytest");
        assert_eq!(deps[2].version, None);
    }

    #[test]
    fn test_parse_python_empty_requirements() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("requirements.txt"), "# only comments\n\n").unwrap();
        let deps = parse_python_deps(dir.path()).unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn test_detect_dependencies_combined() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[dependencies]
serde = "1.0"
"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"react": "^18"}}"#,
        )
        .unwrap();

        let deps = detect_dependencies(dir.path());
        assert_eq!(deps.len(), 2);
        let cargo_deps: Vec<_> = deps
            .iter()
            .filter(|d| matches!(d.source, DependencySource::Cargo))
            .collect();
        let npm_deps: Vec<_> = deps
            .iter()
            .filter(|d| matches!(d.source, DependencySource::Npm))
            .collect();
        assert_eq!(cargo_deps.len(), 1);
        assert_eq!(npm_deps.len(), 1);
    }

    #[test]
    fn test_detect_dependencies_subdirectory() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("backend");
        fs::create_dir(&sub).unwrap();
        fs::write(
            sub.join("Cargo.toml"),
            r#"
[dependencies]
axum = "0.7"
"#,
        )
        .unwrap();

        let deps = detect_dependencies(dir.path());
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "axum");
    }

    #[test]
    fn test_detect_dependencies_skips_hidden_dirs() {
        let dir = tempdir().unwrap();
        let hidden = dir.path().join(".hidden");
        fs::create_dir(&hidden).unwrap();
        fs::write(
            hidden.join("Cargo.toml"),
            r#"
[dependencies]
secret = "1.0"
"#,
        )
        .unwrap();

        let deps = detect_dependencies(dir.path());
        assert!(deps.is_empty());
    }

    #[test]
    fn test_detect_no_manifest() {
        let dir = tempdir().unwrap();
        let deps = detect_dependencies(dir.path());
        assert!(deps.is_empty());
    }
}
