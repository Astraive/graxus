use anyhow::Result;
use graxus_core::ScannedFile;
use serde::{Deserialize, Serialize};

use crate::safety;

#[derive(Debug, Clone)]
pub enum SearchMode {
    Literal,
    Regex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub text: String,
    pub match_text: String,
}

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
                    re.as_ref()
                        .unwrap()
                        .find_iter(line)
                        .map(|m| (m.start(), m.end()))
                        .collect()
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
    use std::path::PathBuf;

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
        let (file, _dir) = make_file("test.rs", "fn hello() {\n    println!(\"world\");\n}");
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
        let (file, _dir) = make_file("test.rs", "fn hello() {\n    println!(\"world\");\n}");
        let hits = search(r"println!\(", &[file], &SearchMode::Regex).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn test_no_matches() {
        let (file, _dir) = make_file("test.rs", "fn hello() {}");
        let hits = search("xyz", &[file], &SearchMode::Literal).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let (file, _dir) = make_file("test.rs", "");
        let hits = search("hello", &[file], &SearchMode::Literal).unwrap();
        assert!(hits.is_empty());
    }
}
