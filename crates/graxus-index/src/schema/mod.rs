pub mod calls;
pub mod di_bindings;
pub mod imports;
pub mod parser_facts;
pub mod routes;
pub mod symbols;
pub mod type_impls;

const BASE_STATEMENTS: &str = r#"
CREATE TABLE IF NOT EXISTS files (
    path TEXT PRIMARY KEY,
    language TEXT NOT NULL,
    hash TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS doc_nodes (
    id TEXT PRIMARY KEY,
    node_type TEXT NOT NULL,
    path TEXT NOT NULL,
    title TEXT NOT NULL,
    tags TEXT,
    headings TEXT
);
CREATE TABLE IF NOT EXISTS doc_edges (
    from_id TEXT NOT NULL,
    to_id TEXT NOT NULL,
    edge_type TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS bridge_edges (
    from_id TEXT NOT NULL,
    to_id TEXT NOT NULL,
    edge_type TEXT NOT NULL,
    confidence TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

pub fn all_statements() -> String {
    [
        BASE_STATEMENTS,
        symbols::STATEMENTS,
        imports::STATEMENTS,
        calls::STATEMENTS,
        parser_facts::STATEMENTS,
        routes::STATEMENTS,
        type_impls::STATEMENTS,
        di_bindings::STATEMENTS,
    ]
    .join("\n")
}
