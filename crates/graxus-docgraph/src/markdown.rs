//! Markdown content extraction: wiki links, tags, headings, and standard links.

use serde::{Deserialize, Serialize};

/// An Obsidian-style wiki link (`[[target]]` or `[[target|alias]]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiLink {
    /// The link target (note name or path, e.g. `"Note"` or `"folder/Note"`).
    pub target: String,
    /// Optional display alias (the part after `|`).
    pub alias: Option<String>,
    /// The full raw text of the link including brackets.
    pub full_text: String,
}

/// A standard markdown link (`[text](url)`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownLink {
    /// The display text inside `[...]`.
    pub text: String,
    /// The URL inside `(...)`.
    pub url: String,
}

/// A heading extracted from markdown content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading {
    /// Heading level (1–6).
    pub level: u8,
    /// The heading text (without the leading `#` characters).
    pub text: String,
    /// 1-based line number where the heading appears.
    pub line: usize,
}

/// Extract `[[wiki links]]` from markdown content.
///
/// Handles both `[[target]]` and `[[target|alias]]` forms.
/// Nested brackets are not supported — only the outermost `[[...]]` pair is matched.
pub fn extract_wiki_links(content: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();
    let mut remaining = content;

    while let Some(start) = remaining.find("[[") {
        remaining = &remaining[start + 2..];
        if let Some(end) = remaining.find("]]") {
            let inner = &remaining[..end];
            let (target, alias) = if let Some(pipe_pos) = inner.find('|') {
                (
                    inner[..pipe_pos].trim().to_string(),
                    Some(inner[pipe_pos + 1..].trim().to_string()),
                )
            } else {
                (inner.trim().to_string(), None)
            };
            links.push(WikiLink {
                target,
                alias,
                full_text: format!("[[{}]]", inner),
            });
            remaining = &remaining[end + 2..];
        }
    }

    links
}

/// Extract `#tags` from markdown content.
///
/// Recognizes inline tags like `#tag` and nested tags like `#nested/tag`.
/// Heading lines (`# Heading`) are not treated as tags. Tags inside
/// frontmatter delimiters (`---`) are skipped.
pub fn extract_tags(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for line in content.lines() {
        // Skip frontmatter delimiter lines
        if line.trim() == "---" {
            continue;
        }

        for word in line.split_whitespace() {
            if word.starts_with('#') && word.len() > 1 {
                // Trim trailing punctuation (commas, periods, parens, etc.)
                // but preserve '/' for nested tags and '-' for hyphenated tags.
                let tag = word[1..].trim_end_matches(|c: char| {
                    !c.is_alphanumeric() && c != '_' && c != '/' && c != '-'
                });
                // Remove any trailing '/' that survived trimming
                let tag = tag.trim_end_matches('/');
                if !tag.is_empty() {
                    tags.push(tag.to_string());
                }
            }
        }
    }
    tags
}

/// Extract headings from markdown content.
///
/// Returns headings at levels 1–6 with their 1-based line numbers.
/// Lines inside code fences are not excluded (callers should strip
/// code blocks beforehand if needed).
pub fn extract_headings(content: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count() as u8;
            if level > 0 && level <= 6 {
                let text = trimmed[level as usize..].trim().to_string();
                if !text.is_empty() {
                    headings.push(Heading {
                        level,
                        text,
                        line: line_num + 1,
                    });
                }
            }
        }
    }
    headings
}

