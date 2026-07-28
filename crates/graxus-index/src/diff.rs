use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDiff {
    pub file: String,
    pub old_content: String,
    pub new_content: String,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub removed: Vec<String>,
    pub added: Vec<String>,
}

/// Compute diff between two strings.
pub fn compute_diff(old: &str, new: &str) -> Vec<DiffHunk> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut hunks = Vec::new();

    let mut old_idx = 0;
    let mut new_idx = 0;

    while old_idx < old_lines.len() || new_idx < new_lines.len() {
        if old_idx < old_lines.len()
            && new_idx < new_lines.len()
            && old_lines[old_idx] == new_lines[new_idx]
        {
            old_idx += 1;
            new_idx += 1;
            continue;
        }

        let hunk_start_old = old_idx;
        let hunk_start_new = new_idx;
        let mut removed = Vec::new();
        let mut added = Vec::new();

        while old_idx < old_lines.len()
            && (new_idx >= new_lines.len() || old_lines[old_idx] != new_lines[new_idx])
        {
            removed.push(old_lines[old_idx].to_string());
            old_idx += 1;
        }

        while new_idx < new_lines.len()
            && (old_idx >= old_lines.len() || old_lines[old_idx] != new_lines[new_idx])
        {
            added.push(new_lines[new_idx].to_string());
            new_idx += 1;
        }

        if !removed.is_empty() || !added.is_empty() {
            hunks.push(DiffHunk {
                old_start: hunk_start_old + 1,
                old_lines: removed.len(),
                new_start: hunk_start_new + 1,
                new_lines: added.len(),
                removed,
                added,
            });
        }
    }

    hunks
}

/// Format diff as unified diff string.
pub fn format_diff(diff: &SnapshotDiff) -> String {
    let mut output = String::new();
    output.push_str(&format!("--- a/{}\n", diff.file));
    output.push_str(&format!("+++ b/{}\n", diff.file));

    for hunk in &diff.hunks {
        output.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
        ));
        for line in &hunk.removed {
            output.push_str(&format!("-{}\n", line));
        }
        for line in &hunk.added {
            output.push_str(&format!("+{}\n", line));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_diff_no_changes() {
        let hunks = compute_diff("goodbye\nworld\n", "goodbye\nworld\n");
        assert!(hunks.is_empty());
    }

    #[test]
    fn test_compute_diff_added_line() {
        let hunks = compute_diff("goodbye\n", "goodbye\nworld\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].added, vec!["world"]);
        assert!(hunks[0].removed.is_empty());
    }

    #[test]
    fn test_compute_diff_removed_line() {
        let hunks = compute_diff("goodbye\nworld\n", "goodbye\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].removed, vec!["world"]);
        assert!(hunks[0].added.is_empty());
    }

    #[test]
    fn test_compute_diff_changed_line() {
        let hunks = compute_diff("goodbye\nworld\n", "goodbye\nrust\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].removed, vec!["world"]);
        assert_eq!(hunks[0].added, vec!["rust"]);
    }

    #[test]
    fn test_format_diff() {
        let diff = SnapshotDiff {
            file: "test.rs".to_string(),
            old_content: "fn main() {}\n".to_string(),
            new_content: "fn main() {\n    println!(\"hi\");\n}\n".to_string(),
            hunks: compute_diff("fn main() {}\n", "fn main() {\n    println!(\"hi\");\n}\n"),
        };
        let formatted = format_diff(&diff);
        assert!(formatted.contains("--- a/test.rs"));
        assert!(formatted.contains("+++ b/test.rs"));
    }
}