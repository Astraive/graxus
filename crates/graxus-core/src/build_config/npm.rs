use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct NpmConfig {
    pub name: Option<String>,
    pub scripts: HashMap<String, String>,
    pub workspaces: Vec<String>,
    #[serde(rename = "type")]
    pub module_type: Option<String>,
}

#[derive(Deserialize)]
struct RawPackageJson {
    name: Option<String>,
    scripts: Option<HashMap<String, String>>,
    workspaces: Option<RawWorkspaces>,
    #[serde(rename = "type")]
    module_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawWorkspaces {
    List(Vec<String>),
    Object { packages: Vec<String> },
}

pub fn parse(root: &Path) -> Result<Option<NpmConfig>> {
    let path = root.join("package.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let raw: RawPackageJson = serde_json::from_str(&content)?;

    let workspaces = match raw.workspaces {
        Some(RawWorkspaces::List(v)) => v,
        Some(RawWorkspaces::Object { packages }) => packages,
        None => Vec::new(),
    };

    Ok(Some(NpmConfig {
        name: raw.name,
        scripts: raw.scripts.unwrap_or_default(),
        workspaces,
        module_type: raw.module_type,
    }))
}