/// Extract `[markdown links](url)` from content.
///
/// Skips wiki links (`[[...]]`). Only standard `[text](url)` links are returned.
pub fn extract_markdown_links(content: &str) -> Vec<MarkdownLink> {
    let mut links = Vec::new();
    let mut remaining = content;

    while let Some(bracket_start) = remaining.find('[') {
        // Skip wiki links
        if remaining[bracket_start..].starts_with("[[") {
            remaining = &remaining[bracket_start + 2..];
            continue;
        }

        remaining = &remaining[bracket_start..];
        if let Some(bracket_end) = remaining.find(']') {
            let text = &remaining[1..bracket_end];
            if remaining.get(bracket_end + 1..bracket_end + 2) == Some("(") {
                if let Some(paren_end) = remaining[bracket_end + 1..].find(')') {
                    let url = &remaining[bracket_end + 2..bracket_end + 1 + paren_end];
                    links.push(MarkdownLink {
                        text: text.to_string(),
                        url: url.to_string(),
                    });
                    remaining = &remaining[bracket_end + 1 + paren_end + 1..];
                    continue;
                }
            }
            remaining = &remaining[1..];
        } else {
            break;
        }
    }

    links
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Wiki links ────────────────────────────────────────────────

    #[test]
    fn wiki_link_simple() {
        let links = extract_wiki_links("See [[My Note]] for details.");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "My Note");
        assert!(links[0].alias.is_none());
        assert_eq!(links[0].full_text, "[[My Note]]");
    }

    #[test]
    fn wiki_link_with_alias() {
        let links = extract_wiki_links("Check [[My Note|this page]].");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "My Note");
        assert_eq!(links[0].alias.as_deref(), Some("this page"));
    }

    #[test]
    fn wiki_link_with_folder_path() {
        let links = extract_wiki_links("Link to [[folder/My Note]].");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "folder/My Note");
    }

    #[test]
    fn wiki_link_multiple() {
        let links = extract_wiki_links("See [[A]] and [[B|second]].");
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "A");
        assert_eq!(links[1].target, "B");
        assert_eq!(links[1].alias.as_deref(), Some("second"));
    }

    #[test]
    fn wiki_link_none() {
        let links = extract_wiki_links("No links here.");
        assert!(links.is_empty());
    }

    #[test]
    fn wiki_link_unclosed() {
        let links = extract_wiki_links("Broken [[link here");
        assert!(links.is_empty());
    }

    #[test]
    fn wiki_link_empty_target() {
        let links = extract_wiki_links("Empty [[]] link.");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "");
    }

    // ── Tags ──────────────────────────────────────────────────────

    #[test]
    fn tag_simple() {
        let tags = extract_tags("This has #rust in it.");
        assert_eq!(tags, vec!["rust"]);
    }

    #[test]
    fn tag_multiple() {
        let tags = extract_tags("#tag1 and #tag2 here.");
        assert_eq!(tags, vec!["tag1", "tag2"]);
    }

    #[test]
    fn tag_nested() {
        let tags = extract_tags("See #project/backend for details.");
        assert_eq!(tags, vec!["project/backend"]);
    }

    #[test]
    fn tag_nested_deep() {
        let tags = extract_tags("Tag #a/b/c works.");
        assert_eq!(tags, vec!["a/b/c"]);
    }

    #[test]
    fn tag_trailing_punctuation() {
        let tags = extract_tags("Tags: #rust, #go; #python.");
        assert_eq!(tags, vec!["rust", "go", "python"]);
    }

    #[test]
    fn tag_with_hyphen() {
        let tags = extract_tags("Use #my-tag please.");
        assert_eq!(tags, vec!["my-tag"]);
    }

    #[test]
    fn tag_none() {
        let tags = extract_tags("No tags here.");
        assert!(tags.is_empty());
    }

    #[test]
    fn tag_heading_not_extracted() {
        // "# Heading" splits to ["#", "Heading"] — "#" alone is length 1
        let tags = extract_tags("# Heading\n## Subheading\n");
        assert!(tags.is_empty());
    }

    #[test]
    fn tag_skips_frontmatter_delimiters() {
        let tags = extract_tags("---\n#real_tag\n---");
        assert_eq!(tags, vec!["real_tag"]);
    }

    #[test]
    fn tag_at_start_of_line() {
        let tags = extract_tags("#important note");
        assert_eq!(tags, vec!["important"]);
    }

    // ── Headings ──────────────────────────────────────────────────

    #[test]
    fn headings_basic() {
        let md = "# Title\n## Section\n### Sub\n";
        let headings = extract_headings(md);
        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "Title");
        assert_eq!(headings[0].line, 1);
        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[1].text, "Section");
        assert_eq!(headings[1].line, 2);
        assert_eq!(headings[2].level, 3);
        assert_eq!(headings[2].text, "Sub");
        assert_eq!(headings[2].line, 3);
    }

    #[test]
    fn headings_empty() {
        let headings = extract_headings("No headings.");
        assert!(headings.is_empty());
    }

    #[test]
    fn headings_level_6() {
        let headings = extract_headings("###### Deep heading");
        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].level, 6);
    }

    #[test]
    fn headings_no_empty_text() {
        // A line with just "##" and no text should be skipped
        let headings = extract_headings("## \n### text\n");
        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].text, "text");
    }

    // ── Markdown links ────────────────────────────────────────────

    #[test]
    fn markdown_link_basic() {
        let links = extract_markdown_links("[Example](https://example.com)");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "Example");
        assert_eq!(links[0].url, "https://example.com");
    }

    #[test]
    fn markdown_link_multiple() {
        let links = extract_markdown_links("[A](http://a.com) and [B](http://b.com)");
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].text, "A");
        assert_eq!(links[1].text, "B");
    }

    #[test]
    fn markdown_link_skips_wiki_links() {
        let links = extract_markdown_links("Wiki [[Note]] and [MD](url)");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "MD");
    }

    #[test]
    fn markdown_link_none() {
        let links = extract_markdown_links("No links.");
        assert!(links.is_empty());
    }

    // ── Mixed content ─────────────────────────────────────────────

    #[test]
    fn mixed_wiki_and_markdown_links() {
        let content = "See [[Wiki Note]] and [MD Link](http://example.com).";
        let wiki = extract_wiki_links(content);
        let md = extract_markdown_links(content);
        assert_eq!(wiki.len(), 1);
        assert_eq!(wiki[0].target, "Wiki Note");
        assert_eq!(md.len(), 1);
        assert_eq!(md[0].text, "MD Link");
    }

    #[test]
    fn empty_content() {
        assert!(extract_wiki_links("").is_empty());
        assert!(extract_tags("").is_empty());
        assert!(extract_headings("").is_empty());
        assert!(extract_markdown_links("").is_empty());
    }
}
