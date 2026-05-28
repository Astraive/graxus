use std::path::Path;

fn contains_git_dir(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == ".git")
}

pub fn is_safe_to_read(path: &Path) -> bool {
    !contains_git_dir(path)
}

pub fn is_safe_to_edit(path: &Path) -> bool {
    if contains_git_dir(path) {
        return false;
    }
    let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    if name.ends_with(".lock") || name == ".env" || name.starts_with(".env.") {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_to_read_normal() {
        assert!(is_safe_to_read(Path::new("src/main.rs")));
    }

    #[test]
    fn test_safe_to_read_git_dir() {
        assert!(!is_safe_to_read(Path::new(".git/config")));
    }

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
}
