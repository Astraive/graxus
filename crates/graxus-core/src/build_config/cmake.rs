use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct CMakeConfig {
    pub project_name: Option<String>,
    pub minimum_version: Option<String>,
    pub languages: Vec<String>,
    pub subdirectories: Vec<String>,
}

pub fn parse(root: &Path) -> Result<Option<CMakeConfig>> {
    let path = root.join("CMakeLists.txt");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;

    let re_project = Regex::new(r"(?mi)project\(\s*([^ )]+)")?;
    let re_min_ver = Regex::new(r"(?mi)cmake_minimum_required\(\s*VERSION\s+([\d.]+)")?;
    let re_languages = Regex::new(r"(?mi)project\([^)]*LANGUAGES\s+([^)]+)\)")?;
    let re_subdir = Regex::new(r"(?mi)add_subdirectory\(\s*([^ )]+)")?;

    let project_name = re_project
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    let minimum_version = re_min_ver
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    let languages = re_languages
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| {
            m.as_str()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let subdirectories = re_subdir
        .captures_iter(&content)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();

    Ok(Some(CMakeConfig {
        project_name,
        minimum_version,
        languages,
        subdirectories,
    }))
}
