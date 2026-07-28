use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::config::GraxusConfig;

/// Find the project root by looking for graxus.yaml or .graxus/ going upward.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start.to_path_buf());
    while let Some(dir) = current {
        if dir.join("graxus.yaml").exists() || dir.join(".graxus").is_dir() {
            return Some(dir);
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Initialize the .graxus directory structure.
pub fn init_graxus_dir(root: &Path) -> Result<PathBuf> {
    let graxus_dir = root.join(".graxus");
    let subdirs = ["docs", "code", "snapshots", "logs", "reports"];
    for subdir in &subdirs {
        std::fs::create_dir_all(graxus_dir.join(subdir))
            .with_context(|| format!("Failed to create .graxus/{}", subdir))?;
    }
    tracing::info!("Initialized .graxus directory at {}", graxus_dir.display());
    Ok(graxus_dir)
}

/// Initialize a new graxus project: create .graxus/ and graxus.yaml.
pub fn init_project(root: &Path) -> Result<GraxusConfig> {
    let config = GraxusConfig::default();
    init_graxus_dir(root)?;
    config.save(root)?;
    tracing::info!("Initialized graxus project at {}", root.display());
    Ok(config)
}

/// Get the .graxus directory path.
pub fn graxus_dir(root: &Path) -> PathBuf {
    root.join(".graxus")
}

/// Get the docs graph output directory.
pub fn docs_dir(root: &Path) -> PathBuf {
    root.join(".graxus").join("docs")
}

/// Get the code codemap output directory.
pub fn code_dir(root: &Path) -> PathBuf {
    root.join(".graxus").join("code")
}

/// Get the snapshots directory.
pub fn snapshots_dir(root: &Path) -> PathBuf {
    root.join(".graxus").join("snapshots")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_init_graxus_dir() {
        let dir = tempdir().unwrap();
        let graxus_dir = init_graxus_dir(dir.path()).unwrap();
        assert!(graxus_dir.exists());
        assert!(graxus_dir.join("docs").exists());
        assert!(graxus_dir.join("code").exists());
        assert!(graxus_dir.join("snapshots").exists());
    }

    #[test]
    fn test_init_project() {
        let dir = tempdir().unwrap();
        let _config = init_project(dir.path()).unwrap();
        assert!(dir.path().join("graxus.yaml").exists());
        assert!(dir.path().join(".graxus").exists());
    }

    #[test]
    fn test_find_root_found() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&sub).unwrap();
        init_graxus_dir(dir.path()).unwrap();
        let found = find_root(&sub);
        assert!(found.is_some());
        assert_eq!(found.unwrap(), dir.path());
    }

    #[test]
    fn test_find_root_not_found() {
        let dir = tempdir().unwrap();
        let found = find_root(dir.path());
        assert!(found.is_none());
    }

    #[test]
    fn test_docs_dir() {
        let dir = tempdir().unwrap();
        let expected = dir.path().join(".graxus").join("docs");
        assert_eq!(docs_dir(dir.path()), expected);
    }

    #[test]
    fn test_code_dir() {
        let dir = tempdir().unwrap();
        let expected = dir.path().join(".graxus").join("code");
        assert_eq!(code_dir(dir.path()), expected);
    }

    #[test]
    fn test_snapshots_dir() {
        let dir = tempdir().unwrap();
        let expected = dir.path().join(".graxus").join("snapshots");
        assert_eq!(snapshots_dir(dir.path()), expected);
    }
}
