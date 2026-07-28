use anyhow::Result;
use graxus_core::ScannedFile;
use serde::{Deserialize, Serialize};

use crate::safety;

/// Controls how the search pattern is interpreted.
#[derive(Debug, Clone)]
pub enum SearchMode {
    /// Match the pattern as a plain string.
    Literal,
    /// Match the pattern as a regular expression.
    Regex,
}

/// A single match found during a search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    /// Relative path of the file containing the match.
    pub file: String,
    /// 1-based line number of the match.
    pub line: usize,
    /// 1-based column of the match start.
    pub column: usize,
    /// Full text of the matching line.
    pub text: String,
    /// The matched substring.
    pub match_text: String,
}

/// Search for `pattern` across `files` using the given `mode`.
///
/// Files that fail safety checks or cannot be read are silently skipped.
/// Returns all matches with file, line, column, and context.
pub fn search(pattern: &str, files: &[ScannedFile], mode: &SearchMode) -> Result<Vec<SearchHit>> {
    let re = match mode {
        SearchMode::Regex => Some(regex::Regex::new(pattern)?),
        SearchMode::Literal => None,
    };

    let mut hits = Vec::new();
    for file in files {
        if !safety::is_safe_to_read(&file.path) {
            continue;
        }
        let content = match std::fs::read_to_string(&file.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (line_num, line) in content.lines().enumerate() {
            let matches: Vec<(usize, usize)> = match mode {
                SearchMode::Literal => {
                    let mut m = Vec::new();
                    let mut start = 0;
                    while let Some(pos) = line[start..].find(pattern) {
                        m.push((start + pos, start + pos + pattern.len()));
                        start += pos + 1;
                    }
                    m
                }
                SearchMode::Regex => {
                    let re = re
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("regex not compiled for Regex mode"))?;
                    re.find_iter(line).map(|m| (m.start(), m.end())).collect()
                }
            };

            for (col_start, col_end) in matches {
                hits.push(SearchHit {
                    file: file.relative_path.clone(),
                    line: line_num + 1,
                    column: col_start + 1,
                    text: line.to_string(),
                    match_text: line[col_start..col_end].to_string(),
                });
            }
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graxus_core::{FileKind, Language};

    fn make_file(path: &str, content: &str) -> (ScannedFile, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let full_path = dir.path().join(path);
        std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        std::fs::write(&full_path, content).unwrap();
        let scanned = ScannedFile {
            path: full_path,
            relative_path: path.to_string(),
            kind: FileKind::Code,
            language: Language::Rust,
            hash: "test".to_string(),
            size: content.len() as u64,
            modified: chrono::Utc::now(),
        };
        (scanned, dir)
    }

    #[test]
    fn test_literal_search() {
        let (file, _dir) = make_file("test.rs", "fn goodbye() {\n    println!(\"world\");\n}");
        let hits = search("println", &[file], &SearchMode::Literal).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 2);
        assert_eq!(hits[0].match_text, "println");
    }

    #[test]
    fn test_literal_search_multiple() {
        let (file, _dir) = make_file("test.rs", "foo()\nbar()\nfoo()");
        let hits = search("foo", &[file], &SearchMode::Literal).unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn test_regex_search() {
        let (file, _dir) = make_file("test.rs", "fn goodbye() {\n    println!(\"world\");\n}");
        let hits = search(r"println!\(", &[file], &SearchMode::Regex).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn test_no_matches() {
        let (file, _dir) = make_file("test.rs", "fn goodbye() {}");
        let hits = search("xyz", &[file], &SearchMode::Literal).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let (file, _dir) = make_file("test.rs", "");
        let hits = search("goodbye", &[file], &SearchMode::Literal).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_regex_search_multiple_matches() {
        let (file, _dir) = make_file("test.rs", "let a = 1;\nlet b = 2;\nlet c = 3;");
        let hits = search(r"let \w+ = \d+;", &[file], &SearchMode::Regex).unwrap();
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].line, 1);
        assert_eq!(hits[1].line, 2);
        assert_eq!(hits[2].line, 3);
    }

    #[test]
    fn test_regex_search_no_matches() {
        let (file, _dir) = make_file("test.rs", "fn goodbye() {}");
        let hits = search(r"\d{3}-\d{4}", &[file], &SearchMode::Regex).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_regex_search_invalid_pattern() {
        let (file, _dir) = make_file("test.rs", "fn goodbye() {}");
        let result = search("[invalid", &[file], &SearchMode::Regex);
        assert!(result.is_err());
    }

    #[test]
    fn test_large_file_search() {
        let mut content = String::new();
        for i in 0..10_000 {
            content.push_str(&format!("line {} with target word\n", i));
        }
        let (file, _dir) = make_file("large.rs", &content);
        let hits = search("target", &[file], &SearchMode::Literal).unwrap();
        assert_eq!(hits.len(), 10_000);
    }

    #[test]
    fn test_regex_search_word_boundary() {
        let (file, _dir) = make_file("test.rs", "foobar foo bar foo_bar");
        let hits = search(r"\bfoo\b", &[file], &SearchMode::Regex).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].match_text, "foo");
    }
}
