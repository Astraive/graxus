use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::GraxusConfig;
use crate::file_types::{self, FileKind, Language};

/// A file discovered during project scanning, with metadata and hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub kind: FileKind,
    pub language: Language,
    pub hash: String,
    pub size: u64,
    pub modified: DateTime<Utc>,
}

/// Scan the project directory and return all matching files.
pub fn scan(root: &Path, config: &GraxusConfig) -> Result<Vec<ScannedFile>> {
    let include_set = build_glob_set(&config.scan.include)?;
    let exclude_set = build_glob_set(&config.scan.exclude)?;

    let mut builder = WalkBuilder::new(root);
    builder
        .git_ignore(config.scan.respect_gitignore)
        .git_global(config.scan.respect_gitignore)
        .git_exclude(config.scan.respect_gitignore)
        .hidden(false);

    let mut files = Vec::new();

    for entry in builder.build() {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let relative_path = match path.strip_prefix(root) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };

        // Check include patterns
        if !include_set.is_empty() && !include_set.is_match(&relative_path) {
            continue;
        }

        // Check exclude patterns
        if !exclude_set.is_empty() && exclude_set.is_match(&relative_path) {
            continue;
        }

        // Skip binary files
        if file_types::is_binary(path) {
            continue;
        }

        let (kind, language) = file_types::detect_file(path);
        let metadata = fs::metadata(path).context("Failed to read file metadata")?;
        let modified: DateTime<Utc> = metadata
            .modified()
            .map(DateTime::from)
            .unwrap_or_else(|_| Utc::now());
        let size = metadata.len();
        let hash = hash_file(path)?;

        files.push(ScannedFile {
            path: path.to_path_buf(),
            relative_path,
            kind,
            language,
            hash,
            size,
            modified,
        });
    }

    tracing::info!("Scanned {} files", files.len());
    Ok(files)
}

/// Build a GlobSet from a list of patterns.
fn build_glob_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).with_context(|| format!("Invalid glob: {}", pattern))?;
        builder.add(glob);
    }
    builder.build().context("Failed to build glob set")
}

/// Compute SHA-256 hash of a file using streaming to avoid loading the entire file into memory.
fn hash_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = fs::File::open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).context("Failed to read file for hashing")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Diff between old and new scan results.
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub added: Vec<ScannedFile>,
    pub modified: Vec<ScannedFile>,
    pub deleted: Vec<String>, // relative paths
}

/// Compare old scan with new scan to find changed files.
pub fn compute_diff(old_files: &[ScannedFile], new_files: &[ScannedFile]) -> FileDiff {
    let old_map: HashMap<&str, &ScannedFile> = old_files
        .iter()
        .map(|f| (f.relative_path.as_str(), f))
        .collect();
    let new_map: HashMap<&str, &ScannedFile> = new_files
        .iter()
        .map(|f| (f.relative_path.as_str(), f))
        .collect();

    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    // Find added and modified files
    for (path, new_file) in &new_map {
        match old_map.get(path) {
            Some(old_file) => {
                if old_file.hash != new_file.hash {
                    modified.push((*new_file).clone());
                }
            }
            None => {
                added.push((*new_file).clone());
            }
        }
    }

    // Find deleted files
    for path in old_map.keys() {
        if !new_map.contains_key(path) {
            deleted.push(path.to_string());
        }
    }

    FileDiff {
        added,
        modified,
        deleted,
    }
}

/// Load saved file list from .graxus/files.json
pub fn load_saved_files(graxus_dir: &Path) -> Option<Vec<ScannedFile>> {
    let path = graxus_dir.join("files.json");
    if path.exists() {
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    } else {
        None
    }
}

/// Save file list to .graxus/files.json
pub fn save_saved_files(graxus_dir: &Path, files: &[ScannedFile]) -> Result<()> {
    let path = graxus_dir.join("files.json");
    let json = serde_json::to_string_pretty(files)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Scan and separate files into docs and code categories.
pub fn scan_categorized(
    root: &Path,
    config: &GraxusConfig,
) -> Result<(Vec<ScannedFile>, Vec<ScannedFile>, Vec<ScannedFile>)> {
    let all = scan(root, config)?;
    let mut docs = Vec::new();
    let mut code = Vec::new();
    let mut config_files = Vec::new();

    for file in all {
        match file.kind {
            FileKind::Doc => docs.push(file),
            FileKind::Code => code.push(file),
            FileKind::Config => config_files.push(file),
            FileKind::Unknown => {} // skip unknown
        }
    }

    Ok((docs, code, config_files))
}
