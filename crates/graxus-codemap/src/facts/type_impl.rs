use serde::{Deserialize, Serialize};

/// Trait/interface implementation mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeImplFact {
    /// Unique identifier.
    pub id: String,
    /// File where the implementation is defined.
    pub file: String,
    /// Source language.
    pub language: String,
    /// The type being implemented (e.g. struct name, class name).
    pub implementing_type: String,
    /// The trait/interface being implemented.
    pub trait_or_interface: String,
    /// Line number.
    pub line: usize,
    /// Whether this is a direct implementation or extends/derives.
    pub kind: ImplKind,
}

/// Kind of type implementation relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplKind {
    /// Rust: impl Trait for Type
    TraitImpl,
    /// Rust: derive(DeriveMacro)
    Derive,
    /// TypeScript: class Foo implements Bar
    Implements,
    /// TypeScript/Java: class Foo extends Bar
    Extends,
    /// C#: class Foo : IBar
    CSharpInheritance,
    /// C++: class Foo : public Bar
    CppInheritance,
}
