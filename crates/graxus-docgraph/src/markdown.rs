use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiLink {
    pub target: String,
    pub alias: Option<String>,
    pub full_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkdownLink {
    pub text: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: usize,
}

/// Extract [[wiki links]] from markdown content.
pub fn extract_wiki_links(content: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();
    let mut remaining = content;

    while let Some(start) = remaining.find("[[") {
        remaining = &remaining[start + 2..];
        if let Some(end) = remaining.find("]]") {
            let inner = &remaining[..end];
            let (target, alias) = if let Some(pipe_pos) = inner.find('|') {
                (inner[..pipe_pos].trim().to_string(), Some(inner[pipe_pos + 1..].trim().to_string()))
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

/// Extract #tags from markdown content.
pub fn extract_tags(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for line in content.lines() {
        // Skip frontmatter
        if line.trim() == "---" {
            continue;
        }

        for word in line.split_whitespace() {
            if word.starts_with('#') && word.len() > 1 {
                let tag = word[1..].trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
                if !tag.is_empty() && !tag.contains('/') {
                    tags.push(tag.to_string());
                }
            }
        }
    }
    tags
}

/// Extract headings from markdown content.
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

/// Extract [markdown links](url) from content.
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
