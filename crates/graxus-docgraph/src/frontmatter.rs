//! YAML frontmatter parsing for Obsidian-compatible markdown files.
//!
//! Extracts and deserializes YAML frontmatter delimited by `---` markers.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Parsed YAML frontmatter from a markdown document.
///
/// Maps standard Obsidian frontmatter fields (`title`, `tags`, `aliases`, etc.)
/// and preserves unknown fields in `extra` via `#[serde(flatten)]`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    /// Document title from frontmatter.
    pub title: Option<String>,
    /// Short description of the document.
    pub description: Option<String>,
    /// Tags declared in frontmatter (e.g. `["rust", "tutorial"]`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Alternate names for the document (Obsidian aliases).
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Paths or identifiers of related source code files.
    #[serde(default)]
    pub related_code: Vec<String>,
    /// Code symbols referenced by this document.
    #[serde(default)]
    pub symbols: Vec<String>,
    /// Any additional frontmatter fields not captured by named fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

/// Extract YAML frontmatter from markdown content.
///
/// Expects frontmatter delimited by `---` at the very start of the content
/// (optionally preceded by whitespace). Returns `(Some(frontmatter), body)`
/// if valid YAML is found, or `(None, original_content)` if no frontmatter
/// is present or parsing fails.
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

/// Parse frontmatter from a file path.
///
/// Reads the file and delegates to [`parse`]. Returns the parsed frontmatter
/// (if any) and the remaining body content as an owned `String`.
pub fn parse_file(path: &Path) -> Result<(Option<Frontmatter>, String)> {
    let content = std::fs::read_to_string(path)?;
    let (fm, body) = parse(&content);
    Ok((fm, body.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_frontmatter() {
        let input = "\
---
title: My Note
description: A test note
tags:
  - rust
  - tutorial
aliases:
  - Test Note
related_code:
  - crates/foo/src/lib.rs
symbols:
  - Foo::bar
---
Body content here.";
        let (fm, body) = parse(input);
        let fm = fm.expect("should parse frontmatter");
        assert_eq!(fm.title.as_deref(), Some("My Note"));
        assert_eq!(fm.description.as_deref(), Some("A test note"));
        assert_eq!(fm.tags, vec!["rust", "tutorial"]);
        assert_eq!(fm.aliases, vec!["Test Note"]);
        assert_eq!(fm.related_code, vec!["crates/foo/src/lib.rs"]);
        assert_eq!(fm.symbols, vec!["Foo::bar"]);
        assert_eq!(body, "\nBody content here.");
    }

    #[test]
    fn parse_title_and_tags_only() {
        let input = "\
---
title: Minimal
tags:
  - dev
---
Content.";
        let (fm, body) = parse(input);
        let fm = fm.expect("should parse frontmatter");
        assert_eq!(fm.title.as_deref(), Some("Minimal"));
        assert_eq!(fm.tags, vec!["dev"]);
        assert_eq!(body, "\nContent.");
    }

    #[test]
    fn parse_no_frontmatter() {
        let input = "Just plain markdown.\nNo frontmatter here.";
        let (fm, body) = parse(input);
        assert!(fm.is_none());
        assert_eq!(body, input);
    }

    #[test]
    fn parse_empty_frontmatter() {
        let input = "\
---
---
Body.";
        let (fm, body) = parse(input);
        // Empty YAML parses to default Frontmatter
        let fm = fm.expect("empty frontmatter should parse");
        assert!(fm.title.is_none());
        assert!(fm.tags.is_empty());
        assert_eq!(body, "\nBody.");
    }

    #[test]
    fn parse_frontmatter_with_extra_fields() {
        let input = "\
---
title: Extended
custom_field: goodbye
nested:
  key: value
---
Body.";
        let (fm, body) = parse(input);
        let fm = fm.expect("should parse frontmatter");
        assert_eq!(fm.title.as_deref(), Some("Extended"));
        assert_eq!(
            fm.extra.get("custom_field").and_then(|v| v.as_str()),
            Some("goodbye")
        );
        assert_eq!(body, "\nBody.");
    }

    #[test]
    fn parse_leading_whitespace_before_frontmatter() {
        let input = "\n\n---\ntitle: Indented\n---\nBody.";
        let (fm, _) = parse(input);
        let fm = fm.expect("should parse despite leading whitespace");
        assert_eq!(fm.title.as_deref(), Some("Indented"));
    }

    #[test]
    fn parse_malformed_yaml_returns_none() {
        let input = "\
---
title: [unclosed
---
Body.";
        let (fm, body) = parse(input);
        assert!(fm.is_none());
        assert_eq!(body, input);
    }

    #[test]
    fn parse_frontmatter_with_no_closing_delimiter() {
        let input = "\
---
title: No Closing
Body without end delimiter.";
        let (fm, body) = parse(input);
        assert!(fm.is_none());
        assert_eq!(body, input);
    }
}