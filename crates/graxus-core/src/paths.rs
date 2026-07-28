//! Cross-platform path utilities.

use std::path::{Path, PathBuf};

/// Normalize a path to use forward slashes for cross-platform storage and comparison.
///
/// On Windows, converts `\\` to `/`. On Unix, this is a no-op.
pub fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Strip the Windows extended-length path prefix `\\?\` from a canonicalized path.
///
/// `std::fs::canonicalize` on Windows returns paths prefixed with `\\?\` (e.g.
/// `\\?\C:\Users\...`). This prefix causes `starts_with()` checks to fail when
/// comparing a prefixed path against a non-prefixed one. This helper strips it.
///
/// On non-Windows platforms, returns the path unchanged.
pub fn normalize_canonical(p: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = p.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_forward_slashes() {
        let path = Path::new("src/main.rs");
        assert_eq!(normalize_path(path), "src/main.rs");
    }

    #[cfg(windows)]
    #[test]
    fn test_normalize_path_backslashes() {
        let path = Path::new("src\\main.rs");
        assert_eq!(normalize_path(path), "src/main.rs");
    }

    #[test]
    fn test_normalize_path_nested() {
        let path = Path::new("crates/graxus-core/src/lib.rs");
        assert_eq!(normalize_path(path), "crates/graxus-core/src/lib.rs");
    }

    #[cfg(unix)]
    #[test]
    fn test_normalize_canonical_no_prefix() {
        let path = PathBuf::from("/some/path");
        assert_eq!(normalize_canonical(path), PathBuf::from("/some/path"));
    }

    #[cfg(windows)]
    #[test]
    fn test_normalize_canonical_strips_prefix() {
        let path = PathBuf::from(r"\\?\C:\Users\test\project");
        assert_eq!(
            normalize_canonical(path),
            PathBuf::from(r"C:\Users\test\project")
        );
    }

    #[cfg(windows)]
    #[test]
    fn test_normalize_canonical_no_prefix_windows() {
        let path = PathBuf::from(r"C:\Users\test\project");
        assert_eq!(
            normalize_canonical(path),
            PathBuf::from(r"C:\Users\test\project")
        );
    }
}
