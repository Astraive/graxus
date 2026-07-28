use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct CargoConfig {
    pub workspace_members: Vec<String>,
    pub features: HashMap<String, Vec<String>>,
    pub edition: Option<String>,
}

pub fn parse(root: &Path) -> Result<Option<CargoConfig>> {
    let path = root.join("Cargo.toml");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;

    let re_members = Regex::new(r"(?m)members\s*=\s*\[([^\]]*)\]")?;
    let re_member_str = Regex::new(r#""([^"]+)""#)?;
    let re_edition = Regex::new(r#"(?m)edition\s*=\s*"([^"]+)""#)?;

    let workspace_members = re_members
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| {
            re_member_str
                .captures_iter(m.as_str())
                .filter_map(|c| c.get(1).map(|v| v.as_str().to_string()))
                .collect()
        })
        .unwrap_or_default();

    let edition = re_edition
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    let features = parse_features(&content);

    Ok(Some(CargoConfig {
        workspace_members,
        features,
        edition,
    }))
}

fn parse_features(content: &str) -> HashMap<String, Vec<String>> {
    let mut features = HashMap::new();
    let re_section = Regex::new(r"(?m)^\[features\]\s*\n((?:[^\[]*\n?)*)").unwrap();
    if let Some(caps) = re_section.captures(content) {
        let block = caps.get(1).unwrap().as_str();
        let re_entry = Regex::new(r#"(?m)^(\w+)\s*=\s*\[([^\]]*)\]"#).unwrap();
        let re_val = Regex::new(r#""([^"]+)""#).unwrap();
        for ec in re_entry.captures_iter(block) {
            let name = ec.get(1).unwrap().as_str().to_string();
            let vals = ec.get(2).unwrap().as_str();
            let items: Vec<String> = re_val
                .captures_iter(vals)
                .filter_map(|c| c.get(1).map(|v| v.as_str().to_string()))
                .collect();
            features.insert(name, items);
        }
    }
    features
}
