use anyhow::{bail, Context, Result};
use graxus_core::ScannedFile;
use serde::{Deserialize, Serialize};

use crate::safety;

/// Controls how the search-and-replace pattern is interpreted.
#[derive(Debug, Clone)]
pub enum ReplaceMode {
    /// Match the pattern as a plain string.
    Literal,
    /// Match the pattern as a regular expression.
    Regex,
}

/// Preview of a replace operation before it is applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplacePreview {
    /// The original search pattern.
    pub old: String,
    /// The replacement string.
    pub new: String,
    /// Mode used (`"literal"` or `"regex"`).
    pub mode: String,
    /// Files that would be modified.
    pub affected_files: Vec<FileChange>,
    /// Total number of replacements across all files.
    pub total_replacements: usize,
}

/// Changes planned for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// Relative path of the file.
    pub file: String,
    /// Number of line-level replacements in this file.
    pub replacements: usize,
    /// Before/after preview for each changed line.
    pub preview_lines: Vec<PreviewLine>,
}

/// A single line-level before/after diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewLine {
    /// 1-based line number.
    pub line_num: usize,
    /// Original line content.
    pub before: String,
    /// Line content after replacement.
    pub after: String,
}

/// Build a preview of a replace operation without modifying any files.
///
/// Scans up to `max_files` files and records per-line before/after diffs.
/// Files that fail safety checks or cannot be read are silently skipped.
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
                ReplaceMode::Regex => {
                    let re = re
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("regex not compiled for Regex mode"))?;
                    re.replace_all(line, new).into_owned()
                }
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

