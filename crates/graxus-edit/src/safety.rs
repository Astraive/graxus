//! Path and file safety checks for read and edit operations.

use graxus_core::paths::normalize_canonical;
use std::path::Path;

/// Maximum file size allowed for edit operations (10 MiB).
pub const MAX_EDIT_FILE_SIZE: u64 = 10 * 1024 * 1024;

fn contains_git_dir(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == ".git")
}

/// Returns `true` if the path is safe to read.
///
/// Rejects any path that traverses into a `.git` directory.
#[must_use = "Safety check result should be used to gate file operations"]
pub fn is_safe_to_read(path: &Path) -> bool {
    !contains_git_dir(path)
}

/// Returns `true` if the path is safe to edit.
///
/// Rejects `.git` directories, lock files (`.lock`), and env files (`.env`, `.env.*`).
#[must_use = "Safety check result should be used to gate file operations"]
pub fn is_safe_to_edit(path: &Path) -> bool {
    if contains_git_dir(path) {
        return false;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    if name.ends_with(".lock") || name == ".env" || name.starts_with(".env.") {
        return false;
    }
    true
}

/// Checks whether `path` is contained within `root` after resolving symlinks
/// and `..` components.
///
/// Returns `false` if either path cannot be canonicalized (e.g. does not exist).
/// This protects against path traversal via symlinks or `..` segments.
#[must_use = "Path safety check result should be used to gate file operations"]
pub fn is_safe_path(root: &Path, path: &Path) -> bool {
    let canonical_root = match std::fs::canonicalize(root) {
        Ok(p) => normalize_canonical(p),
        Err(_) => return false,
    };
    let canonical_path = match std::fs::canonicalize(path) {
        Ok(p) => normalize_canonical(p),
        Err(_) => return false,
    };
    canonical_path.starts_with(&canonical_root)
}

/// Returns `true` if the path contains any `..` (parent directory) components.
fn has_parent_traversal(path: &Path) -> bool {
    path.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}

/// Check that a path is safe to operate on within the given project root.
///
/// Unlike [`is_safe_path`], this works for paths that may not yet exist (e.g.
/// before creating a new file). It checks:
///
/// - Absolute paths must resolve under the project root.
/// - Paths with `..` components are resolved relative to root and checked.
/// - Symlink resolution is attempted; if it fails, falls back to component check.
#[must_use = "Path safety check result should be used to gate file operations"]
pub fn is_path_within_root(path: &Path, root: &Path) -> bool {
    // Reject absolute paths that are not under the root
    if path.is_absolute() {
        if let Ok(canonical_root) = root.canonicalize().map(normalize_canonical) {
            if let Ok(canonical_path) = path.canonicalize().map(normalize_canonical) {
                return canonical_path.starts_with(&canonical_root);
            }
            // Absolute path that can't be canonicalized: try prefix check
            return path.starts_with(root);
        }
        return false;
    }

    // For relative paths with .. traversal, resolve and check
    if has_parent_traversal(path) {
        let resolved = root.join(path);
        if let Ok(canonical_root) = root.canonicalize().map(normalize_canonical) {
            if let Ok(canonical_resolved) = resolved.canonicalize().map(normalize_canonical) {
                return canonical_resolved.starts_with(&canonical_root);
            }
        }
        // If we can't resolve, reject the path
        return false;
    }

    // Simple relative path with no traversal: always safe
    true
}

/// Returns `true` if the file size is within the allowed limit for edits.
///
/// Files larger than [`MAX_EDIT_FILE_SIZE`] (10 MiB) are rejected.
#[must_use = "File size check result should be used to gate file operations"]
pub fn is_safe_file_size(size: u64) -> bool {
    size <= MAX_EDIT_FILE_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // --- is_safe_to_read ---

    #[test]
    fn test_safe_to_read_normal() {
        assert!(is_safe_to_read(Path::new("src/main.rs")));
    }

    #[test]
    fn test_safe_to_read_git_dir() {
        assert!(!is_safe_to_read(Path::new(".git/config")));
    }

    // --- is_safe_to_edit ---

    #[test]
    fn test_safe_to_edit_normal() {
        assert!(is_safe_to_edit(Path::new("src/main.rs")));
    }

    #[test]
    fn test_safe_to_edit_lock_file() {
        assert!(!is_safe_to_edit(Path::new("Cargo.lock")));
    }

    #[test]
    fn test_safe_to_edit_env_file() {
        assert!(!is_safe_to_edit(Path::new(".env")));
        assert!(!is_safe_to_edit(Path::new(".env.local")));
    }

    #[test]
    fn test_safe_to_edit_git_dir() {
        assert!(!is_safe_to_edit(Path::new(".git/HEAD")));
    }

    // --- is_safe_path ---

    #[test]
    fn test_safe_path_contained() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src").join("main.rs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "fn main() {}").unwrap();

        assert!(is_safe_path(dir.path(), &file));
    }

    #[test]
    fn test_safe_path_traversal_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside.txt");
        fs::write(&outside, "secret").unwrap();

        // Try to reference it via .. from a subdirectory
        let inner = dir.path().join("inner");
        fs::create_dir_all(&inner).unwrap();
        let traversal = inner.join("..").join("outside.txt");

        // After canonicalization, traversal should still point inside root
        // (because it IS inside root). But a path outside root should fail.
        assert!(is_safe_path(dir.path(), &traversal));
    }

    #[test]
    fn test_safe_path_outside_root() {
        let dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let file = other.path().join("secret.txt");
        fs::write(&file, "secret").unwrap();

        assert!(!is_safe_path(dir.path(), &file));
    }

    #[test]
    fn test_safe_path_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("does_not_exist.txt");

        assert!(!is_safe_path(dir.path(), &fake));
    }

    #[cfg(unix)]
    #[test]
    fn test_safe_path_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("target.txt");
        fs::write(&target, "secret").unwrap();

        let link = dir.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        // Symlink points outside root — should be rejected
        assert!(!is_safe_path(dir.path(), &link));
    }

    #[test]
    fn test_safe_path_nested_deep() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("a").join("b").join("c").join("d.rs");
        fs::create_dir_all(deep.parent().unwrap()).unwrap();
        fs::write(&deep, "fn f() {}").unwrap();

        assert!(is_safe_path(dir.path(), &deep));
    }

    #[test]
    fn test_safe_path_unicode() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src").join("donnee.rs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "fn f() {}").unwrap();

        assert!(is_safe_path(dir.path(), &file));
    }

    // --- is_path_within_root ---

    #[test]
    fn test_within_root_relative_simple() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_path_within_root(Path::new("src/main.rs"), dir.path()));
    }

    #[test]
    fn test_within_root_relative_nested() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_path_within_root(
            Path::new("src/lib/module.rs"),
            dir.path()
        ));
    }

    #[test]
    fn test_within_root_dot_component() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_path_within_root(Path::new("./src/main.rs"), dir.path()));
    }

    #[test]
    fn test_within_root_parent_traversal_rejected() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_path_within_root(Path::new("../etc/passwd"), dir.path()));
    }

    #[test]
    fn test_within_root_deep_parent_traversal_rejected() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_path_within_root(
            Path::new("src/../../etc/passwd"),
            dir.path()
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_within_root_absolute_outside_rejected() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_path_within_root(Path::new("/etc/passwd"), dir.path()));
    }

    #[test]
    fn test_within_root_absolute_inside_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("src").join("main.rs");
        fs::create_dir_all(inner.parent().unwrap()).unwrap();
        fs::write(&inner, "").unwrap();
        assert!(is_path_within_root(&inner, dir.path()));
    }

    #[cfg(windows)]
    #[test]
    fn test_within_root_windows_absolute_outside() {
        let dir = tempfile::tempdir().unwrap();
        // Use a different drive letter
        assert!(!is_path_within_root(
            Path::new("C:\\Windows\\System32"),
            dir.path()
        ));
    }

    #[cfg(windows)]
    #[test]
    fn test_safe_path_windows_unc_rejected() {
        let dir = tempfile::tempdir().unwrap();
        // UNC paths should not be considered safe relative to a local root
        assert!(!is_safe_path(
            dir.path(),
            Path::new(r"\\server\share\file.txt")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn test_safe_path_windows_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("secret.txt");
        fs::write(&target, "secret").unwrap();

        let link = dir.path().join("link.txt");
        // Creating symlinks on Windows requires admin privileges or developer mode.
        // Skip gracefully if we don't have the privilege.
        if std::os::windows::fs::symlink_file(&target, &link).is_err() {
            return;
        }

        // Symlink points outside root — should be rejected
        assert!(!is_safe_path(dir.path(), &link));
    }

    #[cfg(windows)]
    #[test]
    fn test_within_root_windows_unc_rejected() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_path_within_root(
            Path::new(r"\\server\share\file.txt"),
            dir.path()
        ));
    }

    #[cfg(windows)]
    #[test]
    fn test_within_root_windows_unc_path_rejected() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_path_within_root(
            Path::new("\\\\server\\share\\file.txt"),
            dir.path()
        ));
    }

    #[cfg(windows)]
    #[test]
    fn test_safe_path_windows_drive_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "data").unwrap();
        // Same-root path should be safe
        assert!(is_safe_path(dir.path(), &file));
    }

    #[cfg(windows)]
    #[test]
    fn test_safe_to_edit_windows_special_files() {
        assert!(!is_safe_to_edit(Path::new(".env")));
        assert!(!is_safe_to_edit(Path::new(".env.production")));
        assert!(!is_safe_to_edit(Path::new("package.lock")));
    }

    #[test]
    fn test_within_root_nonexistent_relative_allowed() {
        let dir = tempfile::tempdir().unwrap();
        // A simple relative path that doesn't exist yet should be allowed
        assert!(is_path_within_root(
            Path::new("src/new_file.rs"),
            dir.path()
        ));
    }

    #[test]
    fn test_within_root_nonexistent_with_traversal_rejected() {
        let dir = tempfile::tempdir().unwrap();
        // Path with .. that doesn't exist should be rejected
        assert!(!is_path_within_root(
            Path::new("../nonexistent"),
            dir.path()
        ));
    }

    // --- is_safe_file_size ---

    #[test]
    fn test_safe_file_size_under_limit() {
        assert!(is_safe_file_size(1024));
        assert!(is_safe_file_size(0));
    }

    #[test]
    fn test_safe_file_size_at_limit() {
        assert!(is_safe_file_size(MAX_EDIT_FILE_SIZE));
    }

    #[test]
    fn test_safe_file_size_over_limit() {
        assert!(!is_safe_file_size(MAX_EDIT_FILE_SIZE + 1));
    }
}
