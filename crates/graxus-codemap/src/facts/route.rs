use serde::{Deserialize, Serialize};

/// HTTP/API route endpoint extracted from framework annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteFact {
    /// Unique identifier for this route.
    pub id: String,
    /// File where this route is defined.
    pub file: String,
    /// Source language.
    pub language: String,
    /// HTTP method (GET, POST, PUT, DELETE, PATCH, etc. or "*" for all).
    pub method: String,
    /// Route path pattern (e.g. "/api/users/:id").
    pub path: String,
    /// Handler function name.
    pub handler: String,
    /// Handler's resolved file path.
    pub handler_file: Option<String>,
    /// Line number where route is registered.
    pub line: usize,
    /// Framework that defined this route.
    pub framework: String,
    /// Middleware applied to this route.
    pub middleware: Vec<String>,
}
