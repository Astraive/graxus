/// Build configuration parsers for various ecosystems (Cargo, npm, tsconfig, Python, Go, CMake, dotnet).
pub mod build_config;
/// Project configuration management (graxus.yaml).
pub mod config;
/// Dependency detection from manifest files (Cargo.toml, package.json, etc.).
pub mod dependencies;
/// File type and language detection from extensions.
pub mod file_types;
/// Cross-platform path utilities.
pub mod paths;
/// Plugin system for extending graxus functionality.
pub mod plugins;
/// File scanning, hashing, and diff computation.
pub mod scanner;
/// Workspace detection (Cargo, npm, Go).
pub mod workspace;
/// Multi-workspace detection and file-to-workspace mapping.
pub mod workspaces;

pub use config::{GraxusConfig, ParserBackend};
pub use file_types::{FileKind, Language};
pub use scanner::ScannedFile;
