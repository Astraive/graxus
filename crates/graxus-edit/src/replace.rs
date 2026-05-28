use anyhow::{Context, Result};
use graxus_core::ScannedFile;
use serde::{Deserialize, Serialize};

use crate::safety;

#[derive(Debug, Clone)]
pub enum ReplaceMode {
    Literal,
    Regex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplacePreview {
    pub old: String,
    pub new: String,
    pub mode: String,
    pub affected_files: Vec<FileChange>,
    pub total_replacements: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub file: String,
    pub replacements: usize,
    pub preview_lines: Vec<PreviewLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewLine {
    pub line_num: usize,
    pub before: String,
    pub after: String,
}

pub fn preview_replace(
    old: &str,
    new: &str,
    files: &[ScannedFile],
    mode: &ReplaceMode,
    max_files: usize,
) -> Result<ReplacePreview> {
    let re = match mode {
        ReplaceMode::Regex => Some(regex::Regex::new(old)?),
        ReplaceMode::Literal => None,
    };

    let mut affected_files = Vec::new();
    let mut total = 0;

    for file in files {
        if affected_files.len() >= max_files {
            break;
        }
        if !safety::is_safe_to_edit(&file.path) {
            continue;
        }
        let content = match std::fs::read_to_string(&file.path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut file_replacements = 0;
        let mut preview_lines = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let replaced = match mode {
                ReplaceMode::Literal => line.replace(old, new),
                ReplaceMode::Regex => re.as_ref().unwrap().replace_all(line, new).into_owned(),
            };

            if replaced != line {
                file_replacements += 1;
                preview_lines.push(PreviewLine {
                    line_num: line_num + 1,
                    before: line.to_string(),
                    after: replaced,
                });
            }
        }

        if file_replacements > 0 {
            affected_files.push(FileChange {
                file: file.relative_path.clone(),
                replacements: file_replacements,
                preview_lines,
            });
            total += file_replacements;
        }
    }

    Ok(ReplacePreview {
        old: old.to_string(),
        new: new.to_string(),
        mode: match mode {
            ReplaceMode::Literal => "literal".into(),
            ReplaceMode::Regex => "regex".into(),
        },
        affected_files,
        total_replacements: total,
    })
}

pub fn apply_replace(preview: &ReplacePreview) -> Result<()> {
    for change in &preview.affected_files {
        let path = std::path::PathBuf::from(&change.file);
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

        for pl in &change.preview_lines {
            if pl.line_num > 0 && pl.line_num <= lines.len() {
                lines[pl.line_num - 1] = pl.after.clone();
            }
        }

        std::fs::write(&path, lines.join("\n"))
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }
    Ok(())
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
    fn test_preview_replace_literal() {
        let (file, _dir) = make_file("test.rs", "fn hello() {}\nfn world() {}");
        let preview = preview_replace("hello", "greet", &[file], &ReplaceMode::Literal, 100).unwrap();
        assert_eq!(preview.total_replacements, 1);
        assert_eq!(preview.affected_files.len(), 1);
        assert_eq!(preview.affected_files[0].replacements, 1);
    }

    #[test]
    fn test_preview_no_matches() {
        let (file, _dir) = make_file("test.rs", "fn hello() {}");
        let preview = preview_replace("xyz", "abc", &[file], &ReplaceMode::Literal, 100).unwrap();
        assert_eq!(preview.total_replacements, 0);
        assert!(preview.affected_files.is_empty());
    }

    #[test]
    fn test_preview_multiple_lines() {
        let (file, _dir) = make_file("test.rs", "foo\nfoo\nfoo");
        let preview = preview_replace("foo", "bar", &[file], &ReplaceMode::Literal, 100).unwrap();
        assert_eq!(preview.total_replacements, 3);
    }

    #[test]
    fn test_preview_regex() {
        let (file, _dir) = make_file("test.rs", "foo123 foo456");
        let preview = preview_replace(r"foo\d+", "bar", &[file], &ReplaceMode::Regex, 100).unwrap();
        assert!(preview.total_replacements >= 1, "Expected at least 1 replacement, got {}", preview.total_replacements);
    }
}
