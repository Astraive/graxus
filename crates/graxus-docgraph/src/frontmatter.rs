use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub related_code: Vec<String>,
    #[serde(default)]
    pub symbols: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

/// Extract YAML frontmatter from markdown content.
/// Returns (frontmatter, content_after_frontmatter).
pub fn parse(content: &str) -> (Option<Frontmatter>, &str) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content);
    }

    let after_first = &trimmed[3..];
    let end = match after_first.find("\n---") {
        Some(pos) => pos,
        None => return (None, content),
    };

    let yaml_str = &after_first[..end];
    let remaining = &after_first[end + 4..]; // skip "\n---"

    match serde_yaml::from_str::<Frontmatter>(yaml_str) {
        Ok(fm) => (Some(fm), remaining),
        Err(e) => {
            tracing::warn!("Failed to parse frontmatter: {}", e);
            (None, content)
        }
    }
}

/// Parse frontmatter from a file.
pub fn parse_file(path: &Path) -> Result<(Option<Frontmatter>, String)> {
    let content = std::fs::read_to_string(path)?;
    let (fm, body) = parse(&content);
    Ok((fm, body.to_string()))
}
