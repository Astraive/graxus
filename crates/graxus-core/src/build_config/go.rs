use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct GoReplace {
    pub old: String,
    pub new: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoConfig {
    pub module: Option<String>,
    pub go_version: Option<String>,
    pub replaces: Vec<GoReplace>,
}

pub fn parse(root: &Path) -> Result<Option<GoConfig>> {
    let path = root.join("go.mod");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;

    let re_module = Regex::new(r"(?m)^module\s+(.+)$")?;
    let re_go = Regex::new(r"(?m)^go\s+([\d.]+)")?;
    let re_replace = Regex::new(r"(?m)^replace\s+(.+?)\s*=>\s*(.+?)(?:\s+v([\d.]+))?$")?;

    let module = re_module
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string());

    let go_version = re_go
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    let mut replaces = Vec::new();
    for cap in re_replace.captures_iter(&content) {
        let old = cap.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        let new = cap.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        let version = cap.get(3).map(|m| m.as_str().to_string());
        replaces.push(GoReplace { old, new, version });
    }

    Ok(Some(GoConfig {
        module,
        go_version,
        replaces,
    }))
}
