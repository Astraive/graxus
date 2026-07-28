use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct DotnetConfig {
    pub project_name: Option<String>,
    pub target_framework: Option<String>,
    pub sdk_version: Option<String>,
    pub package_references: Vec<String>,
}

#[derive(Deserialize)]
struct GlobalJson {
    sdk: Option<SdkConfig>,
}

#[derive(Deserialize)]
struct SdkConfig {
    version: Option<String>,
}

pub fn parse(root: &Path) -> Result<Option<DotnetConfig>> {
    let csproj = find_csproj(root)?;
    let content = match csproj {
        Some(c) => c,
        None => return Ok(None),
    };

    let re_tf = Regex::new(r"(?mi)<TargetFramework>\s*(.+?)\s*</TargetFramework>")?;
    let re_pkg = Regex::new(r#"(?mi)<PackageReference\s+Include="([^"]+)""#)?;
    let re_proj_name = Regex::new(r"(?mi)<AssemblyName>\s*(.+?)\s*</AssemblyName>")?;
    let re_sdk = Regex::new(r#"(?mi)<Project\s+Sdk="([^"]+)""#)?;

    let target_framework = re_tf
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    let package_references = re_pkg
        .captures_iter(&content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();

    let project_name = re_proj_name
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .or_else(|| {
            re_sdk
                .captures(&content)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
        });

    // Try to read global.json for SDK version
    let sdk_version = read_global_json(root);

    Ok(Some(DotnetConfig {
        project_name,
        target_framework,
        sdk_version,
        package_references,
    }))
}

fn find_csproj(root: &Path) -> Result<Option<String>> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("csproj") {
            return Ok(Some(std::fs::read_to_string(path)?));
        }
    }
    Ok(None)
}

fn read_global_json(root: &Path) -> Option<String> {
    let path = root.join("global.json");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    let parsed: GlobalJson = serde_json::from_str(&content).ok()?;
    parsed.sdk.and_then(|s| s.version)
}
