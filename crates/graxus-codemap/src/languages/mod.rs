pub mod go;
pub mod python;
pub mod rust_lang;
pub mod typescript;

use std::collections::HashMap;

use crate::LanguageIndexer;

/// Create a registry of all supported language indexers.
pub fn registry() -> HashMap<&'static str, Box<dyn LanguageIndexer>> {
    let mut map: HashMap<&'static str, Box<dyn LanguageIndexer>> = HashMap::new();
    map.insert("typescript", Box::new(typescript::TypeScriptIndexer));
    map.insert("javascript", Box::new(typescript::TypeScriptIndexer));
    map.insert("rust", Box::new(rust_lang::RustIndexer));
    map.insert("go", Box::new(go::GoIndexer));
    map.insert("python", Box::new(python::PythonIndexer));
    map
}
