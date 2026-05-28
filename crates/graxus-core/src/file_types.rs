use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Code,
    Doc,
    Config,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Go,
    Python,
    Markdown,
    Html,
    Css,
    Toml,
    Yaml,
    Json,
    #[serde(rename = "unknown")]
    Unknown,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
            Language::Go => "go",
            Language::Python => "python",
            Language::Markdown => "markdown",
            Language::Html => "html",
            Language::Css => "css",
            Language::Toml => "toml",
            Language::Yaml => "yaml",
            Language::Json => "json",
            Language::Unknown => "unknown",
        }
    }
}

/// Detect file kind and language from the file extension.
pub fn detect_file(path: &Path) -> (FileKind, Language) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "rs" => (FileKind::Code, Language::Rust),
        "ts" | "mts" | "cts" => (FileKind::Code, Language::TypeScript),
        "tsx" => (FileKind::Code, Language::TypeScript),
        "js" | "mjs" | "cjs" => (FileKind::Code, Language::JavaScript),
        "jsx" => (FileKind::Code, Language::JavaScript),
        "go" => (FileKind::Code, Language::Go),
        "py" | "pyi" => (FileKind::Code, Language::Python),
        "md" | "mdx" => (FileKind::Doc, Language::Markdown),
        "txt" => (FileKind::Doc, Language::Unknown),
        "html" | "htm" => (FileKind::Code, Language::Html),
        "css" | "scss" | "sass" | "less" => (FileKind::Code, Language::Css),
        "toml" => (FileKind::Config, Language::Toml),
        "yaml" | "yml" => (FileKind::Config, Language::Yaml),
        "json" | "jsonc" => (FileKind::Config, Language::Json),
        "lock" => (FileKind::Config, Language::Unknown),
        _ => (FileKind::Unknown, Language::Unknown),
    }
}

/// Detect only the language from extension.
pub fn detect_language(path: &Path) -> Language {
    detect_file(path).1
}

/// Detect only the file kind from extension.
pub fn detect_kind(path: &Path) -> FileKind {
    detect_file(path).0
}

/// Check if the file is a binary based on extension.
pub fn is_binary(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        ext.as_str(),
        "exe" | "dll" | "so" | "dylib" | "bin" | "o" | "a" | "lib"
            | "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "svg" | "webp"
            | "mp3" | "mp4" | "wav" | "avi" | "mov" | "mkv" | "flv" | "webm"
            | "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar"
            | "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx"
            | "woff" | "woff2" | "ttf" | "otf" | "eot"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rust_file() {
        assert_eq!(detect_language(Path::new("main.rs")), Language::Rust);
    }

    #[test]
    fn test_detect_typescript_file() {
        assert_eq!(detect_language(Path::new("index.ts")), Language::TypeScript);
    }

    #[test]
    fn test_detect_markdown_file() {
        assert_eq!(detect_language(Path::new("README.md")), Language::Markdown);
    }

    #[test]
    fn test_detect_unknown_file() {
        assert_eq!(detect_language(Path::new("data.xyz")), Language::Unknown);
    }

    #[test]
    fn test_detect_file_rust() {
        let (kind, lang) = detect_file(Path::new("main.rs"));
        assert_eq!(kind, FileKind::Code);
        assert_eq!(lang, Language::Rust);
    }

    #[test]
    fn test_detect_file_config() {
        let (kind, lang) = detect_file(Path::new("Cargo.toml"));
        assert_eq!(kind, FileKind::Config);
        assert_eq!(lang, Language::Toml);
    }

    #[test]
    fn test_detect_file_doc() {
        let (kind, lang) = detect_file(Path::new("README.md"));
        assert_eq!(kind, FileKind::Doc);
        assert_eq!(lang, Language::Markdown);
    }

    #[test]
    fn test_detect_kind_code() {
        assert_eq!(detect_kind(Path::new("main.rs")), FileKind::Code);
        assert_eq!(detect_kind(Path::new("index.ts")), FileKind::Code);
        assert_eq!(detect_kind(Path::new("main.py")), FileKind::Code);
        assert_eq!(detect_kind(Path::new("app.go")), FileKind::Code);
    }

    #[test]
    fn test_detect_kind_config() {
        assert_eq!(detect_kind(Path::new("config.toml")), FileKind::Config);
        assert_eq!(detect_kind(Path::new("settings.yaml")), FileKind::Config);
        assert_eq!(detect_kind(Path::new("data.json")), FileKind::Config);
    }

    #[test]
    fn test_is_binary() {
        assert!(is_binary(Path::new("image.png")));
        assert!(is_binary(Path::new("archive.zip")));
        assert!(is_binary(Path::new("lib.so")));
        assert!(!is_binary(Path::new("main.rs")));
        assert!(!is_binary(Path::new("README.md")));
    }

    #[test]
    fn test_language_as_str() {
        assert_eq!(Language::Rust.as_str(), "rust");
        assert_eq!(Language::TypeScript.as_str(), "typescript");
        assert_eq!(Language::Go.as_str(), "go");
        assert_eq!(Language::Python.as_str(), "python");
        assert_eq!(Language::Markdown.as_str(), "markdown");
    }
}