/// Apply a previously previewed replace operation, writing changes to disk.
///
/// Each file is read, modified in memory using the preview's line-level diffs,
/// and written back. Returns an error if any file cannot be read or written.
///
/// # TOCTOU Protection
///
/// Before applying changes, verifies that the file's content still matches
/// the preview. If the file was modified between preview and apply, the
/// operation is rejected with an error to prevent silent data corruption.
pub fn apply_replace(preview: &ReplacePreview) -> Result<()> {
    for change in &preview.affected_files {
        let path = std::path::PathBuf::from(&change.file);
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        // Verify the file hasn't changed since preview by checking that
        // all "before" lines still exist at their expected positions.
        let lines: Vec<&str> = content.lines().collect();
        for pl in &change.preview_lines {
            if pl.line_num == 0 || pl.line_num > lines.len() {
                bail!(
                    "File {} has changed since preview: line {} no longer exists (file may have been modified)",
                    path.display(),
                    pl.line_num
                );
            }
            if lines[pl.line_num - 1] != pl.before {
                bail!(
                    "File {} has changed since preview: line {} content differs \
                     (expected '{}', found '{}'). Re-run preview to generate a fresh plan.",
                    path.display(),
                    pl.line_num,
                    pl.before,
                    lines[pl.line_num - 1]
                );
            }
        }

        let mut lines: Vec<String> = lines.into_iter().map(|l| l.to_string()).collect();

        for pl in &change.preview_lines {
            if pl.line_num > 0 && pl.line_num <= lines.len() {
                lines[pl.line_num - 1] = pl.after.clone();
            }
        }

        let mut result = lines.join("\n");
        // Preserve trailing newline from original file
        if content.ends_with('\n') {
            result.push('\n');
        }

        std::fs::write(&path, result)
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
        let (file, _dir) = make_file("test.rs", "fn goodbye() {}\nfn world() {}");
        let preview =
            preview_replace("goodbye", "greet", &[file], &ReplaceMode::Literal, 100).unwrap();
        assert_eq!(preview.total_replacements, 1);
        assert_eq!(preview.affected_files.len(), 1);
        assert_eq!(preview.affected_files[0].replacements, 1);
    }

    #[test]
    fn test_preview_no_matches() {
        let (file, _dir) = make_file("test.rs", "fn goodbye() {}");
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
        // total_replacements counts lines changed, not individual matches
        assert_eq!(preview.total_replacements, 1);
        assert_eq!(preview.mode, "regex");
        // Both matches are on the same line, so the after should reflect both
        assert_eq!(preview.affected_files[0].preview_lines[0].after, "bar bar");
    }

    #[test]
    fn test_preview_multiple_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = Vec::new();
        for (name, content) in [("a.rs", "fn foo() {}"), ("b.rs", "fn bar() { foo(); }")] {
            let full_path = dir.path().join(name);
            std::fs::write(&full_path, content).unwrap();
            files.push(ScannedFile {
                path: full_path,
                relative_path: name.to_string(),
                kind: FileKind::Code,
                language: Language::Rust,
                hash: "test".to_string(),
                size: content.len() as u64,
                modified: chrono::Utc::now(),
            });
        }
        let preview = preview_replace("foo", "baz", &files, &ReplaceMode::Literal, 100).unwrap();
        assert_eq!(preview.affected_files.len(), 2);
        assert_eq!(preview.total_replacements, 2);
    }

    #[test]
    fn test_preview_max_files_limit() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = Vec::new();
        for i in 0..5 {
            let name = format!("f{}.rs", i);
            let content = format!("fn foo{}() {{ foo(); }}", i);
            let full_path = dir.path().join(&name);
            std::fs::write(&full_path, &content).unwrap();
            files.push(ScannedFile {
                path: full_path,
                relative_path: name,
                kind: FileKind::Code,
                language: Language::Rust,
                hash: "test".to_string(),
                size: content.len() as u64,
                modified: chrono::Utc::now(),
            });
        }
        let preview = preview_replace("foo", "bar", &files, &ReplaceMode::Literal, 2).unwrap();
        assert_eq!(preview.affected_files.len(), 2);
    }

    /// Helper that creates a file in a temp dir and returns a ScannedFile with
    /// the full path as relative_path, so apply_replace can read/write it.
    fn make_file_full_path(path: &str, content: &str) -> (ScannedFile, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let full_path = dir.path().join(path);
        std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        std::fs::write(&full_path, content).unwrap();
        let scanned = ScannedFile {
            path: full_path.clone(),
            relative_path: full_path.to_string_lossy().to_string(),
            kind: FileKind::Code,
            language: Language::Rust,
            hash: "test".to_string(),
            size: content.len() as u64,
            modified: chrono::Utc::now(),
        };
        (scanned, dir)
    }

    #[test]
    fn test_apply_replace_writes_changes() {
        let (file, dir) = make_file_full_path("test.rs", "fn goodbye() {}\nfn world() {}");
        let preview =
            preview_replace("goodbye", "greet", &[file], &ReplaceMode::Literal, 100).unwrap();
        assert_eq!(preview.total_replacements, 1);

        apply_replace(&preview).unwrap();

        let content = std::fs::read_to_string(dir.path().join("test.rs")).unwrap();
        assert_eq!(content, "fn greet() {}\nfn world() {}");
    }

    #[test]
    fn test_apply_replace_rollback_by_re_previewing() {
        let (file, dir) = make_file_full_path("test.rs", "alpha beta gamma");

        // Step 1: Replace alpha -> omega
        let preview1 = preview_replace(
            "alpha",
            "omega",
            std::slice::from_ref(&file),
            &ReplaceMode::Literal,
            100,
        )
        .unwrap();
        apply_replace(&preview1).unwrap();
        let after1 = std::fs::read_to_string(dir.path().join("test.rs")).unwrap();
        assert_eq!(after1, "omega beta gamma");

        // Step 2: Replace omega -> alpha (rollback)
        let preview2 =
            preview_replace("omega", "alpha", &[file], &ReplaceMode::Literal, 100).unwrap();
        apply_replace(&preview2).unwrap();
        let after2 = std::fs::read_to_string(dir.path().join("test.rs")).unwrap();
        assert_eq!(after2, "alpha beta gamma");
    }

    #[test]
    fn test_preview_regex_with_groups() {
        let (file, _dir) = make_file("test.rs", "let x = 42;\nlet y = 100;");
        let preview = preview_replace(
            r"let (\w+) = (\d+);",
            r"const $1: i32 = $2;",
            &[file],
            &ReplaceMode::Regex,
            100,
        )
        .unwrap();
        assert_eq!(preview.total_replacements, 2);
        assert_eq!(
            preview.affected_files[0].preview_lines[0].after,
            "const x: i32 = 42;"
        );
        assert_eq!(
            preview.affected_files[0].preview_lines[1].after,
            "const y: i32 = 100;"
        );
    }
}
