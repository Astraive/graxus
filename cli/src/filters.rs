//! Shared glob/language filter helpers for `--include` / `--exclude` / `--lang`.
//!
//! Centralized so `index`, `replace`, and any future command apply identical
//! semantics to user-supplied file filters.

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};

use graxus_core::scanner;

/// Build a `GlobSet` from user-supplied glob patterns. An empty input yields an
/// empty set, which callers treat as "match everything".
pub fn build_glob_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).with_context(|| format!("Invalid glob: {}", pattern))?;
        builder.add(glob);
    }
    builder.build().context("Failed to build glob set")
}

/// Apply `--include` / `--exclude` / `--lang` filters to a vector of scanned
/// files in place. Files are matched on their slash-normalized relative path.
pub fn apply_filters(
    files: &mut Vec<scanner::ScannedFile>,
    include: &GlobSet,
    exclude: &GlobSet,
    lang: &[String],
) {
    if !include.is_empty() {
        files.retain(|f| include.is_match(&f.relative_path));
    }
    if !exclude.is_empty() {
        files.retain(|f| !exclude.is_match(&f.relative_path));
    }
    if !lang.is_empty() {
        files.retain(|f| {
            lang.iter()
                .any(|l| l.eq_ignore_ascii_case(f.language.as_str()))
        });
    }
}
