//! Language-specific indexers for code extraction using tree-sitter.

pub mod c;
pub mod cpp;
pub mod csharp;
pub mod go;
pub mod java;
pub mod kotlin;
pub mod python;
pub mod rust_lang;
pub mod swift;
pub mod typescript;
pub mod visibility;

use std::collections::HashMap;

use crate::LanguageIndexer;

/// Create a registry of all supported language indexers.
///
/// Returns a map from language identifier to its corresponding indexer.
/// Each indexer has pre-compiled tree-sitter queries for efficient extraction.
pub fn registry() -> HashMap<&'static str, Box<dyn LanguageIndexer>> {
    let mut map: HashMap<&'static str, Box<dyn LanguageIndexer>> = HashMap::new();
    map.insert("typescript", Box::new(typescript::TypeScriptIndexer::new()));
    map.insert("javascript", Box::new(typescript::TypeScriptIndexer::new()));
    map.insert("rust", Box::new(rust_lang::RustIndexer::new()));
    map.insert("go", Box::new(go::GoIndexer::new()));
    map.insert("python", Box::new(python::PythonIndexer::new()));
    map.insert("c", Box::new(c::CIndexer::new()));
    map.insert("cpp", Box::new(cpp::CppIndexer::new()));
    map.insert("csharp", Box::new(csharp::CSharpIndexer::new()));
    map.insert("java", Box::new(java::JavaIndexer::new()));
    map.insert("kotlin", Box::new(kotlin::KotlinIndexer::new()));
    map.insert("swift", Box::new(swift::SwiftIndexer::new()));
    map
}
