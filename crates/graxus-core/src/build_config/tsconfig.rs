use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct TsConfig {
    pub compiler_options_base_url: Option<String>,
    pub paths: HashMap<String, Vec<String>>,
    pub references: Vec<String>,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct RawTsConfig {
    compilerOptions: Option<RawCompilerOptions>,
    references: Option<Vec<RawReference>>,
}

#[derive(Deserialize, Default)]
#[allow(non_snake_case)]
struct RawCompilerOptions {
    baseUrl: Option<String>,
    paths: Option<HashMap<String, Vec<String>>>,
}

#[derive(Deserialize)]
struct RawReference {
    path: Option<String>,
}

pub fn parse(root: &Path) -> Result<Option<TsConfig>> {
    let path = root.join("tsconfig.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let raw: RawTsConfig = serde_json::from_str(&content)?;

    let compiler_options = raw.compilerOptions.unwrap_or_default();
    let references = raw
        .references
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| r.path)
        .collect();

    Ok(Some(TsConfig {
        compiler_options_base_url: compiler_options.baseUrl,
        paths: compiler_options.paths.unwrap_or_default(),
        references,
    }))
}
