use serde::{Deserialize, Serialize};

/// Dependency injection registration/binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DIFact {
    /// Unique identifier.
    pub id: String,
    /// File where this DI binding is registered.
    pub file: String,
    /// Source language.
    pub language: String,
    /// The abstract type/interface being bound.
    pub abstract_type: String,
    /// The concrete implementation type.
    pub concrete_type: String,
    /// Lifetime/scope (singleton, transient, scoped).
    pub lifetime: Option<String>,
    /// Line number.
    pub line: usize,
    /// Framework that manages this binding.
    pub framework: String,
}
