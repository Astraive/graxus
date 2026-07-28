use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct PythonConfig {
    pub name: Option<String>,
    pub python_version: Option<String>,
    pub dependencies: Vec<String>,
    pub scripts: HashMap<String, String>,
}

pub fn parse(root: &Path) -> Result<Option<PythonConfig>> {
    let path = root.join("pyproject.toml");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;

    let re_project_name = Regex::new(r#"(?m)^\[project\]\s*\nname\s*=\s*["']([^"']+)["']"#)?;
    let re_poetry_name = Regex::new(r#"(?m)^\[tool\.poetry\]\s*\nname\s*=\s*["']([^"']+)["']"#)?;
    let re_python_ver = Regex::new(r#"(?m)requires-python\s*=\s*[">=< ]*([0-9.]+)"#)?;
    let re_poetry_python = Regex::new(r#"(?m)^\[tool\.poetry\.dependencies\]\s*\npython\s*=\s*["'][><= ]*([0-9.]+)"#)?;
    let re_deps = Regex::new(r#"(?m)^\[project\]\s*\ndependencies\s*=\s*\[([^\]]*)\]"#)?;
    let re_dep_str = Regex::new(r#""([^"]+)""#)?;

    let name = re_project_name
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .or_else(|| {
            re_poetry_name
                .captures(&content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        });

    let python_version = re_python_ver
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .or_else(|| {
            re_poetry_python
                .captures(&content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        });

    let dependencies = re_deps
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| {
            re_dep_str
                .captures_iter(m.as_str())
                .filter_map(|c| c.get(1).map(|v| v.as_str().to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(Some(PythonConfig {
        name,
        python_version,
        dependencies,
        scripts: HashMap::new(),
    }))
}
